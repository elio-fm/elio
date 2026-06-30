use crate::app::{App, FrameState};
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
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
        " Password for \"{}\" ",
        helpers::clamp_label(archive_name, 30)
    );
    let popup_width = area.width.saturating_sub(8).clamp(40, 64);
    let popup_height = 6u16;
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.archive_password_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(&block_title, palette.chrome_alt, palette),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(1)])
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
    let masked_input = "*".repeat(input.chars().count());
    let (visible_text, visible_cursor_col) =
        helpers::input_window(&masked_input, cursor_col, input_area.width);

    let line = if input.is_empty() {
        Line::from(Span::styled(
            "password…",
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

    if let Some(error) = app.archive_password_error() {
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
