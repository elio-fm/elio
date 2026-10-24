use super::{ClipboardError, FileClipboard, freedesktop};
use anyhow::{Context, Result, anyhow};
use std::{
    io::Write,
    os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd},
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const CLIPBOARD_READ_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_CLIPBOARD_READ_BYTES: usize = 8 * 1024 * 1024;

/// A Wayland selection can be current yet irrelevant to the requested paste.
/// Keep that distinct from an unavailable data-control protocol so callers do
/// not resurrect an older XWayland selection.
pub(super) enum ReadResult<T> {
    Unavailable,
    Empty,
    Value(T),
}
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle,
    protocol::{wl_registry, wl_seat::WlSeat},
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::{self, ExtDataControlManagerV1},
    ext_data_control_offer_v1::{self, ExtDataControlOfferV1},
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::{self, ZwlrDataControlOfferV1},
    zwlr_data_control_source_v1::{self, ZwlrDataControlSourceV1},
};

/// Owns a multi-MIME Wayland clipboard offer until another application replaces
/// it or elio exits. This does not require, or integrate with, a clipboard
/// manager. A manager may persist the offer after elio exits, but must retain
/// all offered MIME types for a later paste to remain a file operation.
pub(super) fn offer(payloads: Vec<(String, Vec<u8>)>) -> Result<()> {
    match offer_ext(payloads.clone()) {
        Ok(()) => return Ok(()),
        Err(error) => debug(format_args!(
            "ext-data-control unavailable for write: {error:#}"
        )),
    }
    offer_wlr(payloads)
}

fn offer_wlr(payloads: Vec<(String, Vec<u8>)>) -> Result<()> {
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("elio-file-clipboard".to_string())
        .spawn(move || run_offer(payloads, ready_tx))
        .context("start Wayland clipboard worker")?;

    ready_rx
        .recv()
        .map_err(|_| anyhow!("Wayland clipboard worker stopped before it was ready"))?
}

pub(super) fn read_files() -> ReadResult<FileClipboard> {
    match ExtReader::open() {
        Ok(mut reader) => return read_files_from(&mut reader),
        Err(error) => debug(format_args!(
            "ext-data-control unavailable while reading files: {error:#}"
        )),
    }
    let mut reader = match Reader::open() {
        Ok(reader) => reader,
        Err(error) => {
            debug(format_args!(
                "zwlr-data-control unavailable while reading files: {error:#}"
            ));
            return ReadResult::Unavailable;
        }
    };
    read_files_from(&mut reader)
}

fn read_files_from(reader: &mut impl ClipboardReader) -> ReadResult<FileClipboard> {
    let types = reader.types().to_vec();
    let refs = types.iter().map(String::as_str).collect::<Vec<_>>();
    match freedesktop::read_file_clipboard(|mime| reader.read_type(mime), &refs) {
        Some(files) => ReadResult::Value(files),
        None => {
            debug(format_args!(
                "selection has no usable local-file representation"
            ));
            ReadResult::Empty
        }
    }
}

pub(super) fn read_text() -> ReadResult<String> {
    match ExtReader::open() {
        Ok(mut reader) => return read_text_from(&mut reader),
        Err(error) => debug(format_args!(
            "ext-data-control unavailable while reading text: {error:#}"
        )),
    }
    let mut reader = match Reader::open() {
        Ok(reader) => reader,
        Err(error) => {
            debug(format_args!(
                "zwlr-data-control unavailable while reading text: {error:#}"
            ));
            return ReadResult::Unavailable;
        }
    };
    read_text_from(&mut reader)
}

fn read_text_from(reader: &mut impl ClipboardReader) -> ReadResult<String> {
    let types = reader.types().to_vec();
    let refs = types.iter().map(String::as_str).collect::<Vec<_>>();
    match freedesktop::read_text(|mime| reader.read_type(mime), &refs) {
        Some(text) => ReadResult::Value(text),
        None => {
            debug(format_args!("selection has no usable text representation"));
            ReadResult::Empty
        }
    }
}

pub(super) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError> {
    offer(freedesktop::file_payloads(paths)).map_err(|_| ClipboardError::Unavailable)
}

