use crate::app::App;
use crate::theme::Palette;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
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
    render_scrollbar(
        frame,
        area,
        total,
        visible_rows,
        app.preview_scroll_offset(),
        palette,
        palette.panel,
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
    render_scrollbar(
        frame,
        area,
        total_rows,
        visible_rows,
        scroll_row,
        palette,
        palette.panel_alt,
    );
}

pub(super) fn render_overlay_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    total_rows: usize,
    visible_rows: usize,
    scroll_row: usize,
    palette: Palette,
) {
    render_scrollbar(
        frame,
        area,
        total_rows,
        visible_rows,
        scroll_row,
        palette,
        palette.chrome_alt,
    );
}

pub(super) fn render_overlay_scrollbar_on_bg(
    frame: &mut Frame<'_>,
    area: Rect,
    total_rows: usize,
    visible_rows: usize,
    scroll_row: usize,
    palette: Palette,
    background: Color,
) {
    render_scrollbar(
        frame,
        area,
        total_rows,
        visible_rows,
        scroll_row,
        palette,
        background,
    );
}

fn render_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    total_rows: usize,
    visible_rows: usize,
    scroll_row: usize,
    palette: Palette,
    background: Color,
) {
    if area.height == 0 || total_rows <= visible_rows.max(1) {
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(background).fg(palette.border)),
            area,
        );
        return;
    }

    let track = vec![
        Line::from(Span::styled("│", Style::default().fg(palette.border)));
        area.height as usize
    ];
    frame.render_widget(
        Paragraph::new(track).style(Style::default().bg(background)),
        area,
    );

    let thumb_height = ((visible_rows.max(1) * area.height as usize) / total_rows)
        .max(1)
        .min(area.height as usize);
    let max_scroll = total_rows.saturating_sub(visible_rows.max(1));
    let thumb_max_top = area.height as usize - thumb_height;
    let thumb_top = scroll_row
        .checked_mul(thumb_max_top)
        .and_then(|offset| offset.checked_div(max_scroll))
        .unwrap_or(0);

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
        .style(Style::default().bg(background)),
        thumb,
    );
}

const MAX_EDIT_OVERLAY_VISIBLE_ROWS: usize = 12;

pub(super) fn scroll_top_for_cursor(cursor_line: usize, visible_rows: usize) -> usize {
    if cursor_line < visible_rows {
        0
    } else {
        cursor_line - visible_rows + 1
    }
}

pub(super) fn visible_edit_rows(area: Rect, row_count: usize, popup_chrome_height: u16) -> u16 {
    let available_rows = area
        .height
        .saturating_sub(popup_chrome_height.saturating_add(2))
        .max(1) as usize;
    row_count
        .clamp(1, MAX_EDIT_OVERLAY_VISIBLE_ROWS)
        .min(available_rows) as u16
}

#[cfg(test)]
#[path = "tests/scrollbars.rs"]
mod tests;
