use crate::app::{App, CopyHit, FrameState};
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

pub(super) fn render_copy_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let row_count = app.copy_row_count().max(1);
    let popup_width = area.width.saturating_sub(8).clamp(56, 104);
    let popup_height = 5;
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height + 2),
        width: popup_width.min(area.width.saturating_sub(2)).max(10),
        height: popup_height.min(area.height.saturating_sub(2)).max(4),
    };
    state.copy_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .title(Span::styled(
                format!(" {} ", app.copy_title()),
                Style::default().fg(palette.muted),
            ))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .border_style(Style::default().fg(palette.border)),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let action_row = rows[1];
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(2),
            Constraint::Percentage(25),
            Constraint::Length(2),
            Constraint::Percentage(25),
            Constraint::Length(2),
            Constraint::Percentage(25),
        ])
        .split(action_row);

    for index in 0..row_count.min(4) {
        let rect = columns[index * 2];
        let shortcut = app
            .copy_row_shortcut(index)
            .unwrap_or('?')
            .to_ascii_lowercase();
        let label = helpers::clamp_label(
            display_label(shortcut, app.copy_row_label(index)),
            rect.width.saturating_sub(5) as usize,
        );

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(shortcut.to_string(), Style::default().fg(palette.accent)),
                Span::styled(" -> ", Style::default().fg(palette.muted)),
                Span::styled(label, Style::default().fg(palette.text)),
            ]))
            .alignment(Alignment::Center)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            rect,
        );

        state.copy_hits.push(CopyHit {
            rect: Rect {
                x: rect.x,
                y: inner.y,
                width: rect.width,
                height: inner.height,
            },
            index,
        });
    }
}

fn display_label(shortcut: char, fallback: &str) -> &str {
    match shortcut {
        'c' => "file name",
        'n' => "name w/o ext",
        'p' => "file path",
        'd' => "directory path",
        _ => fallback,
    }
}
