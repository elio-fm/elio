use crate::{
    app::{App, ScreenRegions},
    theme::{self, Palette},
    ui::helpers,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
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
    let overwriting = save.overwrite();
    let popup_width = if overwriting {
        area.width.saturating_sub(8).clamp(40, 60)
    } else {
        area.width.saturating_sub(8).clamp(36, 64)
    };
    let popup = helpers::centered_rect(area, popup_width, 6);
    state.save_as_panel = Some(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(
            if overwriting && popup.width < 28 {
                " Overwrite? "
            } else if overwriting {
                " Overwrite existing file? "
            } else {
                " Save as "
            },
            palette.chrome_alt,
            palette,
        ),
        popup,
    );
    let inner = helpers::inner_with_padding(popup);
    if overwriting {
        let path = save.overwrite_path();
        let filename = path
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| save.input().to_string());
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(1)])
            .split(inner);
        frame.render_widget(
            helpers::rounded_block(palette.path_bg, palette.border),
            rows[0],
        );
        let filename_area = rows[0].inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        let (icon, icon_color) = path
            .map(|path| {
                (
                    theme::path_symbol(path, false),
                    theme::path_color(path, false, palette),
                )
            })
            .unwrap_or(("󰈔", palette.muted));
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    icon,
                    Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    helpers::clamp_label(&filename, filename_area.width.saturating_sub(2) as usize),
                    Style::default().fg(palette.muted),
                ),
            ]))
            .style(Style::default().bg(palette.path_bg)),
            filename_area,
        );
        let (confirm, cancel) =
            render_confirmation_buttons(frame, rows[1], save.overwrite_confirmed(), palette);
        state.save_as_confirm_btn = Some(confirm);
        state.save_as_cancel_btn = Some(cancel);
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

fn render_confirmation_buttons(
    frame: &mut Frame<'_>,
    area: Rect,
    confirm_selected: bool,
    palette: Palette,
) -> (Rect, Rect) {
    let confirm_style = if confirm_selected {
        Style::default()
            .bg(palette.selected_bg)
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(palette.chrome_alt).fg(palette.muted)
    };
    let cancel_style = if !confirm_selected {
        Style::default()
            .bg(palette.selected_bg)
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(palette.chrome_alt).fg(palette.muted)
    };
    let total_width = 24;
    if area.width >= total_width {
        let left_pad = area.width.saturating_sub(total_width) / 2;
        let confirm = Rect {
            x: area.x + left_pad,
            y: area.y,
            width: 11,
            height: 1,
        };
        let cancel = Rect {
            x: confirm.x + confirm.width + 3,
            y: area.y,
            width: 10,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " ".repeat(left_pad as usize),
                    Style::default().bg(palette.chrome_alt),
                ),
                Span::styled("  Confirm  ", confirm_style),
                Span::styled("   ", Style::default().bg(palette.chrome_alt)),
                Span::styled("  Cancel  ", cancel_style),
            ]))
            .style(Style::default().bg(palette.chrome_alt)),
            area,
        );
        (confirm, cancel)
    } else {
        let confirm = Rect {
            width: area.width / 2,
            ..area
        };
        let cancel = Rect {
            x: confirm.x + confirm.width,
            width: area.width.saturating_sub(confirm.width),
            ..confirm
        };
        frame.render_widget(
            Paragraph::new(helpers::clamp_label("Confirm", confirm.width as usize))
                .alignment(Alignment::Center)
                .style(confirm_style),
            confirm,
        );
        frame.render_widget(
            Paragraph::new(helpers::clamp_label("Cancel", cancel.width as usize))
                .alignment(Alignment::Center)
                .style(cancel_style),
            cancel,
        );
        (confirm, cancel)
    }
}
