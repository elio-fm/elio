mod draw;
mod input;
mod kitty_dnd;
mod session_files;
mod terminal;

use self::{
    draw::draw_terminal_frame,
    input::{RuntimeInputEvent, RuntimeInputReader},
    session_files::{write_chooser_file_if_requested, write_cwd_file_if_requested},
    terminal::{
        AppTerminal, Drainer, init_terminal, restore_terminal, resume_terminal, suspend_terminal,
    },
};
use crate::{
    RunOptions, RunOutcome,
    app::{App, ChooserExit, ClipOp, PendingTerminalTask},
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
const MAX_INPUT_EVENTS_PER_FRAME: usize = 64;
const MAX_INPUT_BATCH_DURATION: Duration = Duration::from_millis(4);

#[derive(Debug)]
struct AppExit {
    final_cwd: Option<PathBuf>,
    chooser: Option<ChooserExit>,
}

#[derive(Debug, Default)]
struct PendingDragOut {
    active: bool,
    uri_list: Vec<u8>,
}

impl PendingDragOut {
    fn reset(&mut self) {
        self.active = false;
        self.uri_list.clear();
    }
}

#[derive(Debug, Default)]
struct PendingDropIn {
    op: Option<ClipOp>,
}

impl PendingDropIn {
    fn set(&mut self, op: ClipOp) {
        self.op = Some(op);
    }

    fn take(&mut self) -> Option<ClipOp> {
        self.op.take()
    }

    fn reset(&mut self) {
        self.op = None;
    }
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
    kitty_dnd: &kitty_dnd::KittyDndRuntime,
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

    let input_reader = match RuntimeInputReader::new(kitty_dnd.is_enabled()) {
        Ok(reader) => reader,
        Err(error) => {
            if kitty_dnd.is_enabled() {
                terminal
                    .backend_mut()
                    .write_all(kitty_dnd::disable_sequence().as_bytes())?;
                terminal.backend_mut().flush()?;
                app.set_status_message(format!("Kitty DND disabled: {error}"));
            }
            RuntimeInputReader::Crossterm
        }
    };
    let mut dirty = true;
    let mut search_cursor_active = false;
    let mut terminal_focused = true;
    let mut last_relative_time_refresh_at = Instant::now();
    let mut pending_drag_out = PendingDragOut::default();
    let mut pending_drop_in = PendingDropIn::default();

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
            || app.archive_create_is_open()
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

        if let Some(first_input) = read_runtime_input(&input_reader, poll_interval)? {
            // Batch all immediately-available events into one render cycle.
            // This prevents lag when events (especially scroll events from high-frequency
            // terminals) arrive faster than the app can render: instead of one render per
            // event we accumulate all queued events first and render the final state once.
            let mut next_input = Some(first_input);
            let batch_started_at = Instant::now();
            let mut batched_events = 0usize;
            loop {
                batched_events += 1;
                let input = match next_input.take() {
                    Some(input) => input,
                    None => match try_read_runtime_input(&input_reader)? {
                        Some(input) => input,
                        None => break,
                    },
                };
                let input = coalesce_resize_inputs(
                    &input_reader,
                    input,
                    &mut next_input,
                    &mut batched_events,
                )?;
                let Some(event) = handle_runtime_input(
                    terminal,
                    &mut app,
                    input,
                    &mut pending_drag_out,
                    &mut pending_drop_in,
                )?
                else {
                    dirty = true;
                    next_input = try_read_runtime_input(&input_reader)?;
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
                next_input = try_read_runtime_input(&input_reader)?;
                if next_input.is_some()
                    && (batched_events >= MAX_INPUT_EVENTS_PER_FRAME
                        || batch_started_at.elapsed() >= MAX_INPUT_BATCH_DURATION)
                {
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
                        pause_runtime_input(&input_reader, true);
                        suspend_terminal(terminal, drainer, true, kitty_dnd)?;
                        run_blocking_in_terminal(&program, &args);
                        resume_terminal(terminal, drainer, kitty_dnd)?;
                        pause_runtime_input(&input_reader, false);
                        None
                    }
                    PendingTerminalTask::Shell { cwd } => {
                        pause_runtime_input(&input_reader, true);
                        suspend_terminal(terminal, drainer, true, kitty_dnd)?;
                        let shell_result = shell::run_in_current_terminal(&cwd);
                        resume_terminal(terminal, drainer, kitty_dnd)?;
                        pause_runtime_input(&input_reader, false);
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
                            pause_runtime_input(&input_reader, true);
                            suspend_terminal(terminal, drainer, false, kitty_dnd)?;
                            let result = zoxide::run_query_in_terminal(&cwd);
                            resume_terminal(terminal, drainer, kitty_dnd)?;
                            pause_runtime_input(&input_reader, false);
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

fn read_runtime_input(
    reader: &RuntimeInputReader,
    timeout: Duration,
) -> Result<Option<RuntimeInputEvent>> {
    match reader {
        RuntimeInputReader::Crossterm => {
            if event::poll(timeout)? {
                Ok(Some(RuntimeInputEvent::Terminal(event::read()?)))
            } else {
                Ok(None)
            }
        }
        RuntimeInputReader::Custom(reader) => Ok(reader.recv_timeout(timeout)?),
    }
}

fn try_read_runtime_input(reader: &RuntimeInputReader) -> Result<Option<RuntimeInputEvent>> {
    match reader {
        RuntimeInputReader::Crossterm => {
            if event::poll(Duration::ZERO)? {
                Ok(Some(RuntimeInputEvent::Terminal(event::read()?)))
            } else {
                Ok(None)
            }
        }
        RuntimeInputReader::Custom(reader) => Ok(reader.try_recv()?),
    }
}

fn pause_runtime_input(reader: &RuntimeInputReader, paused: bool) {
    if let RuntimeInputReader::Custom(reader) = reader {
        reader.set_paused(paused);
    }
}

fn coalesce_resize_inputs(
    reader: &RuntimeInputReader,
    input: RuntimeInputEvent,
    next_input: &mut Option<RuntimeInputEvent>,
    batched_events: &mut usize,
) -> Result<RuntimeInputEvent> {
    let RuntimeInputEvent::Terminal(Event::Resize(mut width, mut height)) = input else {
        return Ok(input);
    };

    let started_at = Instant::now();
    loop {
        let candidate = if let Some(input) = next_input.take() {
            Some(input)
        } else {
            try_read_runtime_input(reader)?
        };
        match candidate {
            Some(RuntimeInputEvent::Terminal(Event::Resize(w, h))) => {
                width = w;
                height = h;
                *batched_events += 1;
                if *batched_events >= MAX_INPUT_EVENTS_PER_FRAME
                    || started_at.elapsed() >= MAX_INPUT_BATCH_DURATION
                {
                    break;
                }
            }
            Some(other) => {
                *next_input = Some(other);
                break;
            }
            None => break,
        }
    }

    Ok(RuntimeInputEvent::Terminal(Event::Resize(width, height)))
}

fn handle_runtime_input(
    terminal: &mut AppTerminal,
    app: &mut App,
    input: RuntimeInputEvent,
    pending_drag_out: &mut PendingDragOut,
    pending_drop_in: &mut PendingDropIn,
) -> Result<Option<Event>> {
    match input {
        RuntimeInputEvent::Terminal(event) => Ok(Some(event)),
        RuntimeInputEvent::KittyDnd(event) => {
            handle_kitty_dnd_event(terminal, app, event, pending_drag_out, pending_drop_in)?;
            Ok(None)
        }
    }
}

fn handle_kitty_dnd_event(
    terminal: &mut AppTerminal,
    app: &mut App,
    event: kitty_dnd::KittyDndEvent,
    pending_drag_out: &mut PendingDragOut,
    pending_drop_in: &mut PendingDropIn,
) -> Result<()> {
    match event {
        kitty_dnd::KittyDndEvent::DropOffer {
            mime_index,
            operation,
            final_drop,
        } => {
            let chosen_op = choose_drop_op(operation);
            let sequence = if pending_drag_out.active && final_drop {
                pending_drag_out.reset();
                pending_drop_in.reset();
                app.clear_drag_state();
                format!(
                    "{}{}",
                    kitty_dnd::finish_drop_sequence(kitty_dnd::DropFinish::Reject),
                    kitty_dnd::cancel_drag_sequence()
                )
            } else if pending_drag_out.active {
                pending_drop_in.reset();
                kitty_dnd::reject_drop_sequence().to_string()
            } else if final_drop {
                pending_drop_in.set(chosen_op);
                kitty_dnd::request_drop_data_sequence(mime_index)
            } else {
                pending_drop_in.set(chosen_op);
                kitty_dnd::accept_drop_sequence(drop_op_to_dnd_operation(chosen_op))
            };
            terminal.backend_mut().write_all(sequence.as_bytes())?;
            terminal.backend_mut().flush()?;
        }
        kitty_dnd::KittyDndEvent::DropData {
            paths,
            unsupported_schemes,
            ..
        } => {
            let was_own_drag = pending_drag_out.active;
            let op = pending_drop_in.take();
            let mut finish = kitty_dnd::DropFinish::Reject;
            if was_own_drag {
                pending_drag_out.reset();
                app.clear_drag_state();
            } else if !unsupported_schemes.is_empty() {
                app.set_status_message(unsupported_drop_scheme_status(&unsupported_schemes));
            } else if let Some(op) = op {
                if app.drop_external_paths(paths, op)? {
                    finish = clip_op_to_drop_finish(op);
                }
            } else {
                app.set_status_message("Drop was not negotiated");
            }
            terminal
                .backend_mut()
                .write_all(kitty_dnd::finish_drop_sequence(finish).as_bytes())?;
            if was_own_drag {
                terminal
                    .backend_mut()
                    .write_all(kitty_dnd::cancel_drag_sequence().as_bytes())?;
            }
            terminal.backend_mut().flush()?;
        }
        kitty_dnd::KittyDndEvent::DropLeave => {}
        kitty_dnd::KittyDndEvent::DropDataError {
            mime_index: _,
            message,
        } => {
            let was_own_drag = pending_drag_out.active;
            if pending_drag_out.active {
                pending_drag_out.reset();
                app.clear_drag_state();
            }
            terminal.backend_mut().write_all(
                kitty_dnd::finish_drop_sequence(kitty_dnd::DropFinish::Reject).as_bytes(),
            )?;
            if was_own_drag {
                terminal
                    .backend_mut()
                    .write_all(kitty_dnd::cancel_drag_sequence().as_bytes())?;
            }
            if !message.is_empty() {
                app.set_status_message(format!("Drop failed: {message}"));
            }
            terminal.backend_mut().flush()?;
        }
        kitty_dnd::KittyDndEvent::DropUnsupported { final_drop } => {
            pending_drop_in.reset();
            let sequence = if final_drop {
                kitty_dnd::finish_drop_sequence(kitty_dnd::DropFinish::Reject)
            } else {
                kitty_dnd::reject_drop_sequence()
            };
            terminal.backend_mut().write_all(sequence.as_bytes())?;
            terminal.backend_mut().flush()?;
        }
        kitty_dnd::KittyDndEvent::DragOffer { x, y } => {
            if pending_drag_out.active {
                return Ok(());
            }
            let paths = app.take_drag_export_paths_at(x, y);
            let uri_list = kitty_dnd::uri_list_payload(&paths);
            if uri_list.is_empty() {
                pending_drag_out.reset();
                app.clear_drag_candidate();
                terminal
                    .backend_mut()
                    .write_all(kitty_dnd::cancel_drag_sequence().as_bytes())?;
                terminal.backend_mut().flush()?;
                return Ok(());
            }

            let label = drag_icon_label(&paths);
            let mut sequence = kitty_dnd::agree_drag_sequence(kitty_dnd::DndOperation::Either);
            sequence.push_str(&kitty_dnd::present_drag_data_sequence(0, &uri_list));
            sequence.push_str(&kitty_dnd::present_drag_icon_sequence(&label));
            sequence.push_str(kitty_dnd::start_drag_sequence());
            terminal.backend_mut().write_all(sequence.as_bytes())?;
            terminal.backend_mut().flush()?;

            pending_drag_out.active = true;
            pending_drag_out.uri_list = uri_list;
        }
        kitty_dnd::KittyDndEvent::DragDataRequested { mime_index } => {
            let sequence = if pending_drag_out.active && mime_index == 0 {
                kitty_dnd::send_drag_data_sequence(mime_index, &pending_drag_out.uri_list)
            } else {
                kitty_dnd::drag_data_error_sequence(mime_index, "ENOENT")
            };
            terminal.backend_mut().write_all(sequence.as_bytes())?;
            terminal.backend_mut().flush()?;
        }
        kitty_dnd::KittyDndEvent::DragStarted
        | kitty_dnd::KittyDndEvent::DragAccepted { mime_index: _ }
        | kitty_dnd::KittyDndEvent::DragActionChanged { operation: _ }
        | kitty_dnd::KittyDndEvent::DragDropped => {}
        kitty_dnd::KittyDndEvent::DragEnded { cancelled: _ } => {
            pending_drag_out.reset();
            app.clear_drag_state();
        }
        kitty_dnd::KittyDndEvent::DragError { message } => {
            pending_drag_out.reset();
            app.clear_drag_state();
            if !message.is_empty() {
                app.set_status_message(format!("Kitty DND drag failed: {message}"));
            }
        }
    }
    Ok(())
}

fn unsupported_drop_scheme_status(schemes: &[String]) -> String {
    match schemes {
        [] => "Drop contains no local files".to_string(),
        [scheme] => format!("Unsupported drop URI scheme: {scheme}"),
        schemes => format!("Unsupported drop URI schemes: {}", schemes.join(", ")),
    }
}

fn choose_drop_op(operation: kitty_dnd::DndOperation) -> ClipOp {
    match operation {
        kitty_dnd::DndOperation::Copy => ClipOp::Yank,
        kitty_dnd::DndOperation::Move | kitty_dnd::DndOperation::Either => ClipOp::Cut,
    }
}

fn drop_op_to_dnd_operation(op: ClipOp) -> kitty_dnd::DndOperation {
    match op {
        ClipOp::Yank => kitty_dnd::DndOperation::Copy,
        ClipOp::Cut => kitty_dnd::DndOperation::Move,
    }
}

fn clip_op_to_drop_finish(op: ClipOp) -> kitty_dnd::DropFinish {
    match op {
        ClipOp::Yank => kitty_dnd::DropFinish::Copy,
        ClipOp::Cut => kitty_dnd::DropFinish::Move,
    }
}

fn drag_icon_label(paths: &[PathBuf]) -> String {
    const MAX_LABEL_CHARS: usize = 32;

    let label = match paths {
        [path] => path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "1 item".to_string()),
        paths => format!("{} items", paths.len()),
    };

    if label.chars().count() <= MAX_LABEL_CHARS {
        return label;
    }

    let mut truncated: String = label.chars().take(MAX_LABEL_CHARS - 3).collect();
    truncated.push_str("...");
    truncated
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
        ACTIVE_SCROLL_POLL_INTERVAL, IDLE_POLL_INTERVAL, drag_icon_label,
        event_implies_terminal_focus, event_poll_interval, unsupported_drop_scheme_status,
    };
    use crossterm::event::Event;
    use std::{path::PathBuf, time::Duration};

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

    #[test]
    fn unsupported_drop_scheme_status_names_one_or_many_schemes() {
        assert_eq!(
            unsupported_drop_scheme_status(&["trash".to_string()]),
            "Unsupported drop URI scheme: trash"
        );
        assert_eq!(
            unsupported_drop_scheme_status(&["trash".to_string(), "smb".to_string()]),
            "Unsupported drop URI schemes: trash, smb"
        );
    }

    #[test]
    fn drag_icon_label_uses_name_for_single_item_and_count_for_many() {
        assert_eq!(
            drag_icon_label(&[PathBuf::from("/tmp/report.pdf")]),
            "report.pdf"
        );
        assert_eq!(
            drag_icon_label(&[PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")]),
            "2 items"
        );
    }

    #[test]
    fn drag_icon_label_truncates_long_names() {
        assert_eq!(
            drag_icon_label(&[PathBuf::from(
                "/tmp/abcdefghijklmnopqrstuvwxyz0123456789.txt"
            )]),
            "abcdefghijklmnopqrstuvwxyz012..."
        );
    }
}
