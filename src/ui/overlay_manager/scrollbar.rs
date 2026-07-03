use crate::ui::theme::Palette;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub(super) fn render_overlay_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    total_rows: usize,
    visible_rows: usize,
    scroll_row: usize,
    palette: Palette,
) {
    render_overlay_scrollbar_on_bg(
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
    bg: Color,
) {
    if area.height == 0 || total_rows <= visible_rows.max(1) {
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(bg).fg(palette.border)),
            area,
        );
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
        .style(Style::default().bg(bg)),
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
        .style(Style::default().bg(bg)),
        Rect {
            x: area.x,
            y: area.y + thumb_top as u16,
            width: area.width,
            height: thumb_height as u16,
        },
    );
}
