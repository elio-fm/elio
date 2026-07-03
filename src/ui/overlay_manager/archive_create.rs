use super::{edit_overlay_visible_rows, scrollbar::render_overlay_scrollbar_on_bg};
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

pub(super) fn render_archive_create_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let item_count = app.archive_create_source_names().len();
    let visible_lines = edit_overlay_visible_rows(area, item_count, 8);
    let popup_width = area.width.saturating_sub(8).clamp(40, 68);
    let max_height = (visible_lines + 8).min(area.height.max(4));
    let inner_height = max_height.saturating_sub(2);
    let footer_height = u16::from(inner_height >= 4);
    let content_lines = inner_height
        .saturating_sub(3 + footer_height + 2)
        .min(visible_lines);
    let content_height = if content_lines > 0 {
        content_lines + 2
    } else {
        0
    };
    let popup_height = (3 + content_height + footer_height).saturating_add(2);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height) / 2,
        width: popup_width.min(area.width),
        height: popup_height,
    };
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

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(content_height),
            Constraint::Length(footer_height),
        ])
        .split(inner);

    render_name_input(frame, rows[0], app, palette);
    if content_lines > 0 {
        render_contents_list(frame, rows[1], app, state, palette, content_lines as usize);
    }
    if footer_height > 0 {
        render_protection_row(frame, rows[2], app, palette);
    }
}

fn render_protection_row(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
    if let Some(error) = app.archive_create_error() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                helpers::clamp_label(error, area.width as usize),
                Style::default().fg(palette.accent),
            )]))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            area,
        );
        return;
    }

    let label = app.archive_create_protection_label();
    let hint = app.archive_create_protection_hint();
    let hint_width = hint.chars().count() as u16;
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(hint_width)])
        .split(area);

    frame.render_widget(
        Paragraph::new(label).style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
        columns[0],
    );
    if !hint.is_empty() {
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().bg(palette.chrome_alt).fg(palette.muted)),
            columns[1],
        );
    }
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
    let scrollbar_area = show_scrollbar.then_some(Rect {
        x: list_area.x + list_area.width.saturating_sub(1),
        width: 1,
        ..list_area
    });
    let scroll_top = app.archive_create_source_scroll(visible_lines);
    let row_width = list_area
        .width
        .saturating_sub(if show_scrollbar { 1 } else { 0 });

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

    if let Some(scrollbar_area) = scrollbar_area {
        render_overlay_scrollbar_on_bg(
            frame,
            scrollbar_area,
            source_names.len(),
            visible_lines,
            scroll_top,
            palette,
            palette.path_bg,
        );
    }
}
