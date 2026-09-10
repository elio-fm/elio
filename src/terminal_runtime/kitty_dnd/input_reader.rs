use std::{
    fs::OpenOptions,
    io::{self, Read},
    os::fd::AsRawFd,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
    time::Duration,
};

use crossterm::{event::Event, terminal};

use super::{KittyDndEvent, Osc72State, crossterm_parser, parse_osc72_with_state};
use crate::terminal_runtime::terminal_input::InputEvent;

const RESIZE_POLL_INTERVAL: Duration = Duration::from_millis(16);

pub(in crate::terminal_runtime) struct InputReader {
    receiver: Receiver<io::Result<InputEvent>>,
    paused: Arc<AtomicBool>,
}

impl InputReader {
    pub(in crate::terminal_runtime) fn spawn() -> io::Result<Self> {
        let tty = OpenOptions::new().read(true).open("/dev/tty")?;
        set_nonblocking(tty.as_raw_fd())?;
        let (sender, receiver) = mpsc::channel();
        let paused = Arc::new(AtomicBool::new(false));
        let thread_paused = Arc::clone(&paused);
        let resize_paused = Arc::clone(&paused);
        let resize_sender = sender.clone();
        thread::Builder::new()
            .name("elio-runtime-input".to_string())
            .spawn(move || read_loop(tty, sender, thread_paused))
            .map_err(io::Error::other)?;
        thread::Builder::new()
            .name("elio-runtime-resize".to_string())
            .spawn(move || resize_loop(resize_sender, resize_paused))
            .map_err(io::Error::other)?;
        Ok(Self { receiver, paused })
    }

    pub(in crate::terminal_runtime) fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    pub(in crate::terminal_runtime) fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> io::Result<Option<InputEvent>> {
        match self.receiver.recv_timeout(timeout) {
            Ok(event) => event.map(Some),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "input reader stopped",
            )),
        }
    }

    pub(in crate::terminal_runtime) fn try_recv(&self) -> io::Result<Option<InputEvent>> {
        match self.receiver.try_recv() {
            Ok(event) => event.map(Some),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "input reader stopped",
            )),
        }
    }
}

fn current_terminal_size() -> Option<(u16, u16)> {
    terminal::size().ok()
}

fn read_loop(
    mut tty: std::fs::File,
    sender: mpsc::Sender<io::Result<InputEvent>>,
    paused: Arc<AtomicBool>,
) {
    let mut buffer = Vec::<u8>::new();
    let mut parser = EventParser::default();
    let mut byte = [0u8; 1];
    loop {
        if paused.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        match tty.read(&mut byte) {
            Ok(0) => thread::sleep(Duration::from_millis(2)),
            Ok(_) => {
                buffer.push(byte[0]);
                parse_buffer(&mut parser, &mut buffer, &sender, true);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                parse_buffer(&mut parser, &mut buffer, &sender, false);
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => {
                let _ = sender.send(Err(error));
                break;
            }
        }
    }
}

fn resize_loop(sender: mpsc::Sender<io::Result<InputEvent>>, paused: Arc<AtomicBool>) {
    let mut last_size = current_terminal_size();
    loop {
        thread::sleep(RESIZE_POLL_INTERVAL);
        if paused.load(Ordering::Relaxed) {
            last_size = current_terminal_size();
            continue;
        }
        let Some(size) = current_terminal_size() else {
            continue;
        };
        if last_size == Some(size) {
            continue;
        }
        last_size = Some(size);
        if sender
            .send(Ok(InputEvent::Terminal(Event::Resize(size.0, size.1))))
            .is_err()
        {
            break;
        }
    }
}

fn parse_buffer(
    parser_state: &mut EventParser,
    buffer: &mut Vec<u8>,
    sender: &mpsc::Sender<io::Result<InputEvent>>,
    input_available: bool,
) {
    loop {
        if buffer.is_empty() {
            return;
        }
        let len_before = buffer.len();
        match parse_event(parser_state, buffer, input_available) {
            Ok(Some(event)) => {
                if buffer.len() == len_before {
                    buffer.clear();
                }
                let _ = sender.send(Ok(event));
            }
            Ok(None) => {
                if buffer.len() != len_before && !buffer.is_empty() {
                    continue;
                }
                return;
            }
            Err(error) => {
                buffer.clear();
                if !is_unsupported_input_sequence(&error) {
                    let _ = sender.send(Err(error));
                }
            }
        }
    }
}

fn is_unsupported_input_sequence(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Other && error.to_string() == "Could not parse an event."
}

fn set_nonblocking(fd: i32) -> io::Result<()> {
    // SAFETY: fcntl is called with a live /dev/tty fd and does not retain pointers.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: same fd; OR-ing O_NONBLOCK preserves existing flags.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[derive(Default)]
struct EventParser {
    kitty_dnd: KittyDndParser,
}

fn parse_event(
    parser: &mut EventParser,
    buffer: &mut Vec<u8>,
    input_available: bool,
) -> io::Result<Option<InputEvent>> {
    let len_before_dnd = buffer.len();
    if let Some(event) = parser.kitty_dnd.parse(buffer) {
        return Ok(Some(InputEvent::KittyDnd(event)));
    }
    if buffer.len() != len_before_dnd {
        return Ok(None);
    }
    if buffer.is_empty() {
        return Ok(None);
    }
    if starts_with_osc(buffer) && osc_end(buffer).is_none() {
        return Ok(None);
    }
    match crossterm_parser::parse_event(buffer, input_available) {
        Ok(Some(crossterm_parser::InternalEvent::Event(event))) => {
            Ok(Some(InputEvent::Terminal(event)))
        }
        Ok(Some(_)) => Ok(None),
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    }
}

#[derive(Default)]
struct KittyDndParser {
    state: Osc72State,
}

impl KittyDndParser {
    fn parse(&mut self, buffer: &mut Vec<u8>) -> Option<KittyDndEvent> {
        if !buffer.starts_with(b"\x1b]72;") {
            return None;
        }
        let end = osc_end(buffer)?;
        let sequence: Vec<u8> = buffer.drain(..end).collect();
        parse_osc72_with_state(&sequence, &mut self.state)
    }
}

fn starts_with_osc(buffer: &[u8]) -> bool {
    buffer.starts_with(b"\x1b]")
}

fn osc_end(buffer: &[u8]) -> Option<usize> {
    let mut index = 0;
    while index < buffer.len() {
        if buffer[index] == b'\x07' {
            return Some(index + 1);
        }
        if buffer[index] == b'\x1b' && buffer.get(index + 1) == Some(&b'\\') {
            return Some(index + 2);
        }
        index += 1;
    }
    None
}

#[cfg(test)]
#[path = "tests/input_reader.rs"]
mod tests;
