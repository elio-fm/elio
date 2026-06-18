use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
    event::{
        self, DisableFocusChange, EnableFocusChange, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};
use std::{
    io::{self, ErrorKind, Write},
    sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError},
    thread,
};

/// The TUI terminal, backed by [`ThreadedWriter`] so frame output never blocks
/// the event loop.
pub(super) type AppTerminal = Terminal<CrosstermBackend<ThreadedWriter>>;

/// State shared between the event-loop side of [`ThreadedWriter`] and its
/// background thread. `buf` accumulates flushed frames; the writer thread swaps
/// it out and performs the blocking write outside the lock. `idle` is true while
/// the writer thread is parked with nothing queued and nothing mid-write — the
/// condition drain barriers wait for.
struct WriterShared {
    buf: Vec<u8>,
    idle: bool,
    dead: bool,
    shutdown: bool,
}

struct WriterChannel {
    state: Mutex<WriterShared>,
    cond: Condvar,
}

impl WriterChannel {
    fn lock(&self) -> MutexGuard<'_, WriterShared> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A [`Write`] that hands each flushed frame to a background thread which
/// performs the blocking write to stdout.
///
/// The TUI event loop must never block on terminal output. A single large frame
/// — above all a multi-megabyte Kitty image payload — written synchronously
/// stalls the entire loop (no input, no resize handling) for as long as the
/// terminal/tmux takes to drain it. Through tmux that drain backs up whenever
/// the outer terminal is busy (e.g. repainting after a resize), which froze the
/// UI for seconds. Flushes append to a shared buffer under a mutex — never
/// waiting on the terminal — so the loop stays responsive and the visible output
/// catches up a beat later. The buffer is unbounded by design: frames are only
/// produced on events/timers, so even a stalled terminal accumulates output far
/// slower than memory matters, and bounding it would reintroduce the very
/// backpressure stall this exists to remove.
pub(super) struct ThreadedWriter {
    channel: Arc<WriterChannel>,
    pending: Vec<u8>,
}

impl ThreadedWriter {
    fn new() -> Self {
        let channel = Arc::new(WriterChannel {
            state: Mutex::new(WriterShared {
                buf: Vec::new(),
                idle: true,
                dead: false,
                shutdown: false,
            }),
            cond: Condvar::new(),
        });
        let writer_channel = Arc::clone(&channel);
        thread::spawn(move || {
            let mut out = io::stdout();
            let mut batch = Vec::new();
            loop {
                {
                    let mut state = writer_channel.lock();
                    while state.buf.is_empty() && !state.shutdown {
                        state.idle = true;
                        writer_channel.cond.notify_all();
                        state = writer_channel
                            .cond
                            .wait(state)
                            .unwrap_or_else(PoisonError::into_inner);
                    }
                    if state.buf.is_empty() {
                        state.idle = true;
                        writer_channel.cond.notify_all();
                        return;
                    }
                    state.idle = false;
                    std::mem::swap(&mut state.buf, &mut batch);
                }
                if out.write_all(&batch).is_err() || out.flush().is_err() {
                    let mut state = writer_channel.lock();
                    state.dead = true;
                    state.idle = true;
                    state.buf.clear();
                    writer_channel.cond.notify_all();
                    return;
                }
                batch.clear();
            }
        });
        Self {
            channel,
            pending: Vec::new(),
        }
    }

    /// A cheap handle for waiting on the writer thread from elsewhere (the
    /// suspend/resume paths) without reaching into the unstable backend writer
    /// accessor.
    fn drainer(&self) -> Drainer {
        Drainer {
            channel: Arc::clone(&self.channel),
        }
    }

