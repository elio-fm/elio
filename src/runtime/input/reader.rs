use std::io;

#[cfg(unix)]
use std::sync::mpsc;

#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use std::{
    fs::OpenOptions,
    io::Read,
    os::fd::AsRawFd,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, TryRecvError},
    },
    thread,
};

#[cfg(unix)]
use crossterm::{event::Event, terminal};

#[cfg(unix)]
use super::{RuntimeInputEvent, parser};

#[cfg(unix)]
const RESIZE_POLL_INTERVAL: Duration = Duration::from_millis(16);

pub(in crate::runtime) enum RuntimeInputReader {
    Crossterm,
    #[cfg(unix)]
    Custom(CustomInputReader),
}

impl RuntimeInputReader {
    pub(in crate::runtime) fn new(use_custom_reader: bool) -> io::Result<Self> {
        if use_custom_reader {
            #[cfg(unix)]
            {
                CustomInputReader::spawn().map(Self::Custom)
            }
            #[cfg(not(unix))]
            {
                Ok(Self::Crossterm)
            }
        } else {
            Ok(Self::Crossterm)
        }
    }
}

#[cfg(unix)]
pub(in crate::runtime) struct CustomInputReader {
    receiver: Receiver<io::Result<RuntimeInputEvent>>,
    paused: Arc<AtomicBool>,
}

#[cfg(unix)]
impl CustomInputReader {
    fn spawn() -> io::Result<Self> {
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

    pub(in crate::runtime) fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    pub(in crate::runtime) fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> io::Result<Option<RuntimeInputEvent>> {
        match self.receiver.recv_timeout(timeout) {
            Ok(event) => event.map(Some),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "input reader stopped",
            )),
        }
    }

    pub(in crate::runtime) fn try_recv(&self) -> io::Result<Option<RuntimeInputEvent>> {
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

#[cfg(unix)]
fn current_terminal_size() -> Option<(u16, u16)> {
    terminal::size().ok()
}

#[cfg(unix)]
fn read_loop(
    mut tty: std::fs::File,
    sender: mpsc::Sender<io::Result<RuntimeInputEvent>>,
    paused: Arc<AtomicBool>,
) {
    let mut buffer = Vec::<u8>::new();
    let mut parser = parser::Parser::default();
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

#[cfg(unix)]
fn resize_loop(sender: mpsc::Sender<io::Result<RuntimeInputEvent>>, paused: Arc<AtomicBool>) {
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
            .send(Ok(RuntimeInputEvent::Terminal(Event::Resize(
                size.0, size.1,
            ))))
            .is_err()
        {
            break;
        }
    }
}

#[cfg(unix)]
fn parse_buffer(
    parser_state: &mut parser::Parser,
    buffer: &mut Vec<u8>,
    sender: &mpsc::Sender<io::Result<RuntimeInputEvent>>,
    input_available: bool,
) {
    loop {
        if buffer.is_empty() {
            return;
        }
        let len_before = buffer.len();
        match parser::parse_event(parser_state, buffer, input_available) {
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

#[cfg(unix)]
fn is_unsupported_input_sequence(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Other && error.to_string() == "Could not parse an event."
}

#[cfg(unix)]
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn unsupported_mouse_button_sequence_is_ignored() {
        let (sender, receiver) = mpsc::channel();
        let mut parser = parser::Parser::default();
        let mut buffer = b"\x1b[<128;10;5M".to_vec();

        parse_buffer(&mut parser, &mut buffer, &sender, false);

        assert!(buffer.is_empty());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn parse_errors_from_real_io_are_still_errors() {
        let error = io::Error::other("different failure");
        assert!(!is_unsupported_input_sequence(&error));
    }

    #[test]
    fn parse_buffer_keeps_concatenated_kitty_dnd_events() {
        let (sender, receiver) = mpsc::channel();
        let mut parser = parser::Parser::default();
        let mut buffer = b"\x1b]72;t=o:x=1:y=2\x1b\\\x1b]72;t=e:x=4:y=1\x1b\\".to_vec();

        parse_buffer(&mut parser, &mut buffer, &sender, false);

        assert!(buffer.is_empty());
        assert_eq!(
            receiver.try_recv().unwrap().unwrap(),
            RuntimeInputEvent::KittyDnd(crate::runtime::kitty_dnd::KittyDndEvent::DragOffer {
                x: 1,
                y: 2,
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap().unwrap(),
            RuntimeInputEvent::KittyDnd(crate::runtime::kitty_dnd::KittyDndEvent::DragEnded {
                cancelled: true,
            })
        );
        assert!(receiver.try_recv().is_err());
    }
}
