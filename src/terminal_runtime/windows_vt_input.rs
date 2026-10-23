//! Windows VT input reader.
//!
//! Windows Terminal sends a terminal paste as ordinary input unless the
//! console is in VT-input mode. Reading that stream lets the same parser used
//! by the Unix raw reader turn bracketed paste into `Event::Paste`, while a
//! console which does not support VT input continues through crossterm.

use std::{
    io, ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
    time::Duration,
};

use crossterm::{event::Event, terminal};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Console::{
        CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_INPUT, GetConsoleMode, GetStdHandle, ReadConsoleW,
        STD_INPUT_HANDLE, SetConsoleMode,
    },
    System::Threading::WaitForSingleObject,
};

use super::{
    crossterm_parser::{self, InternalEvent},
    input_reader::InputEvent,
};

const RESIZE_POLL_INTERVAL: Duration = Duration::from_millis(16);
const PAUSED_POLL_INTERVAL: Duration = Duration::from_millis(10);
const READ_BUFFER_LEN: usize = 256;
const READ_WAIT_MILLIS: u32 = 2;

pub(in crate::terminal_runtime) struct InputReader {
    receiver: Receiver<io::Result<InputEvent>>,
    paused: Arc<AtomicBool>,
    console_mode: ConsoleInputMode,
}

impl InputReader {
    pub(in crate::terminal_runtime) fn spawn() -> io::Result<Self> {
        let console_mode = ConsoleInputMode::enable_vt_input()?;
        let (sender, receiver) = mpsc::channel();
        let paused = Arc::new(AtomicBool::new(false));

        let read_paused = Arc::clone(&paused);
        let read_sender = sender.clone();
        // HANDLE is a raw pointer and intentionally not Send. Its numeric
        // value remains valid for this process lifetime; reconstruct it only
        // inside the reader thread.
        let input = console_mode.handle as usize;
        thread::Builder::new()
            .name("elio-runtime-vt-input".to_string())
            .spawn(move || read_loop(input as HANDLE, read_sender, read_paused))
            .map_err(io::Error::other)?;

        let resize_paused = Arc::clone(&paused);
        thread::Builder::new()
            .name("elio-runtime-resize".to_string())
            .spawn(move || resize_loop(sender, resize_paused))
            .map_err(io::Error::other)?;

        Ok(Self {
            receiver,
            paused,
            console_mode,
        })
    }

    pub(in crate::terminal_runtime) fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
        // The TUI calls this immediately before disabling raw mode, and again
        // immediately after restoring it. Keep VT input scoped to the TUI so
        // child terminal programs receive the console mode they expect.
        let _ = self.console_mode.set_vt_input(!paused);
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
                "Windows VT input reader stopped",
            )),
        }
    }

    pub(in crate::terminal_runtime) fn try_recv(&self) -> io::Result<Option<InputEvent>> {
        match self.receiver.try_recv() {
            Ok(event) => event.map(Some),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Windows VT input reader stopped",
            )),
        }
    }
}

struct ConsoleInputMode {
    handle: HANDLE,
    raw_mode: CONSOLE_MODE,
}

impl ConsoleInputMode {
    fn enable_vt_input() -> io::Result<Self> {
        // SAFETY: GetStdHandle does not retain data and STD_INPUT_HANDLE is a
        // constant identifying the process's console input handle.
        let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let mut raw_mode = 0;
        // SAFETY: handle is the process console input handle and raw_mode is
        // writable for the duration of this call.
        if unsafe { GetConsoleMode(handle, &mut raw_mode) } == 0 {
            return Err(io::Error::last_os_error());
        }

        let reader = Self { handle, raw_mode };
        reader.set_vt_input(true)?;
        Ok(reader)
    }

