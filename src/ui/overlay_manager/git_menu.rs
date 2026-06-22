use crate::app::{App, FrameState, GoToHit};
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

pub(super) fn render_git_menu_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let row_count = app.git_menu_row_count();
    let popup_width = area.width.saturating_sub(8).clamp(40, 80);
    let popup_height = (row_count as u16).saturating_mul(2).saturating_add(4);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height + 2),
        width: popup_width.min(area.width.saturating_sub(2)).max(10),
        height: popup_height.min(area.height.saturating_sub(2)).max(4),
    };
    state.git_menu_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .title(Span::styled(
                format!(" {} ", app.git_menu_title()),
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
    let available_rows = inner.height.saturating_sub(2) as usize;
    if available_rows == 0 || row_count == 0 {
        return;
    }

    let rendered_rows = row_count.min(available_rows.div_ceil(2));
    let mut vertical_constraints =
        Vec::with_capacity(rendered_rows.saturating_mul(2).saturating_sub(1));
    for row in 0..rendered_rows {
        if row > 0 {
            vertical_constraints.push(Constraint::Length(1));
        }
        vertical_constraints.push(Constraint::Length(1));
    }
    let layout_height = rendered_rows
        .saturating_mul(2)
        .saturating_sub(1)
        .min(available_rows) as u16;
    let row_rects = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vertical_constraints)
        .split(Rect {
            y: inner.y + 1,
            height: layout_height,
            ..inner
        });

    for row in 0..rendered_rows {
        render_git_menu_entry(frame, app, state, palette, row_rects[row * 2], row);
    }
}

fn render_git_menu_entry(
    frame: &mut Frame<'_>,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
    rect: Rect,
    index: usize,
) {
    let shortcut = app.git_menu_row_shortcut(index).unwrap_or('?');
    let label = helpers::clamp_label(
        app.git_menu_row_label(index),
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

    state.git_menu_hits.push(GoToHit { rect, index });
}
