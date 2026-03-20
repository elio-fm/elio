mod app;
mod config;
mod file_info;
mod fs;
mod preview;
mod ui;

use crate::app::App;
use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
    event::{
        self, Event, KeyboardEnhancementFlags, MouseEvent, MouseEventKind,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const ACTIVE_SCROLL_POLL_INTERVAL: Duration = Duration::from_millis(12);
const RELATIVE_TIME_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

fn main() -> Result<()> {
    config::initialize();
    ui::theme::initialize();
    let mut terminal = init_terminal()?;
    let result = run(&mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;

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

    if matches!(supports_keyboard_enhancement(), Ok(true)) {
        // Ctrl+Backspace and similar modified editing keys need the kitty keyboard
        // protocol's "all keys as escape codes" mode to stay distinguishable.
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            )
        )?;
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    terminal.hide_cursor()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    // Disable in reverse order and do it before leaving the alternate screen so the
    // terminal processes the escape sequences while still in the right mode.
    let backend = terminal.backend_mut();
    write!(backend, "\x1b[>4;0m")?; // reset XTSHIFTESCAPE
    write!(backend, "\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l")?; // disable mouse modes
    backend.flush()?;

    execute!(
        terminal.backend_mut(),
        event::DisableMouseCapture,
        SetCursorStyle::DefaultUserShape,
        PopKeyboardEnhancementFlags,
        LeaveAlternateScreen,
    )?;
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new()?;

    // Enable terminal image previews. Detection (`detect_terminal_image_backend`) handles
    // all policy: Kitty is always enabled; Ghostty and WezTerm require ELIO_IMAGE_PREVIEWS=1;
    // other terminals get no images. All image bytes are routed through terminal.backend_mut()
    // so they never bypass crossterm and cannot corrupt mouse reporting.
    app.enable_terminal_image_previews();

    let mut dirty = true;
    let mut search_cursor_active = false;
    let mut last_relative_time_refresh_at = Instant::now();

    loop {
        if last_relative_time_refresh_at.elapsed() >= RELATIVE_TIME_REFRESH_INTERVAL {
            dirty = true;
            last_relative_time_refresh_at = Instant::now();
        }

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

        if app.process_browser_wheel_timers() {
            dirty = true;
        }

        if app.process_image_preview_timers() {
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
            terminal.draw(|frame| ui::render(frame, &app, &mut frame_state))?;
            dirty = app.set_frame_state(frame_state);
            if !app.browser_wheel_burst_active() {
                let overlay_bytes = app.present_preview_overlay()?;
                if !overlay_bytes.is_empty() {
                    terminal.backend_mut().write_all(&overlay_bytes)?;
                    terminal.backend_mut().flush()?;
                }
            }
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

        if app.should_quit {
            break;
        }

        let base_poll_interval = if app.has_pending_scroll()
            || app.has_pending_auto_reload()
            || app.has_pending_background_work()
        {
            ACTIVE_SCROLL_POLL_INTERVAL
        } else {
            IDLE_POLL_INTERVAL
        };
        let poll_interval = app
            .pending_pdf_preview_timer()
            .into_iter()
            .chain(app.pending_image_preview_timer())
            .chain(app.pending_preview_refresh_timer())
            .chain(app.pending_preview_prefetch_timer())
            .chain(app.pending_browser_wheel_timer())
            .min()
            .map(|delay| delay.min(base_poll_interval))
            .unwrap_or(base_poll_interval);

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
                        .open("/tmp/elio-mouse.log")
                        .and_then(|mut f| {
                            writeln!(f, "{:?} col={} row={}", m.kind, m.column, m.row)
                        });
                }
                if matches!(event, Event::Resize(_, _)) {
                    app.handle_terminal_image_resize();
                    dirty = true;
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
                    if needs_render {
                        dirty = true;
                    }
                }
                // Stop batching once there are no more immediately available events.
                if !event::poll(Duration::ZERO)? {
                    break;
                }
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
