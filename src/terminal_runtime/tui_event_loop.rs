use super::{
    cd_on_exit,
    input_reader::{InputEvent, InputReader},
    kitty_dnd, shell_here,
    tui_drawing::{AppTerminal, Drainer, ThreadedWriter, draw_terminal_frame},
    zoxide,
};
use crate::{
    RunOptions, RunOutcome,
    app::{App, PendingTerminalTask},
    chooser::{self, ChooserExit},
};
use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
    event::{
        self, DisableBracketedPaste, DisableFocusChange, EnableBracketedPaste, EnableFocusChange,
        Event, KeyboardEnhancementFlags, MouseEvent, MouseEventKind, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};
#[cfg(unix)]
use std::fs::OpenOptions;
use std::{
    io::{self, ErrorKind, IsTerminal, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
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

fn init_terminal() -> Result<(AppTerminal, Drainer, kitty_dnd::KittyDndRuntime)> {
    match try_init_terminal() {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            let _ = cleanup_terminal_state();
            Err(error)
        }
    }
}

fn try_init_terminal() -> Result<(AppTerminal, Drainer, kitty_dnd::KittyDndRuntime)> {
    enable_raw_mode()?;
    let (mut terminal_output, frame_output) = terminal_output_handles()?;
    let kitty_dnd = kitty_dnd::detect_kitty_dnd_runtime();
    execute!(
        terminal_output,
        EnterAlternateScreen,
        event::EnableMouseCapture,
        EnableBracketedPaste,
        EnableFocusChange
    )?;

    write!(
        terminal_output,
        "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h"
    )?;
    write!(terminal_output, "\x1b[>4;1m")?;

    if kitty_dnd.is_enabled() {
        write!(
            terminal_output,
            "{}",
            kitty_dnd::startup_sequence(kitty_dnd.drag_machine_id())
        )?;
    }

    terminal_output.flush()?;
    push_keyboard_enhancement_if_supported(&mut terminal_output)?;

    let writer = ThreadedWriter::new(frame_output);
    let drainer = writer.drainer();
    let backend = CrosstermBackend::new(writer);
    let mut terminal = Terminal::new(backend)?;
    clear_for_full_repaint(&mut terminal)?;
    terminal.hide_cursor()?;
    Ok((terminal, drainer, kitty_dnd))
}

#[cfg(unix)]
fn terminal_output_handles() -> io::Result<(Box<dyn Write + Send>, Box<dyn Write + Send>)> {
    let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    Ok((Box::new(tty.try_clone()?), Box::new(tty)))
}

#[cfg(not(unix))]
fn terminal_output_handles() -> io::Result<(Box<dyn Write + Send>, Box<dyn Write + Send>)> {
    Ok((Box::new(io::stdout()), Box::new(io::stdout())))
}

pub(super) fn clear_for_full_repaint(terminal: &mut AppTerminal) -> io::Result<()> {
    let size = terminal.size()?;
    terminal.resize(Rect::new(0, 0, size.width, size.height))
}

fn suspend_terminal(
    terminal: &mut AppTerminal,
    drainer: &Drainer,
    leave_alternate: bool,
    kitty_dnd: &kitty_dnd::KittyDndRuntime,
) -> Result<()> {
    let backend = terminal.backend_mut();
    if kitty_dnd.is_enabled() {
        write!(backend, "{}", kitty_dnd::disable_sequence())?;
    }
    write!(backend, "\x1b[>4;0m")?;
    write!(backend, "\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l")?;
    backend.flush()?;
    pop_keyboard_enhancement_if_supported(terminal.backend_mut())?;
    execute!(
        terminal.backend_mut(),
        event::DisableMouseCapture,
        DisableBracketedPaste,
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
    let _ = terminal.backend_mut().flush();
    drainer.drain();
    Ok(())
}

fn resume_terminal(
    terminal: &mut AppTerminal,
    drainer: &Drainer,
    kitty_dnd: &kitty_dnd::KittyDndRuntime,
) -> Result<()> {
    enable_raw_mode()?;
    {
        let backend = terminal.backend_mut();
        execute!(
            backend,
            EnterAlternateScreen,
            event::EnableMouseCapture,
            EnableBracketedPaste,
            EnableFocusChange,
        )?;
        write!(backend, "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h")?;
        write!(backend, "\x1b[>4;1m")?;
        if kitty_dnd.is_enabled() {
            write!(
                backend,
                "{}",
                kitty_dnd::startup_sequence(kitty_dnd.drag_machine_id())
            )?;
        }
        backend.flush()?;
    }
    drainer.drain();
    push_keyboard_enhancement_if_supported(terminal.backend_mut())?;
    clear_for_full_repaint(terminal)?;
    terminal.hide_cursor()?;
    Ok(())
}

fn restore_terminal(
    terminal: &mut AppTerminal,
    drainer: &Drainer,
    kitty_dnd: &kitty_dnd::KittyDndRuntime,
) -> Result<()> {
    let backend = terminal.backend_mut();
    if kitty_dnd.is_enabled() {
        write!(backend, "{}", kitty_dnd::disable_sequence())?;
    }
    write!(backend, "\x1b[>4;0m")?;
    write!(backend, "\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l")?;
    backend.flush()?;
    pop_keyboard_enhancement_if_supported(terminal.backend_mut())?;
    execute!(
        terminal.backend_mut(),
        event::DisableMouseCapture,
        DisableBracketedPaste,
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
    if let Ok((mut terminal_output, _)) = terminal_output_handles() {
        let _ = write!(terminal_output, "\x1b[>4;0m");
        let _ = write!(terminal_output, "{}", kitty_dnd::disable_sequence());
        let _ = write!(
            terminal_output,
            "\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l"
        );
        let _ = terminal_output.flush();
        let _ = execute!(
            terminal_output,
            event::DisableMouseCapture,
            DisableBracketedPaste,
            DisableFocusChange,
            SetCursorStyle::DefaultUserShape,
            LeaveAlternateScreen,
        );
    }
    disable_raw_mode()?;
    Ok(())
}

fn push_keyboard_enhancement_if_supported<W: Write>(writer: &mut W) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Ok(());
    }

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
    let (mut terminal, drainer, kitty_dnd) = init_terminal()?;
    let result = run_app(
        &mut terminal,
        &drainer,
        &kitty_dnd,
        start_dir,
        start_focus,
        reveal_hidden_start_focus,
        chooser_file.is_some(),
    );
    restore_terminal(&mut terminal, &drainer, &kitty_dnd)?;
    let app_exit = result?;
    if let Some(final_cwd) = app_exit.final_cwd {
        cd_on_exit::write_if_requested(cwd_file.as_deref(), &final_cwd)?;
    }
    match app_exit.chooser {
        Some(ChooserExit::Confirmed(paths)) => {
            chooser::write_selected_paths(chooser_file.as_deref(), &paths)?;
            Ok(RunOutcome::Success)
        }
        Some(ChooserExit::Cancelled) => Ok(RunOutcome::Cancelled),
        None => Ok(RunOutcome::Success),
    }
}

fn run_open_command_in_terminal(
    program: &str,
    args: &[String],
    elevated_cwd: &Path,
) -> std::io::Result<std::process::ExitStatus> {
    let mut command = Command::new(program);
    command.args(args);
    #[cfg(unix)]
    crate::elevated_session::prepare_external(&mut command, Some(elevated_cwd))?;
    #[cfg(not(unix))]
    let _ = elevated_cwd;
    command.status()
}

#[cfg(unix)]
struct EditorTempCleanup(PathBuf);

#[cfg(unix)]
impl Drop for EditorTempCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(unix)]
fn run_blocking_in_terminal_result(
    program: &str,
    args: &[String],
) -> std::io::Result<std::process::ExitStatus> {
    let mut command = Command::new(program);
    command.args(args);
    crate::elevated_session::prepare_external(&mut command, None)?;
    command.status()
}

