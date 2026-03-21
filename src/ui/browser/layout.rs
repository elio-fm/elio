use super::super::theme::Palette;
use super::entries::render_entries;
use super::preview::render_preview;
use super::sidebar::render_sidebar;
use crate::app::{App, FrameState};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

pub(in crate::ui) fn render_body(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let columns = if area.width >= 126 {
        let outer = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(80)])
            .split(area);
        let content = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(54), Constraint::Percentage(46)])
            .split(outer[1]);
        vec![outer[0], content[0], content[1]]
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(22), Constraint::Min(42)])
            .split(area)
            .to_vec()
    };

    render_sidebar(frame, columns[0], app, state, palette);

    if columns.len() == 3 {
        render_entries(frame, columns[1], app, state, palette);
        render_preview(frame, columns[2], app, state, palette);
    } else {
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(12), Constraint::Length(11)])
            .split(columns[1]);
        render_entries(frame, right[0], app, state, palette);
        render_preview(frame, right[1], app, state, palette);
    }
}
