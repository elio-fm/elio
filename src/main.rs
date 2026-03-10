mod app;
mod appearance;
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
const ACTIVE_SCROLL_POLL_INTERVAL: Duration = Duration::from_millis(16);
const RELATIVE_TIME_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

fn main() -> Result<()> {
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

        if app.process_pending_scroll() {
            dirty = true;
        }

        if app.process_auto_reload()? {
            dirty = true;
        }

        if dirty {
            let mut frame_state = app::FrameState::default();
            terminal.draw(|frame| ui::render(frame, &app, &mut frame_state))?;
            dirty = app.set_frame_state(frame_state);
        }

        let wants_search_cursor = app.search_is_open();
        if wants_search_cursor != search_cursor_active {
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

        let poll_interval = if app.has_pending_scroll() || app.has_pending_auto_reload() {
            ACTIVE_SCROLL_POLL_INTERVAL
        } else {
            IDLE_POLL_INTERVAL
        };

        if event::poll(poll_interval)? {
            let event = event::read()?;
            if matches!(event, Event::Resize(_, _)) {
                dirty = true;
                continue;
            }
            app.handle_event(event)?;
            dirty = true;
        }
    }

    Ok(())
}
