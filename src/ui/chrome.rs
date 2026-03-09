use super::helpers;
use super::theme::Palette;
use crate::app::{App, FrameState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub(super) fn render_toolbar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let block = Block::default()
        .style(Style::default().bg(palette.chrome).fg(palette.text))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(palette.border));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let control_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(34),
            Constraint::Min(2),
            Constraint::Length(39),
        ])
        .split(inner);
    let nav_buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(7),
            Constraint::Length(11),
        ])
        .split(control_row[0]);
    let meta = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),
            Constraint::Length(13),
            Constraint::Length(10),
        ])
        .split(control_row[2]);

    state.back_button = Some(nav_buttons[0]);
    state.forward_button = Some(nav_buttons[1]);
    state.parent_button = Some(nav_buttons[2]);
    state.refresh_button = Some(nav_buttons[3]);
    state.hidden_button = Some(meta[1]);
    state.view_button = Some(meta[2]);

    helpers::render_button(
        frame,
        nav_buttons[0],
        "Back",
        "󰁍",
        app.can_go_back(),
        palette,
    );
    helpers::render_button(
        frame,
        nav_buttons[1],
        "Next",
        "󰁔",
        app.can_go_forward(),
        palette,
    );
    helpers::render_button(frame, nav_buttons[2], "Up", "󰁝", true, palette);
    helpers::render_button(frame, nav_buttons[3], "Refresh", "󰑐", true, palette);
    frame.render_widget(
        Paragraph::new(Line::from(vec![helpers::chip_span(
            &format!("Sort: {}", app.sort_mode.label()),
            palette.button_bg,
            palette.text,
            true,
        )]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(palette.chrome).fg(palette.text)),
        meta[0],
    );
    helpers::render_button(
        frame,
        meta[1],
        if app.show_hidden {
            "Hidden On"
        } else {
            "Hidden Off"
        },
        "󰈉",
        true,
        palette,
    );
    helpers::render_button(frame, meta[2], app.view_mode.label(), "󰕮", true, palette);
}

pub(super) fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(24), Constraint::Length(34)])
        .split(area);

    let right_text = if app.status_message().is_empty() {
        "f folders  Ctrl+F files  ? help".to_string()
    } else {
        helpers::truncate_middle(app.status_message(), sections[1].width as usize)
    };
    frame.render_widget(
        Paragraph::new(app.selection_summary()).style(
            Style::default()
                .bg(palette.chrome)
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ),
        sections[0],
    );
    frame.render_widget(
        Paragraph::new(right_text)
            .alignment(Alignment::Right)
            .style(Style::default().bg(palette.chrome).fg(palette.muted)),
        sections[1],
    );
}
