use super::{ClipboardError, FileClipboard, freedesktop};
use anyhow::{Context, Result, anyhow};
use std::{
    io::{Read, Write},
    os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd},
    path::PathBuf,
    sync::mpsc,
    thread,
};

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
    let Ok(mut reader) = Reader::open() else {
        return ReadResult::Unavailable;
    };
    let types = reader.types().to_vec();
    let refs = types.iter().map(String::as_str).collect::<Vec<_>>();
    match freedesktop::read_file_clipboard(|mime| reader.read_type(mime), &refs) {
        Some(files) => ReadResult::Value(files),
        None => ReadResult::Empty,
    }
}

pub(super) fn read_text() -> ReadResult<String> {
    let Ok(mut reader) = Reader::open() else {
        return ReadResult::Unavailable;
    };
    let types = reader.types().to_vec();
    let refs = types.iter().map(String::as_str).collect::<Vec<_>>();
    match freedesktop::read_text(|mime| reader.read_type(mime), &refs) {
        Some(text) => ReadResult::Value(text),
        None => ReadResult::Empty,
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
            .context("compositor lacks Wayland data-control clipboard support")?
            .clone();
        let seat = state
            .seat
            .as_ref()
            .context("Wayland session has no seat")?
            .clone();
        let device = manager.get_data_device(&seat, &qh, ());
        queue.roundtrip(&mut state)?;
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
        let (read_fd, write_fd) = pipe().ok()?;
        offer.receive(mime.to_string(), write_fd.as_fd());
        self.queue.flush().ok()?;
        drop(write_fd);
        let mut bytes = Vec::new();
        std::fs::File::from(read_fd).read_to_end(&mut bytes).ok()?;
        Some(bytes)
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
            .context("compositor lacks Wayland data-control clipboard support")?;
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
