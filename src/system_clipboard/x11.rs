//! Native X11 selection clipboard transport.
//!
//! X11 clipboard contents live in the selection owner, not on the server. An
//! owner worker therefore remains connected after a copy and responds to
//! selection requests until another client takes CLIPBOARD ownership.

use super::{ClipboardError, FileClipboard, freedesktop};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use x11rb::{
    COPY_FROM_PARENT, atom_manager,
    connection::Connection,
    protocol::{
        Event,
        xproto::{
            Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, CreateWindowAux,
            EventMask, GetPropertyReply, PropMode, Property, SELECTION_NOTIFY_EVENT,
            SelectionNotifyEvent, Window, WindowClass,
        },
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

const EVENT_WAIT_STEP: Duration = Duration::from_millis(2);
const SELECTION_TIMEOUT: Duration = Duration::from_secs(2);
// Conservative enough for old X servers and Xwayland; larger data uses INCR.
const INCR_CHUNK_SIZE: usize = 60 * 1024;

atom_manager! {
    Atoms:
    AtomsCookie {
        CLIPBOARD,
        TARGETS,
        TIMESTAMP,
        INCR,
        UTF8_STRING,
        TEXT,
        STRING,
        TEXT_URI_LIST: b"text/uri-list",
        KDE_URI_LIST: b"application/x-kde4-urilist",
        GNOME_COPIED_FILES: b"x-special/gnome-copied-files",
        KDE_CUT_SELECTION: b"application/x-kde-cutselection",
        TEXT_PLAIN: b"text/plain",
        TEXT_PLAIN_UTF8: b"text/plain;charset=utf-8",
        ELIO_SELECTION: b"_ELIO_SELECTION",
        ELIO_TIMESTAMP: b"_ELIO_TIMESTAMP",
    }
}

#[derive(Clone)]
struct Payload {
    type_: Atom,
    bytes: Vec<u8>,
}

struct PendingIncr {
    type_: Atom,
    bytes: Vec<u8>,
    offset: usize,
}

pub(super) fn read_files() -> Option<FileClipboard> {
    let mut clipboard = ClipboardReader::connect()?;
    let targets = clipboard.targets()?;
    let types = clipboard.target_names(&targets);
    freedesktop::read_file_clipboard(|mime| clipboard.read_named(mime), &types)
}

pub(super) fn read_text() -> Option<String> {
    let mut clipboard = ClipboardReader::connect()?;
    let targets = clipboard.targets()?;
    let types = clipboard.target_names(&targets);
    freedesktop::read_text(|mime| clipboard.read_named(mime), &types)
}

pub(super) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError> {
    #[cfg(test)]
    if let Some(tool) = std::env::var_os("ELIO_TEST_ITEM_CLIPBOARD_TOOL") {
        return super::dispatcher::platform_command(
            &tool.to_string_lossy(),
            &[],
            &freedesktop::uri_list(paths),
        );
    }
    offer(freedesktop::file_payloads(paths))
}

pub(super) fn write_text(text: &str) -> Result<(), ClipboardError> {
    #[cfg(test)]
    if let Some(tool) = std::env::var_os("ELIO_TEST_CLIPBOARD_TOOL") {
        return super::dispatcher::platform_command(&tool.to_string_lossy(), &[], text);
    }
    offer(vec![(
        "text/plain;charset=utf-8".to_string(),
        text.as_bytes().to_vec(),
    )])
}

fn offer(payloads: Vec<(String, Vec<u8>)>) -> Result<(), ClipboardError> {
    if payloads.is_empty() {
        return Err(ClipboardError::WriteFailed);
    }
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("elio-x11-clipboard".to_string())
        .spawn(move || run_owner(payloads, ready_tx))
        .map_err(|_| ClipboardError::Unavailable)?;
    ready_rx
        .recv_timeout(SELECTION_TIMEOUT)
        .ok()
        .and_then(Result::ok)
        .ok_or(ClipboardError::Unavailable)
}

fn run_owner(
    payloads: Vec<(String, Vec<u8>)>,
    ready: mpsc::SyncSender<Result<(), ClipboardError>>,
) {
    let result = (|| -> Result<(), ClipboardError> {
        let (connection, screen_num) =
            x11rb::connect(None).map_err(|_| ClipboardError::Unavailable)?;
        let window = input_window(&connection, screen_num)?;
        let atoms = Atoms::new(&connection)
            .map_err(|_| ClipboardError::Unavailable)?
            .reply()
            .map_err(|_| ClipboardError::Unavailable)?;
        let timestamp = server_timestamp(&connection, window, atoms.ELIO_TIMESTAMP)?;
        connection
            .set_selection_owner(window, atoms.CLIPBOARD, timestamp)
            .map_err(|_| ClipboardError::Unavailable)?;
        connection
            .flush()
            .map_err(|_| ClipboardError::Unavailable)?;
        if connection
            .get_selection_owner(atoms.CLIPBOARD)
            .map_err(|_| ClipboardError::Unavailable)?
            .reply()
            .map_err(|_| ClipboardError::Unavailable)?
            .owner
            != window
        {
            return Err(ClipboardError::Unavailable);
        }
        ready.send(Ok(())).ok();
        serve_selection(
            connection,
            atoms,
            timestamp,
            export_payloads(&atoms, payloads),
        )
    })();
    if let Err(error) = result {
        ready.send(Err(error)).ok();
    }
}

fn input_window(connection: &RustConnection, screen_num: usize) -> Result<Window, ClipboardError> {
    let window = connection
        .generate_id()
        .map_err(|_| ClipboardError::Unavailable)?;
    connection
        .create_window(
            COPY_FROM_PARENT as u8,
            window,
            connection.setup().roots[screen_num].root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(|_| ClipboardError::Unavailable)?;
    Ok(window)
}

fn server_timestamp(
    connection: &RustConnection,
    window: Window,
    property: Atom,
) -> Result<u32, ClipboardError> {
    connection
        .change_property8(PropMode::REPLACE, window, property, AtomEnum::STRING, &[0])
        .map_err(|_| ClipboardError::Unavailable)?;
    connection
        .flush()
        .map_err(|_| ClipboardError::Unavailable)?;
    loop {
        match connection
            .wait_for_event()
            .map_err(|_| ClipboardError::Unavailable)?
        {
            Event::PropertyNotify(event) if event.window == window && event.atom == property => {
                return Ok(event.time);
            }
            _ => {}
        }
    }
}

fn export_payloads(atoms: &Atoms, payloads: Vec<(String, Vec<u8>)>) -> HashMap<Atom, Payload> {
    let mut exported = HashMap::new();
    for (mime, bytes) in payloads {
        let atom = match mime.as_str() {
            "text/uri-list" => atoms.TEXT_URI_LIST,
            "application/x-kde4-urilist" => atoms.KDE_URI_LIST,
            "x-special/gnome-copied-files" => atoms.GNOME_COPIED_FILES,
            "application/x-kde-cutselection" => atoms.KDE_CUT_SELECTION,
            "text/plain;charset=utf-8" => atoms.TEXT_PLAIN_UTF8,
            _ => continue,
        };
        exported.insert(
            atom,
            Payload {
                type_: atom,
                bytes: bytes.clone(),
            },
        );
        if atom == atoms.TEXT_PLAIN_UTF8 {
            for plain_atom in [atoms.TEXT_PLAIN, atoms.UTF8_STRING] {
                exported.insert(
                    plain_atom,
                    Payload {
                        type_: atoms.UTF8_STRING,
                        bytes: bytes.clone(),
                    },
                );
            }
        }
    }
    exported
}

fn serve_selection(
    connection: RustConnection,
    atoms: Atoms,
    timestamp: u32,
    payloads: HashMap<Atom, Payload>,
) -> Result<(), ClipboardError> {
    let mut transfers = HashMap::<(Window, Atom), PendingIncr>::new();
    loop {
        match connection
            .wait_for_event()
            .map_err(|_| ClipboardError::Unavailable)?
        {
            Event::SelectionClear(event) if event.selection == atoms.CLIPBOARD => return Ok(()),
            Event::SelectionRequest(request) if request.selection == atoms.CLIPBOARD => {
                respond_to_request(
                    &connection,
                    &atoms,
                    timestamp,
                    &payloads,
                    &mut transfers,
                    request,
                )?;
            }
            Event::PropertyNotify(event) if event.state == Property::DELETE => {
                if let Some(transfer) = transfers.get_mut(&(event.window, event.atom))
                    && send_next_incr_chunk(&connection, event.window, event.atom, transfer)?
                {
                    transfers.remove(&(event.window, event.atom));
                }
            }
            _ => {}
        }
    }
}

fn respond_to_request(
    connection: &RustConnection,
    atoms: &Atoms,
    timestamp: u32,
    payloads: &HashMap<Atom, Payload>,
    transfers: &mut HashMap<(Window, Atom), PendingIncr>,
    request: x11rb::protocol::xproto::SelectionRequestEvent,
) -> Result<(), ClipboardError> {
    let property = if request.property == u32::from(AtomEnum::NONE) {
        request.target
    } else {
        request.property
    };
    let mut reply_property = property;
    if request.target == atoms.TARGETS {
        let mut targets = vec![atoms.TARGETS, atoms.TIMESTAMP];
        targets.extend(payloads.keys().copied());
        connection
            .change_property32(
                PropMode::REPLACE,
                request.requestor,
                property,
                AtomEnum::ATOM,
                &targets,
            )
            .map_err(|_| ClipboardError::Unavailable)?;
    } else if request.target == atoms.TIMESTAMP {
        connection
            .change_property32(
                PropMode::REPLACE,
                request.requestor,
                property,
                AtomEnum::INTEGER,
                &[timestamp],
            )
            .map_err(|_| ClipboardError::Unavailable)?;
    } else if let Some(payload) = payloads.get(&request.target) {
        if payload.bytes.len() > INCR_CHUNK_SIZE {
            connection
                .change_window_attributes(
                    request.requestor,
                    &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
                )
                .map_err(|_| ClipboardError::Unavailable)?;
            connection
                .change_property32(
                    PropMode::REPLACE,
                    request.requestor,
                    property,
                    atoms.INCR,
                    &[payload
                        .bytes
                        .len()
                        .try_into()
                        .map_err(|_| ClipboardError::WriteFailed)?],
                )
                .map_err(|_| ClipboardError::Unavailable)?;
            transfers.insert(
                (request.requestor, property),
                PendingIncr {
                    type_: payload.type_,
                    bytes: payload.bytes.clone(),
                    offset: 0,
                },
            );
        } else {
            connection
                .change_property8(
                    PropMode::REPLACE,
                    request.requestor,
                    property,
                    payload.type_,
                    &payload.bytes,
                )
                .map_err(|_| ClipboardError::Unavailable)?;
        }
    } else {
        reply_property = u32::from(AtomEnum::NONE);
    }
    connection
        .send_event(
            false,
            request.requestor,
            EventMask::NO_EVENT,
            selection_notify_event(
                request.time,
                request.requestor,
                request.selection,
                request.target,
                reply_property,
            ),
        )
        .map_err(|_| ClipboardError::Unavailable)?;
    connection.flush().map_err(|_| ClipboardError::Unavailable)
}

fn selection_notify_event(
    time: u32,
    requestor: Window,
    selection: Atom,
    target: Atom,
    property: Atom,
) -> SelectionNotifyEvent {
    SelectionNotifyEvent {
        response_type: SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time,
        requestor,
        selection,
        target,
        property,
    }
}

fn send_next_incr_chunk(
    connection: &RustConnection,
    requestor: Window,
    property: Atom,
    transfer: &mut PendingIncr,
) -> Result<bool, ClipboardError> {
    if transfer.offset == transfer.bytes.len() {
        connection
            .change_property8(PropMode::REPLACE, requestor, property, transfer.type_, &[])
            .map_err(|_| ClipboardError::Unavailable)?;
        connection
            .flush()
            .map_err(|_| ClipboardError::Unavailable)?;
        return Ok(true);
    }
    let end = next_incr_chunk_end(transfer.offset, transfer.bytes.len())
        .expect("an unfinished INCR transfer has a next chunk");
    connection
        .change_property8(
            PropMode::REPLACE,
            requestor,
            property,
            transfer.type_,
            &transfer.bytes[transfer.offset..end],
        )
        .map_err(|_| ClipboardError::Unavailable)?;
    transfer.offset = end;
    connection
        .flush()
        .map_err(|_| ClipboardError::Unavailable)?;
    Ok(false)
}

fn next_incr_chunk_end(offset: usize, length: usize) -> Option<usize> {
    (offset < length).then(|| (offset + INCR_CHUNK_SIZE).min(length))
}

struct ClipboardReader {
    connection: RustConnection,
    window: Window,
    atoms: Atoms,
}

impl ClipboardReader {
    fn connect() -> Option<Self> {
        let (connection, screen_num) = x11rb::connect(None).ok()?;
        let window = input_window(&connection, screen_num).ok()?;
        let atoms = Atoms::new(&connection).ok()?.reply().ok()?;
        connection.flush().ok()?;
        Some(Self {
            connection,
            window,
            atoms,
        })
    }

    fn targets(&mut self) -> Option<Vec<Atom>> {
        let property = self.selection_property(self.atoms.TARGETS)?;
        let reply = self
            .connection
            .get_property(false, self.window, property, AtomEnum::ATOM, 0, u32::MAX)
            .ok()?
            .reply()
            .ok()?;
        self.connection
            .delete_property(self.window, property)
            .ok()?;
        self.connection.flush().ok()?;
        atoms_from_property(&reply)
    }

    fn target_names(&self, targets: &[Atom]) -> Vec<&'static str> {
        [
            (
                self.atoms.GNOME_COPIED_FILES,
                "x-special/gnome-copied-files",
            ),
            (
                self.atoms.KDE_CUT_SELECTION,
                "application/x-kde-cutselection",
            ),
            (self.atoms.TEXT_URI_LIST, "text/uri-list"),
            (self.atoms.KDE_URI_LIST, "application/x-kde4-urilist"),
            (self.atoms.TEXT_PLAIN_UTF8, "text/plain;charset=utf-8"),
            (self.atoms.TEXT_PLAIN, "text/plain"),
            (self.atoms.UTF8_STRING, "utf8_string"),
            (self.atoms.STRING, "string"),
            (self.atoms.TEXT, "text"),
        ]
        .into_iter()
        .filter_map(|(atom, name)| targets.contains(&atom).then_some(name))
        .collect()
    }

    fn read_named(&mut self, mime: &str) -> Option<Vec<u8>> {
        let target = match mime {
            "x-special/gnome-copied-files" => self.atoms.GNOME_COPIED_FILES,
            "application/x-kde-cutselection" => self.atoms.KDE_CUT_SELECTION,
            "text/uri-list" => self.atoms.TEXT_URI_LIST,
            "application/x-kde4-urilist" => self.atoms.KDE_URI_LIST,
            "text/plain;charset=utf-8" => self.atoms.TEXT_PLAIN_UTF8,
            "text/plain" => self.atoms.TEXT_PLAIN,
            "utf8_string" => self.atoms.UTF8_STRING,
            "string" => self.atoms.STRING,
            "text" => self.atoms.TEXT,
            _ => return None,
        };
        self.request(target)
    }

    fn request(&mut self, target: Atom) -> Option<Vec<u8>> {
        let property = self.selection_property(target)?;
        self.read_property(property, Instant::now() + SELECTION_TIMEOUT)
    }

    fn selection_property(&mut self, target: Atom) -> Option<Atom> {
        self.connection
            .convert_selection(
                self.window,
                self.atoms.CLIPBOARD,
                target,
                self.atoms.ELIO_SELECTION,
                0u32,
            )
            .ok()?;
        self.connection.flush().ok()?;
        let deadline = Instant::now() + SELECTION_TIMEOUT;
        loop {
            match poll_event_until(&self.connection, deadline)? {
                Event::SelectionNotify(event)
                    if event.requestor == self.window
                        && event.selection == self.atoms.CLIPBOARD
                        && event.target == target =>
                {
                    if event.property == u32::from(AtomEnum::NONE) {
                        return None;
                    }
                    return Some(event.property);
                }
                _ => {}
            }
        }
    }

    fn read_property(&mut self, property: Atom, deadline: Instant) -> Option<Vec<u8>> {
        let reply = self
            .connection
            .get_property(false, self.window, property, AtomEnum::ANY, 0, u32::MAX)
            .ok()?
            .reply()
            .ok()?;
        if reply.type_ != self.atoms.INCR {
            self.connection
                .delete_property(self.window, property)
                .ok()?;
            self.connection.flush().ok()?;
            return Some(reply.value);
        }
        self.connection
            .delete_property(self.window, property)
            .ok()?;
        self.connection.flush().ok()?;
        let mut bytes = Vec::new();
        loop {
            match poll_event_until(&self.connection, deadline)? {
                Event::PropertyNotify(event)
                    if event.window == self.window
                        && event.atom == property
                        && event.state == Property::NEW_VALUE =>
                {
                    let chunk = self
                        .connection
                        .get_property(true, self.window, property, AtomEnum::ANY, 0, u32::MAX)
                        .ok()?
                        .reply()
                        .ok()?;
                    if chunk.value.is_empty() {
                        return Some(bytes);
                    }
                    bytes.extend(chunk.value);
                    self.connection.flush().ok()?;
                }
                _ => {}
            }
        }
    }
}

fn poll_event_until(connection: &RustConnection, deadline: Instant) -> Option<Event> {
    loop {
        if let Some(event) = connection.poll_for_event().ok()? {
            return Some(event);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(EVENT_WAIT_STEP);
    }
}

fn atoms_from_property(reply: &GetPropertyReply) -> Option<Vec<Atom>> {
    reply.value32().map(Iterator::collect)
}

#[cfg(test)]
pub(super) fn atoms_from_property_for_test(reply: &GetPropertyReply) -> Option<Vec<Atom>> {
    atoms_from_property(reply)
}

#[cfg(test)]
pub(super) fn exported_target_names_for_test(
    payloads: Vec<(String, Vec<u8>)>,
) -> Vec<&'static str> {
    let mut names = Vec::new();
    for (mime, _) in payloads {
        match mime.as_str() {
            "text/uri-list" => names.push("text/uri-list"),
            "application/x-kde4-urilist" => names.push("application/x-kde4-urilist"),
            "x-special/gnome-copied-files" => names.push("x-special/gnome-copied-files"),
            "application/x-kde-cutselection" => names.push("application/x-kde-cutselection"),
            "text/plain;charset=utf-8" => {
                names.extend(["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"])
            }
            _ => {}
        }
    }
    names
}

#[cfg(test)]
pub(super) fn next_incr_chunk_end_for_test(offset: usize, length: usize) -> Option<usize> {
    next_incr_chunk_end(offset, length)
}

#[cfg(test)]
pub(super) fn selection_notify_event_for_test() -> SelectionNotifyEvent {
    selection_notify_event(1, 2, 3, 4, 5)
}
