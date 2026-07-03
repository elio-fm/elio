use super::{
    compute_scroll_top, edit_overlay_visible_rows, scrollbar::render_overlay_scrollbar_on_bg,
};
use crate::app::{App, FrameState};
use crate::ui::{
    helpers,
    theme::{self, Palette},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
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
    let visible_lines = edit_overlay_visible_rows(area, line_count, 5);
    let popup_width = area.width.saturating_sub(8).clamp(36, 64);
    let popup_height = visible_lines + 5;
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

    let cursor_line = app.create_cursor_line();
    let cursor_col = app.create_cursor_col();
    let scroll_top = compute_scroll_top(cursor_line, visible_lines as usize);
    state.create_list_area = Some(list_area);
    state.create_scroll_top = scroll_top;

    let show_scrollbar = line_count > visible_lines as usize;
    let scrollbar_area = show_scrollbar.then_some(Rect {
        x: list_area.x + list_area.width.saturating_sub(1),
        width: 1,
        ..list_area
    });

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

        let prefix_width = helpers::display_width(icon).saturating_add(2) as u16;
        let text_width = list_area
            .width
            .saturating_sub(prefix_width)
            .saturating_sub(if show_scrollbar { 1 } else { 0 });
        let col = if is_cursor_line { cursor_col } else { 0 };
        let (visible_text, visible_col) = helpers::input_window(line_text, col, text_width);

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
                .saturating_sub(if show_scrollbar { 1 } else { 0 }),
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(line_widget)
                .style(Style::default().bg(palette.path_bg).fg(palette.text)),
            row_rect,
        );

        if is_cursor_line {
            let cursor_x = row_rect.x + prefix_width + visible_col;
            let cursor_x = cursor_x.min(row_rect.x + row_rect.width.saturating_sub(1));
            cursor_screen_pos = Some((cursor_x, row_rect.y));
        }
    }

    if let Some((cx, cy)) = cursor_screen_pos {
        frame.set_cursor_position((cx, cy));
    }

    if let Some(scrollbar_area) = scrollbar_area {
        render_overlay_scrollbar_on_bg(
            frame,
            scrollbar_area,
            line_count,
            visible_lines as usize,
            scroll_top,
            palette,
            palette.path_bg,
        );
    }

    if let Some(error) = app.create_line_error(cursor_line) {
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
