use super::{ClipboardError, FileClipboard, dispatcher::platform_command, freedesktop};
use anyhow::{Context, Result, anyhow};
use std::{
    io::Write,
    os::fd::{AsRawFd, OwnedFd},
    path::PathBuf,
    sync::mpsc,
    thread,
};
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

pub(super) fn read_files() -> Option<FileClipboard> {
    let types = types()?;
    let refs = types.iter().map(String::as_str).collect::<Vec<_>>();
    freedesktop::read_file_clipboard(read_type, &refs)
}

pub(super) fn read_text() -> Option<String> {
    let types = types()?;
    let refs = types.iter().map(String::as_str).collect::<Vec<_>>();
    freedesktop::read_text(read_type, &refs)
}

pub(super) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError> {
    offer(freedesktop::file_payloads(paths)).map_err(|_| ClipboardError::Unavailable)
}

pub(super) fn write_text(text: &str) -> Result<(), ClipboardError> {
    platform_command("wl-copy", &["--type", "text/plain;charset=utf-8"], text)
}

fn types() -> Option<Vec<String>> {
    let output = command("wl-paste", &["--list-types"])?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .map(str::to_owned)
            .collect()
    })
}

fn read_type(mime: &str) -> Option<Vec<u8>> {
    let output = command("wl-paste", &["--no-newline", "--type", mime])?;
    output.status.success().then_some(output.stdout)
}

fn command(program: &str, args: &[&str]) -> Option<std::process::Output> {
    std::process::Command::new(program).args(args).output().ok()
}

struct State {
    manager: Option<ZwlrDataControlManagerV1>,
    seat: Option<WlSeat>,
    payloads: Vec<(String, Vec<u8>)>,
    cancelled: bool,
}

impl State {
    fn new(payloads: Vec<(String, Vec<u8>)>) -> Self {
        Self {
            manager: None,
            seat: None,
            payloads,
            cancelled: false,
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
        if matches!(event, zwlr_data_control_device_v1::Event::Finished) {
            state.cancelled = true;
        }
    }
}

impl Dispatch<ZwlrDataControlOfferV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrDataControlOfferV1,
        _: zwlr_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
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
