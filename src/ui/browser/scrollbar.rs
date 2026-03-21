use super::super::theme::Palette;
use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub(super) fn render_preview_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    visible_rows: usize,
    visible_cols: usize,
    palette: Palette,
) {
    let total = app.preview_total_lines(visible_cols);
    if area.height == 0 || total <= visible_rows.max(1) {
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(palette.panel).fg(palette.border)),
            area,
        );
        return;
    }

    let track = vec![
        Line::from(Span::styled("│", Style::default().fg(palette.border),));
        area.height as usize
    ];
    frame.render_widget(
        Paragraph::new(track).style(Style::default().bg(palette.panel)),
        area,
    );

    let thumb_height = ((visible_rows.max(1) * area.height as usize) / total)
        .max(1)
        .min(area.height as usize);
    let max_scroll = total.saturating_sub(visible_rows.max(1));
    let thumb_max_top = area.height as usize - thumb_height;
    let thumb_top = if max_scroll == 0 {
        0
    } else {
        (app.preview_scroll_offset() * thumb_max_top) / max_scroll
    };

    let thumb = Rect {
        x: area.x,
        y: area.y + thumb_top as u16,
        width: area.width,
        height: thumb_height as u16,
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "┃",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            thumb.height as usize
        ])
        .style(Style::default().bg(palette.panel)),
        thumb,
    );
}

pub(super) fn split_scrollbar_area(area: Rect) -> (Rect, Option<Rect>) {
    if area.width >= 6 {
        let parts = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);
        (parts[0], Some(parts[1]))
    } else {
        (area, None)
    }
}

pub(super) fn render_browser_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    total_rows: usize,
    visible_rows: usize,
    scroll_row: usize,
    palette: Palette,
) {
    if area.height == 0 || total_rows <= visible_rows.max(1) {
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(palette.panel_alt).fg(palette.border)),
            area,
        );
        return;
    }

    let track = vec![
        Line::from(Span::styled("│", Style::default().fg(palette.border)));
        area.height as usize
    ];
    frame.render_widget(
        Paragraph::new(track).style(Style::default().bg(palette.panel_alt)),
        area,
    );

    let thumb_height = ((visible_rows.max(1) * area.height as usize) / total_rows)
        .max(1)
        .min(area.height as usize);
    let max_scroll = total_rows.saturating_sub(visible_rows.max(1));
    let thumb_max_top = area.height as usize - thumb_height;
    let thumb_top = if max_scroll == 0 {
        0
    } else {
        (scroll_row * thumb_max_top) / max_scroll
    };

    let thumb = Rect {
        x: area.x,
        y: area.y + thumb_top as u16,
        width: area.width,
        height: thumb_height as u16,
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "┃",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            thumb.height as usize
        ])
        .style(Style::default().bg(palette.panel_alt)),
        thumb,
    );
}