    fn set_vt_input(&self, enabled: bool) -> io::Result<()> {
        let mode = if enabled {
            self.raw_mode | ENABLE_VIRTUAL_TERMINAL_INPUT
        } else {
            self.raw_mode
        };
        // SAFETY: handle is a live process console input handle. This changes
        // only the mode saved from crossterm's already-enabled raw mode.
        if unsafe { SetConsoleMode(self.handle, mode) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for ConsoleInputMode {
    fn drop(&mut self) {
        // crossterm restores the pre-Elio mode after this reader drops. First
        // restore its raw mode so the two state transitions remain paired.
        let _ = self.set_vt_input(false);
    }
}

fn read_loop(input: HANDLE, sender: mpsc::Sender<io::Result<InputEvent>>, paused: Arc<AtomicBool>) {
    let mut buffer = Vec::new();
    let mut pending_high_surrogate = None;
    let mut utf16 = [0u16; READ_BUFFER_LEN];

    loop {
        if paused.load(Ordering::Relaxed) {
            thread::sleep(PAUSED_POLL_INTERVAL);
            continue;
        }

        // Waiting with a short timeout gives a lone Escape the same delayed
        // disambiguation that crossterm's stream reader provides, without
        // blocking resize delivery or turning it into an Alt sequence.
        // SAFETY: input is the live process console input handle.
        match unsafe { WaitForSingleObject(input, READ_WAIT_MILLIS) } {
            WAIT_OBJECT_0 => {}
            WAIT_TIMEOUT => {
                parse_buffer(&mut buffer, &sender, false);
                continue;
            }
            _ => {
                let _ = sender.send(Err(io::Error::last_os_error()));
                break;
            }
        }

        let mut read = 0;
        // SAFETY: utf16 is writable for READ_BUFFER_LEN UTF-16 code units;
        // ReadConsoleW does not retain either pointer.
        if unsafe {
            ReadConsoleW(
                input,
                utf16.as_mut_ptr().cast(),
                READ_BUFFER_LEN as u32,
                &mut read,
                ptr::null(),
            )
        } == 0
        {
            let _ = sender.send(Err(io::Error::last_os_error()));
            break;
        }
        if read == 0 {
            continue;
        }

        for &unit in &utf16[..read as usize] {
            append_utf16_as_utf8(&mut buffer, &mut pending_high_surrogate, &[unit]);
            parse_buffer(&mut buffer, &sender, true);
        }
    }
}

fn resize_loop(sender: mpsc::Sender<io::Result<InputEvent>>, paused: Arc<AtomicBool>) {
    let mut last_size = terminal::size().ok();
    loop {
        thread::sleep(RESIZE_POLL_INTERVAL);
        if paused.load(Ordering::Relaxed) {
            last_size = terminal::size().ok();
            continue;
        }
        let Some(size) = terminal::size().ok() else {
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

fn append_utf16_as_utf8(buffer: &mut Vec<u8>, pending_high: &mut Option<u16>, input: &[u16]) {
    for &unit in input {
        if let Some(high) = pending_high.take() {
            if let Some(ch) = char::decode_utf16([high, unit]).next().and_then(Result::ok) {
                append_char(buffer, ch);
                continue;
            }
            append_char(buffer, char::REPLACEMENT_CHARACTER);
        }

        if (0xD800..=0xDBFF).contains(&unit) {
            *pending_high = Some(unit);
        } else if let Some(ch) = char::decode_utf16([unit]).next().and_then(Result::ok) {
            append_char(buffer, ch);
        } else {
            append_char(buffer, char::REPLACEMENT_CHARACTER);
        }
    }
}

fn append_char(buffer: &mut Vec<u8>, ch: char) {
    let mut encoded = [0; 4];
    buffer.extend_from_slice(ch.encode_utf8(&mut encoded).as_bytes());
}

fn parse_buffer(
    buffer: &mut Vec<u8>,
    sender: &mpsc::Sender<io::Result<InputEvent>>,
    input_available: bool,
) {
    loop {
        if buffer.is_empty() {
            return;
        }
        let len_before = buffer.len();
        match crossterm_parser::parse_event(buffer, input_available) {
            Ok(Some(InternalEvent::Event(event))) => {
                // The reader feeds this parser a single UTF-16 input unit at a
                // time, matching crossterm's stream parser contract. A parsed
                // event therefore consumes the complete buffered sequence.
                buffer.clear();
                let _ = sender.send(Ok(InputEvent::Terminal(event)));
            }
            Ok(Some(_)) => buffer.clear(),
            Ok(None) => return,
            Err(_) => {
                buffer.clear();
                return;
            }
        }
        debug_assert!(buffer.len() < len_before);
    }
}

#[cfg(test)]
#[path = "tests/windows_vt_input.rs"]
mod tests;
