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

const MAX_VISIBLE_CONTENTS: usize = 8;

pub(super) fn render_archive_create_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let item_count = app.archive_create_source_names().len();
    let visible_lines = item_count.min(MAX_VISIBLE_CONTENTS) as u16;
    let has_error = app.archive_create_error().is_some();
    let popup_width = area.width.saturating_sub(8).clamp(40, 68);
    let popup_height = visible_lines + 7 + u16::from(has_error);
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.archive_create_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(
            &format!(" {} ", app.archive_create_title()),
            palette.chrome_alt,
            palette,
        ),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let error_height = u16::from(has_error);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(error_height),
            Constraint::Length(visible_lines + 2),
        ])
        .split(inner);

    render_name_input(frame, rows[0], app, palette);

    if let Some(error) = app.archive_create_error() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                helpers::clamp_label(error, rows[1].width.saturating_sub(2) as usize),
                Style::default().fg(palette.accent),
            )]))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            rows[1],
        );
    }

    render_contents_list(frame, rows[2], app, state, palette, visible_lines as usize);
}

fn render_name_input(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        area,
    );
    let input_area = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    let input = app.archive_create_input();
    let cursor_col = app.archive_create_cursor_col();
    let (visible_text, visible_cursor_col) =
        helpers::input_window(input, cursor_col, input_area.width);

    let line = if input.is_empty() {
        Line::from(Span::styled(
            "archive.zip",
            Style::default().fg(palette.muted),
        ))
    } else {
        Line::from(Span::styled(
            visible_text,
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ))
    };
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(palette.path_bg).fg(palette.text)),
        input_area,
    );

    let cursor_x =
        (input_area.x + visible_cursor_col).min(input_area.x + input_area.width.saturating_sub(1));
    frame.set_cursor_position((cursor_x, input_area.y));
}

fn render_contents_list(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
    visible_lines: usize,
) {
    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        area,
    );
    let list_area = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    state.archive_create_list_area = Some(list_area);
    let source_names = app.archive_create_source_names();
    let show_scrollbar = source_names.len() > visible_lines;
    let scroll_top = app.archive_create_source_scroll(visible_lines);
    let row_width = list_area
        .width
        .saturating_sub(if show_scrollbar { 2 } else { 0 });

    for (row_offset, name) in source_names
        .iter()
        .skip(scroll_top)
        .take(visible_lines)
        .enumerate()
    {
        let is_dir = name.ends_with('/');
        let live_path = app.navigation.cwd.join(name.trim_end_matches('/'));
        let (icon, icon_color) = (
            theme::path_symbol(&live_path, is_dir),
            theme::path_color(&live_path, is_dir, palette),
        );
        let prefix_width = helpers::display_width(icon).saturating_add(2) as u16;
        let text_width = row_width.saturating_sub(prefix_width);
        let visible_text = helpers::clamp_label(name, text_width as usize);
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
                Span::styled(visible_text, Style::default().fg(palette.text)),
            ]))
            .style(Style::default().bg(palette.path_bg).fg(palette.text)),
            row_rect,
        );
    }

    if show_scrollbar {
        let bar_x = list_area.x + list_area.width.saturating_sub(1);
        let thumb_size = (visible_lines * visible_lines / source_names.len()).max(1);
        let max_scroll = source_names.len().saturating_sub(visible_lines);
        let thumb_top = if max_scroll == 0 || visible_lines <= thumb_size {
            0
        } else {
            scroll_top * (visible_lines - thumb_size) / max_scroll
        };
        for row_offset in 0..visible_lines {
            let in_thumb = row_offset >= thumb_top && row_offset < thumb_top + thumb_size;
            let bar_char = if in_thumb { "▐" } else { " " };
            let bar_color = if in_thumb {
                palette.muted
            } else {
                palette.path_bg
            };
            frame.buffer_mut()[(bar_x, list_area.y + row_offset as u16)].set_symbol(bar_char);
            frame.buffer_mut()[(bar_x, list_area.y + row_offset as u16)]
                .set_style(Style::default().bg(palette.path_bg).fg(bar_color));
        }
    }
}