pub(super) fn write_text(text: &str) -> Result<(), ClipboardError> {
    offer(freedesktop::text_payloads(text)).map_err(|_| ClipboardError::Unavailable)
}

struct State {
    manager: Option<ZwlrDataControlManagerV1>,
    seat: Option<WlSeat>,
    payloads: Vec<(String, Vec<u8>)>,
    cancelled: bool,
    selection: Option<ZwlrDataControlOfferV1>,
    selection_types: Vec<String>,
    pending_offer_types: Vec<String>,
}

impl State {
    fn new(payloads: Vec<(String, Vec<u8>)>) -> Self {
        Self {
            manager: None,
            seat: None,
            payloads,
            cancelled: false,
            selection: None,
            selection_types: Vec::new(),
            pending_offer_types: Vec::new(),
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "zwlr_data_control_manager_v1" => {
                    state.manager = Some(registry.bind::<ZwlrDataControlManagerV1, _, _>(
                        name,
                        version.min(2),
                        qh,
                        (),
                    ));
                }
                "wl_seat" if state.seat.is_none() => {
                    state.seat = Some(registry.bind::<WlSeat, _, _>(name, version.min(7), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ZwlrDataControlManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrDataControlManagerV1,
        _: wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: wayland_client::protocol::wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrDataControlDeviceV1, ()> for State {
    fn event_created_child(
        opcode: u16,
        qh: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            0 => qh.make_data::<ZwlrDataControlOfferV1, ()>(()),
            _ => panic!("unexpected data-control device child opcode {opcode}"),
        }
    }

    fn event(
        state: &mut Self,
        _: &ZwlrDataControlDeviceV1,
        event: zwlr_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_data_control_device_v1::Event::DataOffer { .. } => {
                state.pending_offer_types.clear();
            }
            zwlr_data_control_device_v1::Event::Selection { id } => {
                state.selection = id;
                state.selection_types = std::mem::take(&mut state.pending_offer_types);
            }
            zwlr_data_control_device_v1::Event::Finished => state.cancelled = true,
            _ => {}
        }
    }
}

impl Dispatch<ZwlrDataControlOfferV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrDataControlOfferV1,
        event: zwlr_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = event {
            state.pending_offer_types.push(mime_type);
        }
    }
}

struct Reader {
    _connection: Connection,
    queue: EventQueue<State>,
    _device: ZwlrDataControlDeviceV1,
    offer: Option<ZwlrDataControlOfferV1>,
    types: Vec<String>,
}

trait ClipboardReader {
    fn types(&self) -> &[String];
    fn read_type(&mut self, mime: &str) -> Option<Vec<u8>>;
}

impl Reader {
    fn open() -> Result<Self> {
        let connection = Connection::connect_to_env().context("connect to Wayland")?;
        let mut queue = connection.new_event_queue();
        let qh = queue.handle();
        let display = connection.display();
        let _registry = display.get_registry(&qh, ());
        let mut state = State::new(Vec::new());
        queue.roundtrip(&mut state)?;
        let manager = state
            .manager
            .as_ref()
            .context("compositor lacks zwlr-data-control clipboard support")?
            .clone();
        let seat = state
            .seat
            .as_ref()
            .context("Wayland session has no seat")?
            .clone();
        let device = manager.get_data_device(&seat, &qh, ());
        queue.roundtrip(&mut state)?;
        debug(format_args!(
            "zwlr-data-control selected; selection offer: {:?}",
            state.selection_types
        ));
        Ok(Self {
            _connection: connection,
            queue,
            _device: device,
            offer: state.selection.clone(),
            types: state.selection_types.clone(),
        })
    }

    fn types(&self) -> &[String] {
        &self.types
    }

    fn read_type(&mut self, mime: &str) -> Option<Vec<u8>> {
        let offer = self.offer.as_ref()?;
        let (read_fd, write_fd) = match pipe() {
            Ok(pipe) => pipe,
            Err(error) => {
                debug(format_args!(
                    "zwlr-data-control cannot create pipe for {mime}: {error}"
                ));
                return None;
            }
        };
        offer.receive(mime.to_string(), write_fd.as_fd());
        if let Err(error) = self.queue.flush() {
            debug(format_args!(
                "zwlr-data-control cannot request {mime}: {error}"
            ));
            return None;
        }
        drop(write_fd);
        let bytes = read_pipe(read_fd, mime, "zwlr-data-control")?;
        debug(format_args!(
            "zwlr-data-control read {mime}: {} bytes",
            bytes.len()
        ));
        Some(bytes)
    }
}

impl ClipboardReader for Reader {
    fn types(&self) -> &[String] {
        self.types()
    }

    fn read_type(&mut self, mime: &str) -> Option<Vec<u8>> {
        self.read_type(mime)
    }
}

fn debug(message: std::fmt::Arguments<'_>) {
    // Developer-only transport trace for compositor-specific clipboard reports.
    if std::env::var_os("ELIO_DEBUG_WAYLAND_CLIPBOARD").is_some() {
        eprintln!("elio Wayland clipboard: {message}");
    }
}

fn pipe() -> std::io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0; 2];
    // `pipe` initializes both entries on success, transferring ownership to
    // the two OwnedFd values below.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// Read clipboard data without allowing an external Wayland owner to block the
/// terminal forever or make it allocate an unbounded amount of memory.
fn read_pipe(read_fd: OwnedFd, mime: &str, protocol: &str) -> Option<Vec<u8>> {
    let raw = read_fd.as_raw_fd();
    let deadline = Instant::now() + CLIPBOARD_READ_TIMEOUT;
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];

