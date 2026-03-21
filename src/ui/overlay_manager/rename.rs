use super::theme::{self, Palette};
use super::{App, FrameState, helpers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

pub(in crate::ui) fn render_rename_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let original = app.rename_original_name();
    let block_title = format!(" Rename \"{}\" ", helpers::clamp_label(original, 30));
    let popup_width = area.width.saturating_sub(8).clamp(40, 64);
    let popup_height = 6u16;
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.rename_panel = Some(popup);

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

    let input = app.rename_input();
    let cursor_col = app.rename_cursor_col();
    let chars: Vec<char> = input.chars().collect();
    let col = cursor_col.min(chars.len());
    let available = input_area.width.saturating_sub(3) as usize;
    let h_start = col.saturating_sub(available);

    let mut visible_text: String = chars.iter().skip(h_start).take(available).collect();
    if h_start > 0 && !visible_text.is_empty() {
        visible_text.remove(0);
        visible_text.insert(0, '…');
    }

    let is_dir = app.rename_item_is_dir();
    let live_path = app.cwd.join(app.rename_input());
    let (icon, icon_color) = (
        theme::path_symbol(&live_path, is_dir),
        theme::path_color(&live_path, is_dir, palette),
    );

    let line = if input.is_empty() {
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
            Span::styled(
                visible_text,
                Style::default()
                    .fg(palette.text)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    };
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(palette.path_bg).fg(palette.text)),
        input_area,
    );

    let visible_cursor_col = col.saturating_sub(h_start);
    let cursor_x = (input_area.x + 3 + visible_cursor_col as u16)
        .min(input_area.x + input_area.width.saturating_sub(1));
    frame.set_cursor_position((cursor_x, input_area.y));

    if let Some(error) = app.rename_error() {
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
