use crate::app::{App, FrameState, GoToHit};
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

const MAX_GOTO_COLUMNS: usize = 5;

pub(super) fn render_goto_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let row_count = app.goto_row_count();
    let columns = goto_column_count(row_count);
    let visual_rows = row_count.div_ceil(columns).max(1);
    let popup_width = area.width.saturating_sub(8).clamp(70, 112);
    let row_gap_count = visual_rows.saturating_sub(1) as u16;
    let popup_height = (visual_rows as u16)
        .saturating_add(row_gap_count)
        .saturating_add(4);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height + 2),
        width: popup_width.min(area.width.saturating_sub(2)).max(10),
        height: popup_height.min(area.height.saturating_sub(2)).max(4),
    };
    state.goto_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .title(Span::styled(
                format!(" {} ", app.goto_title()),
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

    let rendered_rows = visual_rows.min(available_rows.div_ceil(2));
    let mut vertical_constraints =
        Vec::with_capacity(rendered_rows.saturating_mul(2).saturating_sub(1));
    for visual_row in 0..rendered_rows {
        if visual_row > 0 {
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

    let column_constraints = std::iter::repeat_n(Constraint::Fill(1), columns).collect::<Vec<_>>();
    for visual_row in 0..rendered_rows {
        let first_index = visual_row * columns;
        let row_items = columns.min(row_count.saturating_sub(first_index));
        let column_rects = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(column_constraints.clone())
            .split(row_rects[visual_row * 2]);
        for column in 0..row_items {
            let index = first_index + column;
            render_goto_entry(frame, app, state, palette, column_rects[column], index);
        }
    }
}

fn render_goto_entry(
    frame: &mut Frame<'_>,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
    rect: Rect,
    index: usize,
) {
    let shortcut = app.goto_row_shortcut(index).unwrap_or('?');
    let label = helpers::clamp_label(
        app.goto_row_label(index),
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

    state.goto_hits.push(GoToHit { rect, index });
}

fn goto_column_count(row_count: usize) -> usize {
    let max_columns = row_count.clamp(1, MAX_GOTO_COLUMNS);
    if row_count > 10 {
        return (1..=max_columns)
            .rev()
            .find(|&columns| row_count % columns != 1)
            .unwrap_or(1);
    }

    (1..=max_columns)
        .filter(|&columns| row_count == 1 || columns > 1)
        .filter(|&columns| row_count <= columns || row_count % columns != 1)
        .min_by_key(|&columns| {
            let rows = row_count.div_ceil(columns);
            (rows * columns - row_count, std::cmp::Reverse(columns))
        })
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::goto_column_count;

    #[test]
    fn goto_columns_avoid_single_item_last_row() {
        assert_eq!(goto_column_count(5), 5);
        assert_eq!(goto_column_count(6), 3);
        assert_eq!(goto_column_count(7), 4);
        assert_eq!(goto_column_count(8), 4);
        assert_eq!(goto_column_count(10), 5);
        assert_eq!(goto_column_count(11), 4);
        assert_eq!(goto_column_count(13), 5);
        assert_eq!(goto_column_count(14), 5);
    }
}