    /// Blocks until the writer thread has written everything queued so far. Used
    /// on drop so no buffered output is lost at exit.
    fn drain(&mut self) {
        let _ = self.flush();
        self.drainer().drain();
    }
}

/// Blocks until the [`ThreadedWriter`] has flushed everything queued before it.
/// Callers must flush the backend first so any per-frame buffered bytes are in
/// the queue; this then waits for the writer thread to drain the queue. Used
/// before handing the real terminal to a child process (suspend) and before the
/// keyboard-enhancement probe (resume), which both need output fully on screen.
pub(super) struct Drainer {
    channel: Arc<WriterChannel>,
}

impl Drainer {
    fn drain(&self) {
        let mut state = self.channel.lock();
        while !(state.dead || state.buf.is_empty() && state.idle) {
            state = self
                .channel
                .cond
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

impl Write for ThreadedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut state = self.channel.lock();
        if state.dead {
            // The writer thread hit a write error; the process is tearing down
            // and there is nothing useful to do with the bytes.
            self.pending.clear();
            return Ok(());
        }
        state.buf.append(&mut self.pending);
        self.channel.cond.notify_all();
        Ok(())
    }
}

impl Drop for ThreadedWriter {
    fn drop(&mut self) {
        self.drain();
        let mut state = self.channel.lock();
        state.shutdown = true;
        self.channel.cond.notify_all();
    }
}

pub(super) fn init_terminal() -> Result<(AppTerminal, Drainer)> {
    match try_init_terminal() {
        Ok(pair) => Ok(pair),
        Err(error) => {
            let _ = cleanup_terminal_state();
            Err(error)
        }
    }
}

fn try_init_terminal() -> Result<(AppTerminal, Drainer)> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        event::EnableMouseCapture,
        EnableFocusChange
    )?;

    // Force mouse tracking modes explicitly after EnableMouseCapture. Crossterm should
    // already send these, but some terminals require an explicit flush or are sensitive
    // to the exact byte sequence arriving in a single write.
    //   1000 = click tracking
    //   1002 = button-event tracking (drag with button held)
    //   1003 = any-event tracking (all motion, needed for hover-based scroll routing)
    //   1006 = SGR extended coordinates (required for columns > 223)
    write!(stdout, "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h")?;

    // Ask the terminal to forward Shift+mouse to the app instead of using it for text
    // selection. Ghostty and some xterm-compatible terminals honor XTSHIFTESCAPE.
    // Terminals that don't support it ignore this silently.
    write!(stdout, "\x1b[>4;1m")?;

    stdout.flush()?;
    push_keyboard_enhancement_if_supported(&mut stdout)?;

    // The startup escapes above go straight to stdout (the backend doesn't exist
    // yet and they're already flushed). From here on, all frame output flows
    // through the background writer so the event loop never blocks on it.
    let writer = ThreadedWriter::new();
    let drainer = writer.drainer();
    let backend = CrosstermBackend::new(writer);
    let mut terminal = Terminal::new(backend)?;
    clear_for_full_repaint(&mut terminal)?;
    terminal.hide_cursor()?;
    Ok((terminal, drainer))
}

/// Clears the alternate screen and forces a full repaint on the next draw,
/// without ratatui's [`Terminal::clear`]. On a Fullscreen viewport `clear`
/// issues a blocking CSI 6n cursor-position query (to snapshot and restore the
/// cursor) which, under tmux, can stall for up to crossterm's 2-second timeout
/// whenever the terminal's report is delayed or swallowed — most visibly during
/// a resize and on every return from zoxide / a shell / Open With, where it
/// reads as a multi-second freeze. [`Terminal::resize`] performs the same
/// clear-region(all) plus back-buffer reset for a Fullscreen viewport but never
/// queries the cursor; elio repositions the cursor every frame, so the
/// save/restore `clear` does is unnecessary.
pub(super) fn clear_for_full_repaint(terminal: &mut AppTerminal) -> io::Result<()> {
    let size = terminal.size()?;
    terminal.resize(Rect::new(0, 0, size.width, size.height))
}

/// Temporarily tears down the TUI so a blocking terminal app can use stdout.
/// Call [`resume_terminal`] afterwards to restore the TUI.
pub(super) fn suspend_terminal(
    terminal: &mut AppTerminal,
    drainer: &Drainer,
    leave_alternate: bool,
) -> Result<()> {
    let backend = terminal.backend_mut();
    write!(backend, "\x1b[>4;0m")?;
    write!(backend, "\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l")?;
    backend.flush()?;
    pop_keyboard_enhancement_if_supported(terminal.backend_mut())?;
    execute!(
        terminal.backend_mut(),
        event::DisableMouseCapture,
        DisableFocusChange,
        SetCursorStyle::DefaultUserShape
    )?;
    if leave_alternate {
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    } else {
        clear_for_full_repaint(terminal)?;
    }
    disable_raw_mode()?;
    terminal.show_cursor()?;
    // The child process is about to take over the real terminal; make sure every
    // queued byte has actually been written before we hand it off.
    let _ = terminal.backend_mut().flush();
    drainer.drain();
    Ok(())
}

