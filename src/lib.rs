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

use crate::app::{App, PendingTerminalTask};
use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
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
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    fs as std_fs,
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    process::Command,
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
    config::initialize();
    ui::theme::initialize();
    let mut terminal = init_terminal()?;
    let result = run_app(&mut terminal, options.start_dir);
    restore_terminal(&mut terminal)?;
    if let Some(final_cwd) = result? {
        write_cwd_file_if_requested(options.cwd_file.as_deref(), &final_cwd)?;
    }
    Ok(())
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    match try_init_terminal() {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            let _ = cleanup_terminal_state();
            Err(error)
        }
    }
}

fn try_init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
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

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    terminal.hide_cursor()?;
    Ok(terminal)
}

/// Temporarily tears down the TUI so a blocking terminal app can use stdout.
/// Call [`resume_terminal`] afterwards to restore the TUI.
fn suspend_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
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
        terminal.clear()?;
    }
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}

/// Restores the TUI after [`suspend_terminal`].  Forces a full redraw on the
/// next render cycle so no stale content is left on screen.
fn resume_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        event::EnableMouseCapture,
        EnableFocusChange,
    )?;
    write!(stdout, "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h")?;
    write!(stdout, "\x1b[>4;1m")?;
    stdout.flush()?;
    push_keyboard_enhancement_if_supported(&mut stdout)?;
    terminal.clear()?;
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

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
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
    if !matches!(supports_keyboard_enhancement(), Ok(true)) {
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

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    cwd: Option<PathBuf>,
) -> Result<Option<PathBuf>> {
    let mut app = match cwd {
        Some(cwd) => App::new_at(cwd)?,
        None => App::new()?,
    };

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

        if terminal_focused && app.process_background_jobs() {
            dirty = true;
        }

        if terminal_focused && app.process_pdf_preview_timers() {
            dirty = true;
        }

        if terminal_focused && app.process_pending_scroll() {
            dirty = true;
        }

        if terminal_focused && app.process_preview_refresh_timers() {
            dirty = true;
        }

        if terminal_focused && app.process_preview_prefetch_timers() {
            dirty = true;
        }

        if terminal_focused && app.process_directory_stats_timer() {
            dirty = true;
        }

        if terminal_focused && app.process_directory_item_count_timer() {
            dirty = true;
        }

        if terminal_focused && app.process_browser_wheel_timers() {
            dirty = true;
        }

        if terminal_focused && app.process_image_preview_timers() {
            dirty = true;
        }

        if terminal_focused && app.process_sidebar_refresh() {
            dirty = true;
        }

        if terminal_focused {
            match app.process_auto_reload() {
                Ok(changed) => {
                    dirty |= changed;
                }
                Err(error) => {
                    app.report_runtime_error("Auto-reload failed", &error);
                    dirty = true;
                }
            }
        }

        if dirty && terminal_focused {
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
                    app.handle_terminal_image_resize();
                    dirty = true;
                } else if matches!(event, Event::Resize(_, _)) {
                    app.handle_terminal_image_resize();
                    dirty |= terminal_focused;
                } else {
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
                        suspend_terminal(terminal, true)?;
                        run_blocking_in_terminal(&program, &args);
                        resume_terminal(terminal)?;
                        None
                    }
                    PendingTerminalTask::Shell { cwd } => {
                        suspend_terminal(terminal, true)?;
                        let shell_result = shell::run_in_current_terminal(&cwd);
                        resume_terminal(terminal)?;
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
                            suspend_terminal(terminal, false)?;
                            let result = zoxide::run_query_in_terminal(&cwd);
                            resume_terminal(terminal)?;
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
    app.queue_forced_iterm_preview_erase();
    let mut overlay_bytes = app.clear_preview_overlay()?;
    overlay_bytes.extend(app.iterm_pre_draw_erase());
    if !overlay_bytes.is_empty() {
        terminal.backend_mut().write_all(&overlay_bytes)?;
        terminal.backend_mut().flush()?;
    }
    Ok(final_cwd)
}

fn draw_terminal_frame(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<bool> {
    execute!(terminal.backend_mut(), BeginSynchronizedUpdate)?;

    let draw_result = (|| -> Result<bool> {
        if app.take_pending_resize_clear() {
            terminal.clear()?;
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
        terminal.draw(|frame| ui::render(frame, app, &mut frame_state))?;
        let dirty = app.set_frame_state(frame_state);
        if !app.browser_wheel_burst_active() {
            let overlay_bytes = app.present_preview_overlay()?;
            if !overlay_bytes.is_empty() {
                terminal.backend_mut().write_all(&overlay_bytes)?;
            }
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
    use crate::{ACTIVE_SCROLL_POLL_INTERVAL, IDLE_POLL_INTERVAL, event_poll_interval};
    use ratatui::{buffer::Buffer, layout::Rect, style::Style};
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
