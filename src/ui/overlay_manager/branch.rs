use super::compute_scroll_top;
use crate::app::{App, FrameState, GoToHit};
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

pub(super) fn render_branch_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let popup_width = area.width.saturating_sub(8).clamp(40, 72);
    let popup_height = area.height.saturating_sub(6).clamp(10, 20);
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.branch_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(" Switch branch ", palette.chrome_alt, palette),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(2),
            Constraint::Length(1),
        ])
        .split(inner);

    // Query input.
    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[0],
    );
    let query_area = rows[0].inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let query = app.branch_query();
    let query_line = if query.is_empty() {
        Line::from(Span::styled(
            "type to filter branches",
            Style::default().fg(palette.muted),
        ))
    } else {
        Line::from(Span::styled(
            query.to_string(),
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ))
    };
    frame.render_widget(
        Paragraph::new(query_line).style(Style::default().bg(palette.path_bg).fg(palette.text)),
        query_area,
    );
    let cursor_x = (query_area.x + app.branch_query_cursor() as u16)
        .min(query_area.x + query_area.width.saturating_sub(1));
    frame.set_cursor_position((cursor_x, query_area.y));

    // Branch list.
    let list_area = rows[1];
    let row_count = app.branch_row_count();
    if row_count == 0 {
        helpers::render_empty_state_with_bg(
            frame,
            list_area,
            "No matching branches",
            palette,
            palette.chrome_alt,
        );
    } else {
        let visible = list_area.height as usize;
        let selected = app.branch_selected_index();
        let scroll_top = compute_scroll_top(selected, visible);
        for offset in 0..visible.min(row_count.saturating_sub(scroll_top)) {
            let index = scroll_top + offset;
            let rect = Rect {
                x: list_area.x,
                y: list_area.y + offset as u16,
                width: list_area.width,
                height: 1,
            };
            let is_selected = index == selected;
            let is_current = app.branch_row_is_current(index);
            let bg = if is_selected {
                palette.selected_bg
            } else {
                palette.chrome_alt
            };
            let marker = if is_current { "● " } else { "  " };
            let marker_color = if is_current {
                palette.accent
            } else {
                palette.muted
            };
            let name_style = if is_selected {
                Style::default()
                    .fg(palette.text)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette.text)
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(marker, Style::default().fg(marker_color)),
                    Span::styled(
                        helpers::clamp_label(
                            app.branch_row_label(index),
                            rect.width.saturating_sub(3) as usize,
                        ),
                        name_style,
                    ),
                ]))
                .style(Style::default().bg(bg).fg(palette.text)),
                rect,
            );
            state.branch_hits.push(GoToHit { rect, index });
        }
    }

    frame.render_widget(
        Paragraph::new("Enter switch  Esc close  ↑↓ move")
            .alignment(Alignment::Right)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.muted)),
        rows[2],
    );
}
