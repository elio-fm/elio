mod draw;
mod session_files;
mod terminal;

use self::{
    draw::draw_terminal_frame,
    session_files::{write_chooser_file_if_requested, write_cwd_file_if_requested},
    terminal::{
        AppTerminal, Drainer, init_terminal, restore_terminal, resume_terminal, suspend_terminal,
    },
};
use crate::{
    RunOptions, RunOutcome,
    app::{App, ChooserExit, PendingTerminalTask},
    config, path_display, shell, ui, zoxide,
};
use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
    event::{self, Event, MouseEvent, MouseEventKind},
    execute,
};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const ACTIVE_SCROLL_POLL_INTERVAL: Duration = Duration::from_millis(12);
const WINDOWS_TERMINAL_ACTIVE_POLL_INTERVAL: Duration = Duration::from_millis(24);
const RELATIVE_TIME_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug)]
struct AppExit {
    final_cwd: Option<PathBuf>,
    chooser: Option<ChooserExit>,
}

pub(crate) fn run_with_startup_state(
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
    restore_terminal(&mut terminal, &drainer)?;
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
    app.refresh_git_branch();

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
            || app.local_filter_is_editing()
            || app.create_is_open()
            || app.rename_is_open()
            || app.bulk_rename_is_open()
            || app.archive_password_is_open();
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
    use super::{
        ACTIVE_SCROLL_POLL_INTERVAL, IDLE_POLL_INTERVAL, event_implies_terminal_focus,
        event_poll_interval,
    };
    use crossterm::event::Event;
    use std::time::Duration;

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
        assert!(event_implies_terminal_focus(&Event::Key(KeyEvent::new(
            KeyCode::Char('j'),
            KeyModifiers::NONE
        ))));
        assert!(event_implies_terminal_focus(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        })));
        assert!(event_implies_terminal_focus(&Event::Paste(
            "text".to_string()
        )));
    }

    #[test]
    fn focus_and_resize_events_do_not_imply_terminal_focus() {
        // Focus/resize events carry no focus implication and must not trip the
        // recovery path (FocusLost in particular would be misread as focus).
        assert!(!event_implies_terminal_focus(&Event::FocusLost));
        assert!(!event_implies_terminal_focus(&Event::FocusGained));
        assert!(!event_implies_terminal_focus(&Event::Resize(80, 24)));
    }
}