    loop {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        let timeout_ms = remaining.as_millis().max(1).min(i32::MAX as u128) as i32;
        let mut poll_fd = libc::pollfd {
            fd: raw,
            events: libc::POLLIN | libc::POLLHUP,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut poll_fd, 1, timeout_ms) };
        if ready == 0 {
            debug(format_args!("{protocol} timed out reading {mime}"));
            return None;
        }
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            debug(format_args!("{protocol} cannot wait for {mime}: {error}"));
            return None;
        }
        if poll_fd.revents & libc::POLLNVAL != 0 {
            debug(format_args!(
                "{protocol} received an invalid pipe for {mime}"
            ));
            return None;
        }

        let read = unsafe { libc::read(raw, chunk.as_mut_ptr().cast(), chunk.len()) };
        if read == 0 {
            return Some(bytes);
        }
        if read < 0 {
            let error = std::io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(libc::EINTR) | Some(libc::EAGAIN)) {
                continue;
            }
            debug(format_args!("{protocol} cannot read {mime}: {error}"));
            return None;
        }

        let read = read as usize;
        if bytes.len().saturating_add(read) > MAX_CLIPBOARD_READ_BYTES {
            debug(format_args!(
                "{protocol} rejected {mime}: clipboard data exceeds {MAX_CLIPBOARD_READ_BYTES} bytes"
            ));
            return None;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
}

struct ExtState {
    manager: Option<ExtDataControlManagerV1>,
    seat: Option<WlSeat>,
    payloads: Vec<(String, Vec<u8>)>,
    cancelled: bool,
    selection: Option<ExtDataControlOfferV1>,
    selection_types: Vec<String>,
    pending_offer_types: Vec<String>,
}

