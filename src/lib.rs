mod app;
mod config;
mod core;
mod file_info;
mod fs;
mod preview;
mod ui;

use crate::app::App;
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
    io::{self, ErrorKind, Write},
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const ACTIVE_SCROLL_POLL_INTERVAL: Duration = Duration::from_millis(12);
const WINDOWS_TERMINAL_ACTIVE_POLL_INTERVAL: Duration = Duration::from_millis(24);
const RELATIVE_TIME_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub fn run() -> Result<()> {
    run_at_path(None)
}

pub fn run_at(cwd: PathBuf) -> Result<()> {
    run_at_path(Some(cwd))
}

fn run_at_path(cwd: Option<PathBuf>) -> Result<()> {
    config::initialize();
    ui::theme::initialize();
    let mut terminal = init_terminal()?;
    let result = run_app(&mut terminal, cwd);
    restore_terminal(&mut terminal)?;
    result
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
fn suspend_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let backend = terminal.backend_mut();
    write!(backend, "\x1b[>4;0m")?;
    write!(backend, "\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l")?;
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

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    cwd: Option<PathBuf>,
) -> Result<()> {
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

            // A terminal app (e.g. nvim) was chosen from Open With.
            // Suspend the TUI, run the command blocking, then restore.
            if let Some((program, args)) = app.pending_terminal_command.take() {
                suspend_terminal(terminal)?;
                run_blocking_in_terminal(&program, &args);
                resume_terminal(terminal)?;
                dirty = true;
            }
        }
    }

    app.queue_forced_iterm_preview_erase();
    let mut overlay_bytes = app.clear_preview_overlay()?;
    overlay_bytes.extend(app.iterm_pre_draw_erase());
    if !overlay_bytes.is_empty() {
        terminal.backend_mut().write_all(&overlay_bytes)?;
        terminal.backend_mut().flush()?;
    }
    Ok(())
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
    use std::{io, time::Duration};

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
