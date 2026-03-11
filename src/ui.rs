mod browser;
mod chrome;
mod helpers;
mod overlays;
mod theme;

use crate::{
    app::{App, FrameState},
    config,
};
use ratatui::{
    Frame,
    widgets::{Block, Clear},
};

pub fn render(frame: &mut Frame<'_>, app: &App, state: &mut FrameState) {
    let palette = theme::palette();
    let ui_config = config::ui();

    state.sidebar_hits.clear();
    state.entry_hits.clear();
    state.search_hits.clear();
    state.search_panel = None;
    state.preview_panel = None;
    state.back_button = None;
    state.forward_button = None;
    state.parent_button = None;
    state.hidden_button = None;
    state.view_button = None;
    state.preview_rows_visible = 0;
    state.preview_cols_visible = 0;

    let area = frame.area();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().style(
            ratatui::style::Style::default()
                .bg(palette.bg)
                .fg(palette.text),
        ),
        area,
    );

    if ui_config.show_top_bar {
        let rows = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(3),
                ratatui::layout::Constraint::Min(10),
                ratatui::layout::Constraint::Length(1),
            ])
            .split(area);

        chrome::render_toolbar(frame, rows[0], app, state, palette);
        browser::render_body(frame, rows[1], app, state, palette);
        chrome::render_status(frame, rows[2], app, palette);
    } else {
        let rows = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Min(10),
                ratatui::layout::Constraint::Length(1),
            ])
            .split(area);

        browser::render_body(frame, rows[0], app, state, palette);
        chrome::render_status(frame, rows[1], app, palette);
    }

    if app.search_is_open() {
        overlays::render_search_overlay(frame, area, app, state, palette);
    } else if app.help_open {
        overlays::render_help(frame, area, palette);
    }
}
