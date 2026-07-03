use crate::app::{App, FrameState};
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

pub(super) fn render_archive_password_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let archive_name = app.archive_password_archive_name();
    let block_title = format!(
        " {} \"{}\" ",
        app.archive_password_title_prefix(),
        helpers::clamp_label(&archive_name, 30)
    );
    let popup_width = area.width.saturating_sub(8).clamp(40, 64);
    let popup_height = if area.height >= 4 {
        6u16.min(area.height)
    } else {
        area.height
    };
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height) / 2,
        width: popup_width.min(area.width),
        height: popup_height,
    };
    state.archive_password_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(&block_title, palette.chrome_alt, palette),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let show_footer = inner.height >= 4;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(if show_footer { 1 } else { 0 }),
        ])
        .split(inner);

    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[0],
    );
    let input_area = rows[0].inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    let input = app.archive_password_input();
    let cursor_col = app.archive_password_cursor_col();
    let password_visible = app.archive_password_is_visible();
    let toggle_icon = if password_visible { "" } else { "" };
    let toggle_width = 3u16.min(input_area.width);
    let input_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(toggle_width)])
        .split(input_area);
    let text_area = input_columns[0];
    let toggle_area = input_columns[1];
    state.archive_password_visibility_btn = Some(toggle_area);

    let masked_input = "*".repeat(input.chars().count());
    let display_input = if password_visible {
        input
    } else {
        &masked_input
    };
    let (visible_text, visible_cursor_col) =
        helpers::input_window(display_input, cursor_col, text_area.width);

    let line = if input.is_empty() {
        Line::from(Span::styled(
            app.archive_password_placeholder(),
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
        text_area,
    );

    let toggle_style = Style::default()
        .bg(palette.button_bg)
        .fg(palette.text)
        .add_modifier(Modifier::BOLD);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(toggle_icon, toggle_style)))
            .alignment(Alignment::Center)
            .style(toggle_style),
        toggle_area,
    );

    let cursor_x =
        (text_area.x + visible_cursor_col).min(text_area.x + text_area.width.saturating_sub(1));
    frame.set_cursor_position((cursor_x, text_area.y));

    if !show_footer {
        return;
    }

    let hint_label = if password_visible {
        "Alt+V hide"
    } else {
        "Alt+V show"
    };
    let hint_width = hint_label.chars().count() as u16;
    let footer_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(hint_width)])
        .split(rows[1]);

    frame.render_widget(
        Paragraph::new(hint_label)
            .alignment(Alignment::Right)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.muted)),
        footer_columns[1],
    );

    if let Some(error) = app.archive_password_error() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                helpers::clamp_label(error, footer_columns[0].width.saturating_sub(1) as usize),
                Style::default().fg(palette.accent),
            )]))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            footer_columns[0],
        );
    }
}
