use super::compute_scroll_top;
use crate::app::{App, FrameState};
use crate::ui::{
    helpers,
    theme::{self, Palette},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

pub(super) fn render_create_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let line_count = app.create_line_count().max(1);
    let visible_lines = line_count.min(8) as u16;
    let popup_width = area.width.saturating_sub(8).clamp(36, 64);
    let popup_height = visible_lines + 6;
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.create_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(
            &format!(" {} ", app.create_title()),
            palette.chrome_alt,
            palette,
        ),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(visible_lines + 2),
            Constraint::Length(1),
        ])
        .split(inner);

    let header_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(18)])
        .split(rows[0]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("󰜄", Style::default().fg(palette.accent)),
            Span::raw("  "),
            Span::styled("/ → folder", Style::default().fg(palette.muted)),
        ]))
        .style(Style::default().bg(palette.chrome_alt)),
        header_cols[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Alt+Enter", Style::default().fg(palette.text)),
            Span::styled(" new line", Style::default().fg(palette.muted)),
        ]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(palette.chrome_alt)),
        header_cols[1],
    );

    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[1],
    );
    let list_area = rows[1].inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    let cursor_line = app.create_cursor_line();
    let cursor_col = app.create_cursor_col();
    let scroll_top = compute_scroll_top(cursor_line, visible_lines as usize);
    state.create_list_area = Some(list_area);
    state.create_scroll_top = scroll_top;

    let show_scrollbar = line_count > visible_lines as usize;
    let thumb_size = if show_scrollbar {
        (visible_lines as usize * visible_lines as usize / line_count).max(1)
    } else {
        0
    };
    let max_scroll = line_count.saturating_sub(visible_lines as usize);
    let thumb_pos = if max_scroll == 0 {
        0
    } else {
        scroll_top * (visible_lines as usize - thumb_size) / max_scroll
    };
    let bar_x = list_area.x + list_area.width.saturating_sub(1);

    let mut cursor_screen_pos: Option<(u16, u16)> = None;

    for row_offset in 0..visible_lines as usize {
        let line_idx = scroll_top + row_offset;
        if line_idx >= line_count {
            break;
        }
        let line_text = app.create_line(line_idx);
        let is_cursor_line = line_idx == cursor_line;

        let is_dir = line_text.starts_with('/') || line_text.ends_with('/');
        let clean_name = line_text.trim_matches('/');
        let (icon, icon_color) = if clean_name.is_empty() {
            if is_dir {
                ("󰉋", palette.accent)
            } else {
                ("󰈔", palette.muted)
            }
        } else {
            let path = app.navigation.cwd.join(clean_name);
            (
                theme::path_symbol(&path, is_dir),
                theme::path_color(&path, is_dir, palette),
            )
        };

        let text_width = list_area
            .width
            .saturating_sub(3)
            .saturating_sub(if show_scrollbar { 2 } else { 0 }) as usize;
        let chars: Vec<char> = line_text.chars().collect();
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

        let line_widget = if line_text.is_empty() && is_cursor_line {
            Line::from(vec![
                Span::styled(
                    icon,
                    Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled("name…", Style::default().fg(palette.muted)),
            ])
        } else {
            Line::from(vec![
                Span::styled(
                    icon,
                    Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(visible_text, text_style),
            ])
        };

        let row_rect = Rect {
            x: list_area.x,
            y: list_area.y + row_offset as u16,
            width: list_area
                .width
                .saturating_sub(if show_scrollbar { 2 } else { 0 }),
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(line_widget)
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
            let cursor_x = row_rect.x + 3 + visible_col as u16;
            let cursor_x = cursor_x.min(row_rect.x + row_rect.width.saturating_sub(1));
            cursor_screen_pos = Some((cursor_x, row_rect.y));
        }
    }

    if let Some((cx, cy)) = cursor_screen_pos {
        frame.set_cursor_position((cx, cy));
    }

    if let Some(error) = app.create_line_error(cursor_line) {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                helpers::clamp_label(error, rows[2].width.saturating_sub(2) as usize),
                Style::default().fg(palette.accent),
            )]))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            rows[2],
        );
    }
}
