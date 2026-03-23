use super::helpers;
use super::theme::Palette;
use crate::app::{App, ClipOp, FrameState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub(super) fn render_toolbar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    helpers::fill_area(frame, area, palette.chrome, palette.text);
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
            Constraint::Length(23),
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
    helpers::fill_area(frame, area, palette.chrome, palette.text);
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(24), Constraint::Length(34)])
        .split(area);

    let right_text = if app.status_message().is_empty() {
        "f folders  ^F files  ? help".to_string()
    } else {
        helpers::truncate_middle(app.status_message(), sections[1].width as usize)
    };
    let clip = app.clipboard_info();
    let sel_count = app.selection_count();

    // Build the left line: optional clipboard chip, optional selection chip,
    // then the path/position summary truncated to whatever space remains.
    let left_line = {
        let mut spans: Vec<Span<'_>> = Vec::new();
        let mut chips_width: u16 = 0;

        if let Some((clip_count, clip_op)) = clip {
            let (label, color) = match clip_op {
                ClipOp::Yank => (format!(" {clip_count} yanked "), palette.yank_bar),
                ClipOp::Cut => (format!(" {clip_count} cut "), palette.cut_bar),
            };
            chips_width += label.len() as u16 + 2;
            spans.push(Span::styled(
                label,
                Style::default()
                    .bg(color)
                    .fg(palette.chrome)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        }

        if sel_count > 0 {
            let chip = format!(" {sel_count} selected ");
            chips_width += chip.len() as u16 + 2;
            spans.push(Span::styled(
                chip,
                Style::default()
                    .bg(palette.selection_bar)
                    .fg(palette.chrome)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        }

        let summary_width = sections[0].width.saturating_sub(chips_width) as usize;
        spans.push(Span::styled(
            helpers::truncate_middle(&app.selection_summary(), summary_width),
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ));

        Line::from(spans)
    };
    frame.render_widget(
        Paragraph::new(left_line).style(Style::default().bg(palette.chrome)),
        sections[0],
    );
    frame.render_widget(
        Paragraph::new(right_text)
            .alignment(Alignment::Right)
            .style(Style::default().bg(palette.chrome).fg(palette.muted)),
        sections[1],
    );
}
