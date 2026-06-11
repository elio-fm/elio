mod app;
mod config;
mod core;
mod file_info;
mod fs;
mod path_display;
mod preview;
mod shell;
mod ui;
mod zoxide;

use crate::app::{App, ChooserExit, PendingTerminalTask};
use anyhow::Result;
use crossterm::{
    cursor::{RestorePosition, SavePosition, SetCursorStyle},
    event::{
        self, DisableFocusChange, EnableFocusChange, Event, KeyboardEnhancementFlags, MouseEvent,
        MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen,
        disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement,
    },
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    buffer::{Buffer, Cell, CellDiffOption},
    layout::Rect,
};
use std::{
    fs as std_fs,
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError},
    thread,
    time::{Duration, Instant},
};

const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const ACTIVE_SCROLL_POLL_INTERVAL: Duration = Duration::from_millis(12);
const WINDOWS_TERMINAL_ACTIVE_POLL_INTERVAL: Duration = Duration::from_millis(24);
const RELATIVE_TIME_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub struct RunOptions {
    pub start_dir: Option<PathBuf>,
    pub cwd_file: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum RunOutcome {
    Success,
    Cancelled,
}

#[derive(Debug)]
struct AppExit {
    final_cwd: Option<PathBuf>,
    chooser: Option<ChooserExit>,
}

pub fn run() -> Result<()> {
    run_with_options(RunOptions::default())
}

pub fn run_at(cwd: PathBuf) -> Result<()> {
    run_with_options(RunOptions {
        start_dir: Some(cwd),
        cwd_file: None,
    })
}

pub fn run_with_options(options: RunOptions) -> Result<()> {
    run_with_startup_state(options, None, false, None).map(|_| ())
}

#[doc(hidden)]
pub fn run_with_startup_options(
    options: RunOptions,
    start_focus: Option<PathBuf>,
    reveal_hidden_start_focus: bool,
    chooser_file: Option<PathBuf>,
) -> Result<RunOutcome> {
    run_with_startup_state(
        options,
        start_focus,
        reveal_hidden_start_focus,
        chooser_file,
    )
}

fn run_with_startup_state(
    options: RunOptions,
    start_focus: Option<PathBuf>,
    reveal_hidden_start_focus: bool,
    chooser_file: Option<PathBuf>,
) -> Result<RunOutcome> {
    let RunOptions {
        start_dir,
        cwd_file,
    } = options;
    config::initialize();
    ui::theme::initialize();
    let (mut terminal, drainer) = init_terminal()?;
    let result = run_app(
        &mut terminal,
        &drainer,
        start_dir,
        start_focus,
        reveal_hidden_start_focus,
        chooser_file.is_some(),
    );
    restore_terminal(&mut terminal)?;
    let app_exit = result?;
    if let Some(final_cwd) = app_exit.final_cwd {
        write_cwd_file_if_requested(cwd_file.as_deref(), &final_cwd)?;
    }
    match app_exit.chooser {
        Some(ChooserExit::Confirmed(paths)) => {
            write_chooser_file_if_requested(chooser_file.as_deref(), &paths)?;
            Ok(RunOutcome::Success)
        }
        Some(ChooserExit::Cancelled) => Ok(RunOutcome::Cancelled),
        None => Ok(RunOutcome::Success),
    }
}

/// The TUI terminal, backed by [`ThreadedWriter`] so frame output never blocks
/// the event loop.
type AppTerminal = Terminal<CrosstermBackend<ThreadedWriter>>;

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
struct ThreadedWriter {
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
struct Drainer {
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

fn init_terminal() -> Result<(AppTerminal, Drainer)> {
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
fn clear_for_full_repaint(terminal: &mut AppTerminal) -> io::Result<()> {
    let size = terminal.size()?;
    terminal.resize(Rect::new(0, 0, size.width, size.height))
}

/// Temporarily tears down the TUI so a blocking terminal app can use stdout.
/// Call [`resume_terminal`] afterwards to restore the TUI.
fn suspend_terminal(
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
fn resume_terminal(terminal: &mut AppTerminal, drainer: &Drainer) -> Result<()> {
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

/// Runs `program args` blocking in the current terminal, inheriting
/// stdin/stdout/stderr.  Errors are ignored — a broken command (e.g. nvim
/// unable to open a file) should not crash the file manager.
fn run_blocking_in_terminal(program: &str, args: &[String]) {
    let _ = Command::new(program).args(args).status();
}

fn refresh_after_shell(app: &mut App, cwd: &Path) {
    let cwd_label = path_display::user_facing(cwd);
    match cwd.try_exists() {
        Ok(true) => {
            if let Err(error) = app.reload() {
                app.report_runtime_error("Shell refresh failed", &error);
            }
        }
        Ok(false) => app.set_status_message(format!(
            "Current folder was removed while shell was open: {}",
            cwd_label
        )),
        Err(error) => app.set_status_message(format!(
            "Could not refresh {cwd_label} after shell: {error}"
        )),
    }
}

fn apply_zoxide_query_result(app: &mut App, result: zoxide::QueryResult) {
    match result {
        zoxide::QueryResult::Selected(path) => app.open_zoxide_selection(path),
        zoxide::QueryResult::Cancelled => {}
        zoxide::QueryResult::NotFound => app.set_status_message("zoxide not found"),
        zoxide::QueryResult::PickerNotFound => app.set_status_message("fzf not found"),
        zoxide::QueryResult::Empty => app.set_status_message("No zoxide directory history found"),
        zoxide::QueryResult::OnlyCurrentDirectory => {
            app.set_status_message("Zoxide history only contains the current directory")
        }
        zoxide::QueryResult::LaunchFailed => app.set_status_message("Could not run zoxide"),
    }
}

fn restore_terminal(terminal: &mut AppTerminal) -> Result<()> {
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
    // Remaining queued bytes (these restore writes included) are flushed when the
    // terminal — and with it the ThreadedWriter — is dropped right after run()
    // returns; ThreadedWriter::drop drains the writer thread before exit.
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

fn write_cwd_file_if_requested(cwd_file: Option<&Path>, final_cwd: &Path) -> Result<()> {
    let Some(cwd_file) = cwd_file else {
        return Ok(());
    };

    write_cwd_file(cwd_file, final_cwd)
}

#[cfg(unix)]
fn write_cwd_file(cwd_file: &Path, final_cwd: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;

    std_fs::write(cwd_file, final_cwd.as_os_str().as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_cwd_file(cwd_file: &Path, final_cwd: &Path) -> Result<()> {
    std_fs::write(cwd_file, final_cwd.to_string_lossy().as_bytes())?;
    Ok(())
}

fn write_chooser_file_if_requested(chooser_file: Option<&Path>, paths: &[PathBuf]) -> Result<()> {
    let Some(chooser_file) = chooser_file else {
        return Ok(());
    };

    write_chooser_file(chooser_file, paths)
}

fn write_chooser_file(chooser_file: &Path, paths: &[PathBuf]) -> Result<()> {
    let bytes = chooser_output_bytes(paths);
    if chooser_file_is_stdout(chooser_file) {
        let mut stdout = io::stdout().lock();
        stdout.write_all(&bytes)?;
        stdout.flush()?;
    } else {
        std_fs::write(chooser_file, bytes)?;
    }
    Ok(())
}

fn chooser_file_is_stdout(chooser_file: &Path) -> bool {
    chooser_file == Path::new("-")
}

#[cfg(unix)]
fn chooser_output_bytes(paths: &[PathBuf]) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(path.as_os_str().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

#[cfg(not(unix))]
fn chooser_output_bytes(paths: &[PathBuf]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(path.to_string_lossy().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

fn run_app(
    terminal: &mut AppTerminal,
    drainer: &Drainer,
    cwd: Option<PathBuf>,
    start_focus: Option<PathBuf>,
    reveal_hidden_start_focus: bool,
    chooser_enabled: bool,
) -> Result<AppExit> {
    let mut app = match cwd {
        Some(cwd) => App::new_at_startup(cwd, start_focus, reveal_hidden_start_focus)?,
        None => App::new()?,
    };
    if chooser_enabled {
        app.enable_chooser_mode();
    }

    // Enable terminal image previews. Detection handles the current policy:
    // Kitty, Ghostty, Warp, WezTerm, iTerm2, and Konsole auto-enable supported
    // image protocols;
    // ELIO_IMAGE_PREVIEWS=1 force-enables Kitty graphics on otherwise unrecognized terminals.
    // All image bytes are routed through terminal.backend_mut() so they never bypass
    // crossterm and cannot corrupt mouse reporting.
    app.enable_terminal_image_previews();

    let mut dirty = true;
    let mut search_cursor_active = false;
    let mut terminal_focused = true;
    let mut last_relative_time_refresh_at = Instant::now();

    loop {
        if app.should_quit {
            break;
        }

        if terminal_focused
            && last_relative_time_refresh_at.elapsed() >= RELATIVE_TIME_REFRESH_INTERVAL
        {
            dirty = true;
            last_relative_time_refresh_at = Instant::now();
        }

        // Background work and drawing run regardless of focus so a still-visible
        // pane stays live. tmux delivers FocusLost to a pane the moment it stops
        // being the active pane (verified: switching panes — not just windows —
        // sends \e[O), and again when another GUI app steals the outer terminal's
        // focus. In a tiled tmux layout that is most of the time, so gating these on
        // focus made elio appear frozen — in-flight previews, directory loads, and
        // filesystem changes never landed until focus returned. Drawing stays cheap
        // when idle because it only runs when `dirty`, which is set solely by real
        // state changes; an unfocused, idle pane sets nothing dirty and never draws.
        // These all run together so the deferred-refresh coordination (see #64) that
        // process_background_jobs shares with the directory timers cannot desync.
        // Only the cosmetic per-second relative-time tick (above) and the poll
        // cadence below stay focus-dependent.
        if app.process_background_jobs() {
            dirty = true;
        }

        if app.process_pdf_preview_timers() {
            dirty = true;
        }

        if app.process_pending_scroll() {
            dirty = true;
        }

        if app.process_preview_refresh_timers() {
            dirty = true;
        }

        if app.process_preview_prefetch_timers() {
            dirty = true;
        }

        if app.process_directory_stats_timer() {
            dirty = true;
        }

        if app.process_directory_item_count_timer() {
            dirty = true;
        }

        if app.process_browser_wheel_timers() {
            dirty = true;
        }

        if app.process_image_preview_timers() {
            dirty = true;
        }

        if app.process_terminal_image_resize_settle_timer() {
            dirty = true;
        }

        if app.process_sidebar_refresh() {
            dirty = true;
        }

        match app.process_auto_reload() {
            Ok(changed) => {
                dirty |= changed;
            }
            Err(error) => {
                app.report_runtime_error("Auto-reload failed", &error);
                dirty = true;
            }
        }

        if dirty {
            dirty = draw_terminal_frame(terminal, &mut app)?;
        }

        let wants_search_cursor = app.search_is_open()
            || app.create_is_open()
            || app.rename_is_open()
            || app.bulk_rename_is_open();
        if wants_search_cursor != search_cursor_active {
            if wants_search_cursor {
                terminal.show_cursor()?;
            } else {
                terminal.hide_cursor()?;
            }
            execute!(
                terminal.backend_mut(),
                if wants_search_cursor {
                    SetCursorStyle::SteadyBar
                } else {
                    SetCursorStyle::DefaultUserShape
                }
            )?;
            search_cursor_active = wants_search_cursor;
        }

        let base_poll_interval = if !terminal_focused {
            IDLE_POLL_INTERVAL
        } else if app.has_pending_scroll()
            || app.has_pending_auto_reload()
            || app.has_pending_background_work()
        {
            if app.is_windows_terminal() {
                WINDOWS_TERMINAL_ACTIVE_POLL_INTERVAL
            } else {
                ACTIVE_SCROLL_POLL_INTERVAL
            }
        } else {
            IDLE_POLL_INTERVAL
        };
        let poll_interval = event_poll_interval(
            base_poll_interval,
            terminal_focused,
            [
                app.pending_pdf_preview_timer(),
                app.pending_image_preview_timer(),
                app.pending_terminal_image_resize_settle_timer(),
                app.pending_preview_refresh_timer(),
                app.pending_preview_prefetch_timer(),
                app.pending_directory_stats_timer(),
                app.pending_directory_item_count_timer(),
                app.pending_browser_wheel_timer(),
            ],
        );

        if event::poll(poll_interval)? {
            // Batch all immediately-available events into one render cycle.
            // This prevents lag when events (especially scroll events from high-frequency
            // terminals) arrive faster than the app can render: instead of one render per
            // event we accumulate all queued events first and render the final state once.
            loop {
                let event = event::read()?;
                if std::env::var_os("ELIO_LOG_MOUSE").is_some()
                    && let Event::Mouse(m) = &event
                {
                    let _ = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(std::env::temp_dir().join("elio-mouse.log"))
                        .and_then(|mut f| {
                            writeln!(f, "{:?} col={} row={}", m.kind, m.column, m.row)
                        });
                }
                if matches!(event, Event::FocusLost) {
                    terminal_focused = false;
                } else if matches!(event, Event::FocusGained) {
                    terminal_focused = true;
                    app.handle_terminal_image_focus_gained();
                    dirty = true;
                } else if matches!(event, Event::Resize(_, _)) {
                    app.handle_terminal_image_resize();
                    dirty = true;
                } else {
                    // Input doubles as an implicit FocusGained; see
                    // event_implies_terminal_focus for why the real one can be dropped.
                    if !terminal_focused && event_implies_terminal_focus(&event) {
                        terminal_focused = true;
                        app.handle_terminal_image_focus_gained();
                        dirty = true;
                    }
                    // Mouse move events only update the hover/target state — nothing
                    // visual changes, so they don't need a re-render. Skipping dirty here
                    // avoids the constant re-render storm that ?1003h (any-event tracking)
                    // causes in terminals like Alacritty, Ghostty, and Gnome Terminal.
                    let needs_render = !matches!(
                        event,
                        Event::Mouse(MouseEvent {
                            kind: MouseEventKind::Moved,
                            ..
                        })
                    );
                    let _ = app.handle_event(event);
                    if needs_render && terminal_focused {
                        dirty = true;
                    }
                }
                // Stop batching once there are no more immediately available events.
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }

            if app.should_quit {
                break;
            }

            // A terminal task (e.g. nvim from Open With, or zoxide) needs the real terminal.
            // Suspend the TUI, run the task blocking, then restore.
            if let Some(task) = app.pending_terminal_task.take() {
                let zoxide_result = match task {
                    PendingTerminalTask::Command { program, args } => {
                        suspend_terminal(terminal, drainer, true)?;
                        run_blocking_in_terminal(&program, &args);
                        resume_terminal(terminal, drainer)?;
                        None
                    }
                    PendingTerminalTask::Shell { cwd } => {
                        suspend_terminal(terminal, drainer, true)?;
                        let shell_result = shell::run_in_current_terminal(&cwd);
                        resume_terminal(terminal, drainer)?;
                        match shell_result {
                            Ok(()) => refresh_after_shell(&mut app, &cwd),
                            Err(error) => app.set_status_message(error),
                        }
                        None
                    }
                    PendingTerminalTask::Zoxide => {
                        let cwd = app.navigation.cwd.clone();
                        if let Some(result) = zoxide::preflight(&cwd) {
                            Some(result)
                        } else {
                            suspend_terminal(terminal, drainer, false)?;
                            let result = zoxide::run_query_in_terminal(&cwd);
                            resume_terminal(terminal, drainer)?;
                            Some(result)
                        }
                    }
                };
                if let Some(result) = zoxide_result {
                    apply_zoxide_query_result(&mut app, result);
                }
                dirty = true;
            }
        }
    }

    let final_cwd = app
        .should_change_directory_on_quit
        .then(|| app.navigation.cwd.clone());
    let chooser = app.take_chooser_exit();
    app.queue_forced_iterm_preview_erase();
    let mut overlay_bytes = app.clear_preview_overlay()?;
    overlay_bytes.extend(app.iterm_pre_draw_erase());
    if !overlay_bytes.is_empty() {
        terminal.backend_mut().write_all(&overlay_bytes)?;
        terminal.backend_mut().flush()?;
    }
    Ok(AppExit { final_cwd, chooser })
}

fn draw_terminal_frame(terminal: &mut AppTerminal, app: &mut App) -> Result<bool> {
    execute!(terminal.backend_mut(), BeginSynchronizedUpdate)?;

    let draw_result = (|| -> Result<bool> {
        if app.take_pending_resize_clear() {
            clear_for_full_repaint(terminal)?;
        }

        // Erase stale image cells before terminal.draw() so ratatui can
        // overpaint them with the correct panel background in the same pass.
        // - iTerm2: images are drawn at pixel level; erasing prevents ghost pixels.
        // - Kitty unicode placeholder: placeholder chars are terminal cells;
        //   ratatui's differential renderer skips "unchanged" cells leaving
        //   stale image content visible after navigation or resize.
        let pre_erase = app.iterm_pre_draw_erase();
        let kitty_erase = app.kitty_pre_draw_erase();
        if !pre_erase.is_empty() || !kitty_erase.is_empty() {
            terminal.backend_mut().write_all(&pre_erase)?;
            terminal.backend_mut().write_all(&kitty_erase)?;
        }
        let mut frame_state = app::FrameState::default();
        let (
            dirty,
            image_behind_modal,
            sixel_collision_erase,
            popup_restore,
            modal_erase,
            skip_overlay_present,
        ) = {
            let completed = terminal.draw(|frame| ui::render(frame, app, &mut frame_state))?;
            let dirty = app.set_frame_state(frame_state);
            let modal_rects = app.collect_popup_rects();
            if !app.browser_wheel_burst_active()
                && app.should_repaint_iterm_inline_under_modal(&modal_rects)
            {
                let image_behind_modal = app.present_preview_overlay_behind_modal()?;
                let popup_restore = collect_buffer_cells(&modal_rects, completed.buffer);
                let modal_erase = app.modal_image_post_draw_erase(&modal_rects, completed.buffer);
                (
                    dirty,
                    image_behind_modal,
                    Vec::new(),
                    popup_restore,
                    modal_erase,
                    true,
                )
            } else if !app.browser_wheel_burst_active()
                && app.should_repaint_sixel_under_modal(&modal_rects)
            {
                let image_behind_modal = app.present_preview_overlay_behind_modal()?;
                let (sixel_collision_rects, sixel_collision_erase) =
                    app.sixel_modal_collision_erase(&modal_rects);
                let popup_restore = collect_buffer_cells(&sixel_collision_rects, completed.buffer);
                let modal_erase = app.modal_image_post_draw_erase(&modal_rects, completed.buffer);
                (
                    dirty,
                    image_behind_modal,
                    sixel_collision_erase,
                    popup_restore,
                    modal_erase,
                    true,
                )
            } else {
                let (sixel_collision_rects, sixel_collision_erase) =
                    app.sixel_modal_collision_erase(&modal_rects);
                let popup_restore = collect_buffer_cells(&sixel_collision_rects, completed.buffer);
                let modal_erase = app.modal_image_post_draw_erase(&modal_rects, completed.buffer);
                (
                    dirty,
                    Vec::new(),
                    sixel_collision_erase,
                    popup_restore,
                    modal_erase,
                    false,
                )
            }
        };
        write_bytes_preserving_cursor(terminal.backend_mut(), &image_behind_modal)?;
        write_bytes_preserving_cursor(terminal.backend_mut(), &sixel_collision_erase)?;
        draw_cells_preserving_cursor(terminal.backend_mut(), &popup_restore)?;
        write_bytes_preserving_cursor(terminal.backend_mut(), &modal_erase)?;
        if !skip_overlay_present && !app.browser_wheel_burst_active() {
            let overlay_bytes = app.present_preview_overlay()?;
            write_bytes_preserving_cursor(terminal.backend_mut(), &overlay_bytes)?;
        }
        terminal.backend_mut().flush()?;
        Ok(dirty)
    })();

    let end_result = execute!(terminal.backend_mut(), EndSynchronizedUpdate);
    match (draw_result, end_result) {
        (Ok(dirty), Ok(())) => Ok(dirty),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
        (Err(error), Err(_)) => Err(error),
    }
}

fn write_bytes_preserving_cursor<W: Write>(writer: &mut W, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    execute!(writer, SavePosition)?;
    writer.write_all(bytes)?;
    execute!(writer, RestorePosition)?;
    Ok(())
}

fn draw_cells_preserving_cursor<W: Write>(
    backend: &mut CrosstermBackend<W>,
    cells: &[(u16, u16, Cell)],
) -> io::Result<()> {
    if cells.is_empty() {
        return Ok(());
    }
    execute!(backend, SavePosition)?;
    ratatui::backend::Backend::draw(backend, cells.iter().map(|(x, y, cell)| (*x, *y, cell)))?;
    execute!(backend, RestorePosition)?;
    Ok(())
}

fn collect_buffer_cells(rects: &[Rect], buffer: &Buffer) -> Vec<(u16, u16, Cell)> {
    let bounds = *buffer.area();
    let mut cells = Vec::new();
    for rect in rects {
        let Some(area) = intersect_rect(*rect, bounds) else {
            continue;
        };
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                let Some(cell) = buffer.cell((x, y)) else {
                    continue;
                };
                if matches!(cell.diff_option, CellDiffOption::Skip) {
                    continue;
                }
                cells.push((x, y, cell.clone()));
            }
        }
    }
    cells
}

fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = a.x.saturating_add(a.width).min(b.x.saturating_add(b.width));
    let y2 =
        a.y.saturating_add(a.height)
            .min(b.y.saturating_add(b.height));
    (x2 > x1 && y2 > y1).then_some(Rect {
        x: x1,
        y: y1,
        width: x2.saturating_sub(x1),
        height: y2.saturating_sub(y1),
    })
}

/// Whether receiving `event` implies the terminal is focused. Key, mouse, and
/// paste events can only originate from a focused terminal, so they double as an
/// implicit FocusGained — the recovery path for terminals (notably on
/// Wayland/Hyprland) that drop the real FocusGained after a spawned GUI app
/// returns focus. Focus and resize events carry no such implication.
fn event_implies_terminal_focus(event: &Event) -> bool {
    matches!(event, Event::Key(_) | Event::Mouse(_) | Event::Paste(_))
}

fn event_poll_interval<I>(
    base_poll_interval: Duration,
    terminal_focused: bool,
    timers: I,
) -> Duration
where
    I: IntoIterator<Item = Option<Duration>>,
{
    if !terminal_focused {
        return base_poll_interval;
    }

    timers
        .into_iter()
        .flatten()
        .min()
        .map(|delay| delay.min(base_poll_interval))
        .unwrap_or(base_poll_interval)
}

#[cfg(test)]
mod tests {
    use crate::{
        ACTIVE_SCROLL_POLL_INTERVAL, IDLE_POLL_INTERVAL, collect_buffer_cells, event_poll_interval,
    };
    use crossterm::event::Event;
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::{Color, Modifier, Style},
    };
    use std::{
        fs, io,
        path::{Path, PathBuf},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-lib-{label}-{unique}"))
    }

    #[test]
    fn cwd_file_is_not_written_when_absent() {
        crate::write_cwd_file_if_requested(None, Path::new("/tmp"))
            .expect("absent cwd file should be a no-op");
    }

    #[test]
    fn cwd_file_writes_path_without_trailing_newline() {
        let root = temp_path("cwd-file");
        fs::create_dir_all(&root).expect("temp directory should be created");
        let cwd_file = root.join("cwd");
        let final_cwd = root.join("nested");
        fs::create_dir_all(&final_cwd).expect("nested temp directory should be created");

        crate::write_cwd_file_if_requested(Some(&cwd_file), &final_cwd)
            .expect("cwd file should be written");

        let bytes = fs::read(&cwd_file).expect("cwd file should be readable");
        assert!(!bytes.ends_with(b"\n"));
        assert_eq!(String::from_utf8_lossy(&bytes), final_cwd.to_string_lossy());

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn chooser_file_is_not_written_when_absent() {
        crate::write_chooser_file_if_requested(None, &[PathBuf::from("/tmp/example")])
            .expect("absent chooser file should be a no-op");
    }

    #[test]
    fn chooser_file_hyphen_targets_stdout() {
        assert!(crate::chooser_file_is_stdout(Path::new("-")));
        assert!(!crate::chooser_file_is_stdout(Path::new("./-")));
        assert!(!crate::chooser_file_is_stdout(Path::new("/dev/stdout")));
    }

    #[test]
    fn chooser_file_writes_paths_with_trailing_newline() {
        let root = temp_path("chooser-file");
        fs::create_dir_all(&root).expect("temp directory should be created");
        let chooser_file = root.join("selection");
        let alpha = root.join("alpha.txt");
        let beta = root.join("beta.txt");

        crate::write_chooser_file_if_requested(Some(&chooser_file), &[alpha.clone(), beta.clone()])
            .expect("chooser file should be written");

        let bytes = fs::read(&chooser_file).expect("chooser file should be readable");
        let expected = format!("{}\n{}\n", alpha.to_string_lossy(), beta.to_string_lossy());
        assert_eq!(String::from_utf8_lossy(&bytes), expected);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn chooser_file_truncates_on_empty_confirmation() {
        let root = temp_path("chooser-empty");
        fs::create_dir_all(&root).expect("temp directory should be created");
        let chooser_file = root.join("selection");
        fs::write(&chooser_file, "stale\n").expect("chooser file should be primed");

        crate::write_chooser_file_if_requested(Some(&chooser_file), &[])
            .expect("empty chooser confirmation should be written");

        let bytes = fs::read(&chooser_file).expect("chooser file should be readable");
        assert!(bytes.is_empty());

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn ratatui_diff_preserves_positions_beyond_u16_max_cells() {
        let area = Rect::new(0, 0, 400, 200);
        let previous = Buffer::empty(area);
        let mut next = Buffer::empty(area);
        next.set_string(123, 180, "X", Style::default());

        let diff = previous.diff(&next);

        assert!(
            diff.iter()
                .any(|(x, y, cell)| *x == 123 && *y == 180 && cell.symbol() == "X"),
            "expected diff to keep the changed cell at (123, 180), got: {:?}",
            diff.iter()
                .map(|(x, y, cell)| (*x, *y, cell.symbol().to_string()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn collect_buffer_cells_captures_popup_cells_with_styles() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 4));
        buffer.set_string(
            2,
            1,
            "OK",
            Style::default()
                .fg(Color::LightGreen)
                .bg(Color::Rgb(1, 2, 3))
                .add_modifier(Modifier::BOLD),
        );

        let cells = collect_buffer_cells(&[Rect::new(2, 1, 2, 1)], &buffer);

        assert_eq!(cells.len(), 2);
        assert_eq!((cells[0].0, cells[0].1, cells[0].2.symbol()), (2, 1, "O"));
        assert_eq!((cells[1].0, cells[1].1, cells[1].2.symbol()), (3, 1, "K"));
        assert_eq!(cells[0].2.fg, Color::LightGreen);
        assert_eq!(cells[0].2.bg, Color::Rgb(1, 2, 3));
        assert!(cells[0].2.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn event_poll_interval_stays_idle_while_terminal_is_unfocused() {
        let interval = event_poll_interval(
            IDLE_POLL_INTERVAL,
            false,
            [
                Some(Duration::from_millis(25)),
                Some(Duration::from_millis(10)),
            ],
        );

        assert_eq!(interval, IDLE_POLL_INTERVAL);
    }

    #[test]
    fn event_poll_interval_uses_pending_timer_when_terminal_is_focused() {
        let delay = Duration::from_millis(25);
        let interval = event_poll_interval(
            ACTIVE_SCROLL_POLL_INTERVAL,
            true,
            [None, Some(delay), Some(Duration::from_millis(50))],
        );

        assert!(interval <= delay);
    }

    #[test]
    fn input_events_imply_terminal_focus() {
        use crossterm::event::{
            KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
        };

        // Key, mouse, and paste events recover focus after a dropped FocusGained.
        assert!(crate::event_implies_terminal_focus(&Event::Key(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)
        )));
        assert!(crate::event_implies_terminal_focus(&Event::Mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            }
        )));
        assert!(crate::event_implies_terminal_focus(&Event::Paste(
            "text".to_string()
        )));
    }

    #[test]
    fn focus_and_resize_events_do_not_imply_terminal_focus() {
        // Focus/resize events carry no focus implication and must not trip the
        // recovery path (FocusLost in particular would be misread as focus).
        assert!(!crate::event_implies_terminal_focus(&Event::FocusLost));
        assert!(!crate::event_implies_terminal_focus(&Event::FocusGained));
        assert!(!crate::event_implies_terminal_focus(&Event::Resize(80, 24)));
    }

    #[test]
    fn keyboard_enhancement_unsupported_detection_matches_crossterm_error() {
        let error = io::Error::new(
            io::ErrorKind::Unsupported,
            "Keyboard progressive enhancement not implemented for the legacy Windows API.",
        );

        assert!(crate::keyboard_enhancement_is_unsupported(&error));
    }

    #[test]
    fn keyboard_enhancement_unsupported_detection_rejects_other_errors() {
        let error = io::Error::other("some other terminal error");

        assert!(!crate::keyboard_enhancement_is_unsupported(&error));
    }
}