fn refresh_after_shell(app: &mut App, cwd: &Path) {
    let cwd_label = crate::filesystem::display_path(cwd);
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
    kitty_dnd: &kitty_dnd::KittyDndRuntime,
    cwd: Option<PathBuf>,
    start_focus: Option<PathBuf>,
    reveal_hidden_start_focus: bool,
    chooser_enabled: bool,
) -> Result<AppExit> {
    #[cfg(unix)]
    if kitty_dnd.is_enabled() {
        kitty_dnd::prewarm_drag_image_renderer();
    }

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

    let input_reader = match InputReader::new(kitty_dnd.is_enabled()) {
        Ok(reader) => reader,
        Err(error) => {
            if kitty_dnd.is_enabled() {
                terminal
                    .backend_mut()
                    .write_all(kitty_dnd::disable_sequence().as_bytes())?;
                terminal.backend_mut().flush()?;
                app.set_status_message(format!("Kitty DND disabled: {error}"));
            }
            InputReader::Crossterm
        }
    };
    let mut dirty = true;
    let mut search_cursor_active = false;
    let mut terminal_focused = true;
    let mut last_relative_time_refresh_at = Instant::now();
    #[cfg(unix)]
    let mut pending_drag_out = kitty_dnd::PendingDragOut::default();
    #[cfg(unix)]
    let mut pending_drop_in = kitty_dnd::PendingDropIn::default();

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
            || app.file_operations.create_is_open()
            || app.file_operations.rename_is_open()
            || app.file_operations.bulk_rename_is_open()
            || app.file_operations.archive_create_is_open()
            || app.file_operations.archive_password_is_open();
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

        if let Some(first_input) = input_reader.read(poll_interval)? {
            // Batch all immediately-available events into one render cycle.
            // This prevents lag when events (especially scroll events from high-frequency
            // terminals) arrive faster than the app can render: instead of one render per
            // event we accumulate all queued events first and render the final state once.
            let mut next_input = Some(first_input);
            loop {
                let input = match next_input.take() {
                    Some(input) => input,
                    None => match input_reader.try_read()? {
                        Some(input) => input,
                        None => break,
                    },
                };
                let input = input_reader.coalesce_resizes(input, &mut next_input)?;
                #[cfg(unix)]
                let input_result = handle_runtime_input(
                    terminal,
                    &mut app,
                    input,
                    &mut pending_drag_out,
                    &mut pending_drop_in,
                )?;
                #[cfg(not(unix))]
                let input_result = handle_runtime_input(input)?;
                let Some(event) = input_result else {
                    dirty = true;
                    next_input = input_reader.try_read()?;
                    continue;
                };
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
                };
                next_input = input_reader.try_read()?;
            }

            if app.should_quit {
                break;
            }

            // A terminal task (e.g. nvim from Open With, or zoxide) needs the real terminal.
            // Suspend the TUI, run the task blocking, then restore.
            if let Some(task) = app.pending_terminal_task.take() {
                let zoxide_result = match task {
                    PendingTerminalTask::Command { program, args } => {
                        let cwd = app.file_browser.cwd.clone();
                        input_reader.set_paused(true);
                        suspend_terminal(terminal, drainer, true, kitty_dnd)?;
                        let result = run_open_command_in_terminal(&program, &args, &cwd);
                        resume_terminal(terminal, drainer, kitty_dnd)?;
                        app.invalidate_terminal_image_overlay_after_terminal_task();
                        input_reader.set_paused(false);
                        if let Err(error) = result {
                            app.set_status_message(format!("Could not open terminal app: {error}"));
                        }
                        None
                    }
                    PendingTerminalTask::Commands(commands) => {
                        let cwd = app.file_browser.cwd.clone();
                        input_reader.set_paused(true);
                        suspend_terminal(terminal, drainer, true, kitty_dnd)?;
                        let mut last_error = None;
                        for (program, args) in commands {
                            if let Err(error) = run_open_command_in_terminal(&program, &args, &cwd)
                            {
                                last_error = Some(error);
                            }
                        }
                        resume_terminal(terminal, drainer, kitty_dnd)?;
                        app.invalidate_terminal_image_overlay_after_terminal_task();
                        input_reader.set_paused(false);
                        if let Some(error) = last_error {
                            app.set_status_message(format!("Could not open terminal app: {error}"));
                        }
                        None
                    }
                    #[cfg(unix)]
                    PendingTerminalTask::EditorBulkRename {
                        program,
                        args,
                        session,
                    } => {
                        let _temp_cleanup = EditorTempCleanup(session.temp_path.clone());
                        input_reader.set_paused(true);
                        suspend_terminal(terminal, drainer, true, kitty_dnd)?;
                        let result = run_blocking_in_terminal_result(&program, &args);
                        resume_terminal(terminal, drainer, kitty_dnd)?;
                        app.invalidate_terminal_image_overlay_after_terminal_task();
                        input_reader.set_paused(false);
                        if let Err(error) = app.finish_editor_bulk_rename(session, result) {
                            app.report_runtime_error("Editor rename failed", &error);
                        }
                        None
                    }
                    PendingTerminalTask::ShellHere { cwd } => {
                        input_reader.set_paused(true);
                        suspend_terminal(terminal, drainer, true, kitty_dnd)?;
                        let shell_result = shell_here::run(&cwd);
                        resume_terminal(terminal, drainer, kitty_dnd)?;
                        app.invalidate_terminal_image_overlay_after_terminal_task();
                        input_reader.set_paused(false);
                        match shell_result {
                            Ok(()) => refresh_after_shell(&mut app, &cwd),
                            Err(error) => app.set_status_message(error),
                        }
                        None
                    }
                    PendingTerminalTask::Zoxide => {
                        let cwd = app.file_browser.cwd.clone();
                        if let Some(result) = zoxide::preflight(&cwd) {
                            Some(result)
                        } else {
                            input_reader.set_paused(true);
                            suspend_terminal(terminal, drainer, false, kitty_dnd)?;
                            let result = zoxide::run_query_in_terminal(&cwd);
                            resume_terminal(terminal, drainer, kitty_dnd)?;
                            app.invalidate_terminal_image_overlay_after_terminal_task();
                            input_reader.set_paused(false);
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
        .then(|| app.file_browser.cwd.clone());
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

#[cfg(unix)]
fn handle_runtime_input(
    terminal: &mut AppTerminal,
    app: &mut App,
    input: InputEvent,
    pending_drag_out: &mut kitty_dnd::PendingDragOut,
    pending_drop_in: &mut kitty_dnd::PendingDropIn,
) -> Result<Option<Event>> {
    match input {
        InputEvent::Terminal(event) => Ok(Some(event)),
        InputEvent::KittyDnd(event) => {
            kitty_dnd::handle_event(terminal, app, event, pending_drag_out, pending_drop_in)?;
            Ok(None)
        }
    }
}

#[cfg(not(unix))]
fn handle_runtime_input(input: InputEvent) -> Result<Option<Event>> {
    match input {
        InputEvent::Terminal(event) => Ok(Some(event)),
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
#[path = "tests/tui_event_loop.rs"]
mod tests;