/// Restores the TUI after [`suspend_terminal`].  Forces a full redraw on the
/// next render cycle so no stale content is left on screen.
pub(super) fn resume_terminal(terminal: &mut AppTerminal, drainer: &Drainer) -> Result<()> {
    enable_raw_mode()?;
    {
        // Route restore escapes through the backend so they stay ordered with
        // any frame output, then drain before the keyboard-enhancement probe,
        // which talks to the terminal directly (stdin/stdout) and must not race
        // the background writer.
        let backend = terminal.backend_mut();
        execute!(
            backend,
            EnterAlternateScreen,
            event::EnableMouseCapture,
            EnableFocusChange,
        )?;
        write!(backend, "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h")?;
        write!(backend, "\x1b[>4;1m")?;
        backend.flush()?;
    }
    drainer.drain();
    push_keyboard_enhancement_if_supported(terminal.backend_mut())?;
    clear_for_full_repaint(terminal)?;
    terminal.hide_cursor()?;
    Ok(())
}

pub(super) fn restore_terminal(terminal: &mut AppTerminal, drainer: &Drainer) -> Result<()> {
    // Disable in reverse order and do it before leaving the alternate screen so the
    // terminal processes the escape sequences while still in the right mode.
    let backend = terminal.backend_mut();
    write!(backend, "\x1b[>4;0m")?; // reset XTSHIFTESCAPE
    write!(backend, "\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l")?; // disable mouse modes
    backend.flush()?;
    pop_keyboard_enhancement_if_supported(terminal.backend_mut())?;
    execute!(
        terminal.backend_mut(),
        event::DisableMouseCapture,
        DisableFocusChange,
        SetCursorStyle::DefaultUserShape,
        LeaveAlternateScreen
    )?;
    disable_raw_mode()?;
    terminal.show_cursor()?;
    terminal.backend_mut().flush()?;
    drainer.drain();
    Ok(())
}

fn cleanup_terminal_state() -> io::Result<()> {
    let mut stdout = io::stdout();
    let _ = write!(stdout, "\x1b[>4;0m");
    let _ = write!(stdout, "\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l");
    let _ = stdout.flush();
    let _ = execute!(
        stdout,
        event::DisableMouseCapture,
        DisableFocusChange,
        SetCursorStyle::DefaultUserShape,
        LeaveAlternateScreen,
    );
    disable_raw_mode()?;
    Ok(())
}

fn push_keyboard_enhancement_if_supported<W: Write>(writer: &mut W) -> io::Result<()> {
    // The probe writes a query to the terminal and blocks on the reply with a
    // 2-second crossterm timeout. Support cannot change within a session, so
    // probe once and reuse the answer — this runs again on every resume from
    // zoxide/shell/Open With, where a swallowed reply read as a 2s freeze.
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    if !*SUPPORTED.get_or_init(|| matches!(supports_keyboard_enhancement(), Ok(true))) {
        return Ok(());
    }

    match execute!(
        writer,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        )
    ) {
        Ok(()) => Ok(()),
        Err(error) if keyboard_enhancement_is_unsupported(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

fn pop_keyboard_enhancement_if_supported<W: Write>(writer: &mut W) -> io::Result<()> {
    match execute!(writer, PopKeyboardEnhancementFlags) {
        Ok(()) => Ok(()),
        Err(error) if keyboard_enhancement_is_unsupported(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

fn keyboard_enhancement_is_unsupported(error: &io::Error) -> bool {
    error.kind() == ErrorKind::Unsupported
        && error
            .to_string()
            .contains("Keyboard progressive enhancement not implemented")
}

#[cfg(test)]
mod tests {
    use super::keyboard_enhancement_is_unsupported;
    use std::io;

    #[test]
    fn keyboard_enhancement_unsupported_detection_matches_crossterm_error() {
        let error = io::Error::new(
            io::ErrorKind::Unsupported,
            "Keyboard progressive enhancement not implemented for the legacy Windows API.",
        );

        assert!(keyboard_enhancement_is_unsupported(&error));
    }

    #[test]
    fn keyboard_enhancement_unsupported_detection_rejects_other_errors() {
        let error = io::Error::other("some other terminal error");

        assert!(!keyboard_enhancement_is_unsupported(&error));
    }
}
