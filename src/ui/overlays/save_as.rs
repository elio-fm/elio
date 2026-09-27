use crate::{
    app::{App, ScreenRegions},
    theme::{self, Palette},
    ui::helpers,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

pub(in crate::ui) fn render_save_as_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut ScreenRegions,
    palette: Palette,
) {
    let Some(save) = app.chooser.save_as() else {
        return;
    };
    let popup_width = area.width.saturating_sub(8).clamp(36, 64);
    let popup = helpers::centered_rect(area, popup_width, 6);
    state.save_as_panel = Some(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(" Save as ", palette.chrome_alt, palette),
        popup,
    );
    let inner = helpers::inner_with_padding(popup);
    if save.overwrite() {
        frame.render_widget(
            Paragraph::new(
                "A file with this name exists. Overwrite?\nEnter confirm  •  Esc cancel",
            )
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            inner,
        );
        return;
    }

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
    let (icon, icon_color) = if save.input().is_empty() {
        ("󰈔", palette.muted)
    } else {
        let path = app.file_browser.cwd.join(save.input());
        (
            theme::path_symbol(&path, false),
            theme::path_color(&path, false, palette),
        )
    };
    let prefix_width = helpers::display_width(icon).saturating_add(2) as u16;
    let (text, col) = helpers::input_window(
        save.input(),
        save.cursor_col(),
        input_area.width.saturating_sub(prefix_width),
    );
    let text = if save.input().is_empty() {
        Span::styled("name…", Style::default().fg(palette.muted))
    } else {
        Span::styled(
            text,
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                icon,
                Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            text,
        ]))
        .style(Style::default().bg(palette.path_bg).fg(palette.text)),
        input_area,
    );
    frame.set_cursor_position((
        (input_area.x + prefix_width + col).min(input_area.x + input_area.width.saturating_sub(1)),
        input_area.y,
    ));
    if let Some(error) = save.error() {
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
