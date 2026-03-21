use super::theme::{self, Palette};
use super::{App, FrameState, compute_create_scroll_top, helpers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

pub(in crate::ui) fn render_bulk_rename_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let item_count = app.bulk_rename_item_count();
    let visible_lines = item_count.min(8) as u16;
    let popup_width = area.width.saturating_sub(8).clamp(40, 68);
    let popup_height = visible_lines + 5;
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.rename_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(
            &format!(" {} ", app.bulk_rename_title()),
            palette.chrome_alt,
            palette,
        ),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(visible_lines + 2), Constraint::Length(1)])
        .split(inner);

    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[0],
    );
    let list_area = rows[0].inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    let cursor_line = app.bulk_rename_cursor_line();
    let cursor_col = app.bulk_rename_cursor_col();

    let scroll_top = compute_create_scroll_top(cursor_line, item_count, visible_lines as usize);
    state.bulk_rename_list_area = Some(list_area);
    state.bulk_rename_scroll_top = scroll_top;

    let show_scrollbar = item_count > visible_lines as usize;
    let thumb_size = if show_scrollbar {
        (visible_lines as usize * visible_lines as usize / item_count).max(1)
    } else {
        0
    };
    let max_scroll = item_count.saturating_sub(visible_lines as usize);
    let thumb_pos = if max_scroll == 0 {
        0
    } else {
        scroll_top * (visible_lines as usize - thumb_size) / max_scroll
    };
    let bar_x = list_area.x + list_area.width.saturating_sub(1);

    let mut cursor_screen_pos: Option<(u16, u16)> = None;

    for row_offset in 0..visible_lines as usize {
        let line_idx = scroll_top + row_offset;
        if line_idx >= item_count {
            break;
        }

        let new_name = app.bulk_rename_new_name(line_idx);
        let is_dir = app.bulk_rename_item_is_dir(line_idx);
        let is_cursor_line = line_idx == cursor_line;

        let live_path = app.cwd.join(new_name);
        let (icon, icon_color) = (
            theme::path_symbol(&live_path, is_dir),
            theme::path_color(&live_path, is_dir, palette),
        );

        let text_width = list_area
            .width
            .saturating_sub(3)
            .saturating_sub(if show_scrollbar { 2 } else { 0 }) as usize;
        let chars: Vec<char> = new_name.chars().collect();
        let col = if is_cursor_line {
            cursor_col.min(chars.len())
        } else {
            0
        };
        let h_start = col.saturating_sub(text_width);
        let mut visible_text: String = chars.iter().skip(h_start).take(text_width).collect();
        if h_start > 0 && !visible_text.is_empty() {
            visible_text.remove(0);
            visible_text.insert(0, '…');
        }

        let text_style = if is_cursor_line {
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text)
        };

        let row_width = list_area
            .width
            .saturating_sub(if show_scrollbar { 2 } else { 0 });
        let row_rect = Rect {
            x: list_area.x,
            y: list_area.y + row_offset as u16,
            width: row_width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    icon,
                    Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(visible_text, text_style),
            ]))
            .style(Style::default().bg(palette.path_bg).fg(palette.text)),
            row_rect,
        );

        if show_scrollbar {
            let y = list_area.y + row_offset as u16;
            let in_thumb = row_offset >= thumb_pos && row_offset < thumb_pos + thumb_size;
            let bar_char = if in_thumb { "▐" } else { " " };
            let bar_color = if in_thumb {
                palette.muted
            } else {
                palette.path_bg
            };
            frame.buffer_mut()[(bar_x, y)].set_symbol(bar_char);
            frame.buffer_mut()[(bar_x, y)]
                .set_style(Style::default().bg(palette.path_bg).fg(bar_color));
        }

        if is_cursor_line {
            let visible_col = col.saturating_sub(h_start);
            let cursor_x = (row_rect.x + 3 + visible_col as u16)
                .min(row_rect.x + row_rect.width.saturating_sub(1));
            cursor_screen_pos = Some((cursor_x, row_rect.y));
        }
    }

    if let Some((cx, cy)) = cursor_screen_pos {
        frame.set_cursor_position((cx, cy));
    }

    if let Some(error) = app.bulk_rename_line_error(cursor_line) {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                helpers::clamp_label(error, rows[1].width.saturating_sub(2) as usize),
                Style::default().fg(palette.accent),
            )]))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            rows[1],
        );
    }
}
