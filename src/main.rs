mod app;
mod appearance;
mod config;
mod file_facts;
mod search;
mod ui;

use crate::app::App;
use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io,
    time::{Duration, Instant},
};

const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const ACTIVE_SCROLL_POLL_INTERVAL: Duration = Duration::from_millis(12);
const RELATIVE_TIME_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

fn main() -> Result<()> {
    config::initialize();
    appearance::initialize();
    let mut terminal = init_terminal()?;
    let result = run(&mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    terminal.hide_cursor()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        SetCursorStyle::DefaultUserShape,
        LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new()?;
    app.enable_terminal_pdf_previews();
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
            let mut frame_state = app::FrameState::default();
            terminal.draw(|frame| ui::render(frame, &app, &mut frame_state))?;
            dirty = app.set_frame_state(frame_state);
            if !app.browser_wheel_burst_active() {
                app.present_pdf_overlay()?;
            }
        }

        let wants_search_cursor = app.search_is_open();
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
            .chain(app.pending_browser_wheel_timer())
            .min()
            .map(|delay| delay.min(base_poll_interval))
            .unwrap_or(base_poll_interval);

        if event::poll(poll_interval)? {
            let event = event::read()?;
            if matches!(event, Event::Resize(_, _)) {
                app.handle_pdf_terminal_resize();
                dirty = true;
                continue;
            }
            match app.handle_event(event) {
                Ok(()) => dirty = true,
                Err(error) => {
                    app.report_runtime_error("Action failed", &error);
                    dirty = true;
                }
            }
        }
    }

    app.clear_pdf_overlay()?;
    Ok(())
}