impl ExtState {
    fn new(payloads: Vec<(String, Vec<u8>)>) -> Self {
        Self {
            manager: None,
            seat: None,
            payloads,
            cancelled: false,
            selection: None,
            selection_types: Vec::new(),
            pending_offer_types: Vec::new(),
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for ExtState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "ext_data_control_manager_v1" => {
                    state.manager = Some(registry.bind::<ExtDataControlManagerV1, _, _>(
                        name,
                        version.min(1),
                        qh,
                        (),
                    ))
                }
                "wl_seat" if state.seat.is_none() => {
                    state.seat = Some(registry.bind::<WlSeat, _, _>(name, version.min(7), qh, ()))
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ExtDataControlManagerV1, ()> for ExtState {
    fn event(
        _: &mut Self,
        _: &ExtDataControlManagerV1,
        _: ext_data_control_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for ExtState {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: wayland_client::protocol::wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for ExtState {
    fn event_created_child(
        opcode: u16,
        qh: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            0 => qh.make_data::<ExtDataControlOfferV1, ()>(()),
            _ => panic!("unexpected ext data-control device child opcode {opcode}"),
        }
    }

    fn event(
        state: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::DataOffer { .. } => {
                state.pending_offer_types.clear()
            }
            ext_data_control_device_v1::Event::Selection { id } => {
                state.selection = id;
                state.selection_types = std::mem::take(&mut state.pending_offer_types);
            }
            ext_data_control_device_v1::Event::Finished => state.cancelled = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtDataControlOfferV1, ()> for ExtState {
    fn event(
        state: &mut Self,
        _: &ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            state.pending_offer_types.push(mime_type);
        }
    }
}

impl Dispatch<ExtDataControlSourceV1, ()> for ExtState {
    fn event(
        state: &mut Self,
        _: &ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_source_v1::Event::Send { mime_type, fd } => {
                let fd: OwnedFd = fd;
                if let Some((_, data)) = state.payloads.iter().find(|(mime, _)| mime == &mime_type)
                {
                    let _ = write_payload(fd, data);
                }
            }
            ext_data_control_source_v1::Event::Cancelled => state.cancelled = true,
            _ => {}
        }
    }
}

struct ExtReader {
    _connection: Connection,
    queue: EventQueue<ExtState>,
    _device: ExtDataControlDeviceV1,
    offer: Option<ExtDataControlOfferV1>,
    types: Vec<String>,
}

impl ExtReader {
    fn open() -> Result<Self> {
        let connection = Connection::connect_to_env().context("connect to Wayland")?;
        let mut queue = connection.new_event_queue();
        let qh = queue.handle();
        let _registry = connection.display().get_registry(&qh, ());
        let mut state = ExtState::new(Vec::new());
        queue.roundtrip(&mut state)?;
        let manager = state
            .manager
            .as_ref()
            .context("compositor lacks ext-data-control clipboard support")?
            .clone();
        let seat = state
            .seat
            .as_ref()
            .context("Wayland session has no seat")?
            .clone();
        let device = manager.get_data_device(&seat, &qh, ());
        queue.roundtrip(&mut state)?;
        debug(format_args!(
            "ext-data-control selected; selection offer: {:?}",
            state.selection_types
        ));
        Ok(Self {
            _connection: connection,
            queue,
            _device: device,
            offer: state.selection.clone(),
            types: state.selection_types.clone(),
        })
    }
}

impl ClipboardReader for ExtReader {
    fn types(&self) -> &[String] {
        &self.types
    }

    fn read_type(&mut self, mime: &str) -> Option<Vec<u8>> {
        let offer = self.offer.as_ref()?;
        let (read_fd, write_fd) = match pipe() {
            Ok(pipe) => pipe,
            Err(error) => {
                debug(format_args!(
                    "ext-data-control cannot create pipe for {mime}: {error}"
                ));
                return None;
            }
        };
        offer.receive(mime.to_string(), write_fd.as_fd());
        if let Err(error) = self.queue.flush() {
            debug(format_args!(
                "ext-data-control cannot request {mime}: {error}"
            ));
            return None;
        }
        drop(write_fd);
        let bytes = read_pipe(read_fd, mime, "ext-data-control")?;
        debug(format_args!(
            "ext-data-control read {mime}: {} bytes",
            bytes.len()
        ));
        Some(bytes)
    }
}

fn offer_ext(payloads: Vec<(String, Vec<u8>)>) -> Result<()> {
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("elio-ext-file-clipboard".to_string())
        .spawn(move || run_ext_offer(payloads, ready_tx))
        .context("start ext Wayland clipboard worker")?;
    ready_rx
        .recv()
        .map_err(|_| anyhow!("ext Wayland clipboard worker stopped before it was ready"))?
}

fn run_ext_offer(payloads: Vec<(String, Vec<u8>)>, ready: mpsc::SyncSender<Result<()>>) {
    let result = (|| -> Result<()> {
        let connection = Connection::connect_to_env().context("connect to Wayland")?;
        let mut queue: EventQueue<ExtState> = connection.new_event_queue();
        let qh = queue.handle();
        let _registry = connection.display().get_registry(&qh, ());
        let mut state = ExtState::new(payloads);
        queue.roundtrip(&mut state)?;
        let manager = state
            .manager
            .as_ref()
            .context("compositor lacks ext-data-control clipboard support")?
            .clone();
        debug(format_args!("ext-data-control selected for write"));
        let seat = state
            .seat
            .as_ref()
            .context("Wayland session has no seat")?
            .clone();
        let device = manager.get_data_device(&seat, &qh, ());
        let source = manager.create_data_source(&qh, ());
        for (mime, _) in &state.payloads {
            source.offer(mime.clone());
        }
        device.set_selection(Some(&source));
        queue.flush()?;
        ready.send(Ok(())).ok();
        while !state.cancelled {
            queue.blocking_dispatch(&mut state)?;
        }
        source.destroy();
        device.destroy();
        Ok(())
    })();
    if let Err(error) = result {
        ready.send(Err(error)).ok();
    }
}

impl Dispatch<ZwlrDataControlSourceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrDataControlSourceV1,
        event: zwlr_data_control_source_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_data_control_source_v1::Event::Send { mime_type, fd } => {
                let fd: OwnedFd = fd;
                if let Some((_, data)) = state.payloads.iter().find(|(mime, _)| mime == &mime_type)
                {
                    let _ = write_payload(fd, data);
                }
            }
            zwlr_data_control_source_v1::Event::Cancelled => state.cancelled = true,
            _ => {}
        }
    }
}

fn run_offer(payloads: Vec<(String, Vec<u8>)>, ready: mpsc::SyncSender<Result<()>>) {
    let result = (|| -> Result<()> {
        let connection = Connection::connect_to_env().context("connect to Wayland")?;
        let mut queue: EventQueue<State> = connection.new_event_queue();
        let qh = queue.handle();
        let display = connection.display();
        let _registry = display.get_registry(&qh, ());
        let mut state = State::new(payloads);
        queue.roundtrip(&mut state)?;

        let manager = state
            .manager
            .as_ref()
            .context("compositor lacks zwlr-data-control clipboard support")?;
        debug(format_args!("zwlr-data-control selected for write"));
        let seat = state.seat.as_ref().context("Wayland session has no seat")?;
        let device = manager.get_data_device(seat, &qh, ());
        let source = manager.create_data_source(&qh, ());
        for (mime, _) in &state.payloads {
            source.offer(mime.clone());
        }
        device.set_selection(Some(&source));
        queue.flush()?;
        ready.send(Ok(())).ok();

        while !state.cancelled {
            queue.blocking_dispatch(&mut state)?;
        }
        source.destroy();
        device.destroy();
        Ok(())
    })();

    if let Err(error) = result {
        ready.send(Err(error)).ok();
    }
}

fn write_payload(fd: OwnedFd, data: &[u8]) -> Result<()> {
    let raw = fd.as_raw_fd();
    // `raw` remains valid for the duration of this function because `fd` owns
    // it. `fcntl` only reads or updates descriptor flags.
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error()).context("read clipboard pipe flags");
    }
    if flags & libc::O_NONBLOCK != 0
        && unsafe { libc::fcntl(raw, libc::F_SETFL, flags & !libc::O_NONBLOCK) } < 0
    {
        return Err(std::io::Error::last_os_error()).context("set clipboard pipe blocking");
    }
    let mut file = std::fs::File::from(fd);
    file.write_all(data).context("write clipboard data")?;
    file.flush().context("flush clipboard data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_reader_collects_a_complete_payload() {
        let (read_fd, write_fd) = pipe().expect("create pipe");
        let writer = thread::spawn(move || write_payload(write_fd, b"clipboard data"));

        assert_eq!(
            read_pipe(read_fd, "text/plain", "test"),
            Some(b"clipboard data".to_vec())
        );
        writer
            .join()
            .expect("writer thread")
            .expect("write payload");
    }

    #[test]
    fn pipe_reader_times_out_when_owner_never_writes() {
        let (read_fd, write_fd) = pipe().expect("create pipe");
        let started = Instant::now();

        assert_eq!(read_pipe(read_fd, "text/plain", "test"), None);
        assert!(
            started.elapsed() >= CLIPBOARD_READ_TIMEOUT - Duration::from_millis(50),
            "clipboard read returned before its timeout"
        );
        drop(write_fd);
    }
}
