use crate::app::{App, FrameState, OpenWithHit};
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

pub(super) fn render_open_with_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let row_count = app.open_with_row_count().max(1);
    // border (2) + one line per row
    let popup_height = (row_count as u16 + 2)
        .min(area.height.saturating_sub(2))
        .max(3);
    let popup_width = area.width.saturating_sub(8).clamp(48, 72);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height + 2),
        width: popup_width.min(area.width.saturating_sub(2)).max(10),
        height: popup_height,
    };
    state.open_with_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .title(Span::styled(
                format!(" {} ", app.open_with_title()),
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
    let row_count = app.open_with_row_count();
    let needs_scrollbar = row_count > inner.height as usize;
    let (rows_area, scrollbar_area) = if needs_scrollbar && inner.width >= 6 {
        (
            Rect {
                width: inner.width.saturating_sub(1),
                ..inner
            },
            Some(Rect {
                x: inner.x + inner.width.saturating_sub(1),
                width: 1,
                ..inner
            }),
        )
    } else {
        (inner, None)
    };
    let visible_rows = rows_area.height as usize;
    let scroll_top = open_with_scroll_top(app.open_with_selected_index(), row_count, visible_rows);

    for (visible_index, index) in (scroll_top..row_count).enumerate() {
        let rect = Rect {
            x: rows_area.x,
            y: rows_area.y + visible_index as u16,
            width: rows_area.width,
            height: 1,
        };
        if rect.y >= rows_area.y + rows_area.height {
            break;
        }

        let shortcut = app
            .open_with_row_shortcut(index)
            .map(|shortcut| shortcut.to_ascii_lowercase().to_string())
            .unwrap_or_else(|| " ".to_string());
        let label = helpers::clamp_label(
            app.open_with_row_label(index),
            rows_area.width.saturating_sub(6) as usize,
        );
        let selected = index == app.open_with_selected_index();
        let row_style = if selected {
            Style::default().bg(palette.selected_bg).fg(palette.text)
        } else {
            Style::default().bg(palette.chrome_alt).fg(palette.text)
        };
        let shortcut_style = if selected {
            Style::default().fg(palette.accent_text)
        } else {
            Style::default().fg(palette.accent)
        };
        let arrow_style = if selected {
            Style::default().fg(palette.text)
        } else {
            Style::default().fg(palette.muted)
        };

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(shortcut, shortcut_style),
                Span::styled(" -> ", arrow_style),
                Span::styled(label, Style::default().fg(palette.text)),
            ]))
            .style(row_style),
            rect,
        );

        state.open_with_hits.push(OpenWithHit { rect, index });
    }

    if let Some(scrollbar) = scrollbar_area {
        render_open_with_scrollbar(
            frame,
            scrollbar,
            row_count,
            visible_rows,
            scroll_top,
            palette,
        );
    }
}

fn render_open_with_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    total_rows: usize,
    visible_rows: usize,
    scroll_top: usize,
    palette: Palette,
) {
    if area.height == 0 || total_rows <= visible_rows.max(1) {
        return;
    }

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "│",
                Style::default().fg(palette.border)
            ));
            area.height as usize
        ])
        .style(Style::default().bg(palette.chrome_alt)),
        area,
    );

    let thumb_height = ((visible_rows.max(1) * area.height as usize) / total_rows)
        .max(1)
        .min(area.height as usize);
    let max_scroll = total_rows.saturating_sub(visible_rows.max(1));
    let thumb_max_top = area.height as usize - thumb_height;
    let thumb_top = scroll_top
        .checked_mul(thumb_max_top)
        .and_then(|offset| offset.checked_div(max_scroll))
        .unwrap_or(0);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "┃",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            thumb_height
        ])
        .style(Style::default().bg(palette.chrome_alt)),
        Rect {
            x: area.x,
            y: area.y + thumb_top as u16,
            width: area.width,
            height: thumb_height as u16,
        },
    );
}

fn open_with_scroll_top(selected: usize, row_count: usize, visible_rows: usize) -> usize {
    if visible_rows == 0 || row_count <= visible_rows {
        return 0;
    }

    selected
        .saturating_add(1)
        .saturating_sub(visible_rows)
        .min(row_count.saturating_sub(visible_rows))
}
