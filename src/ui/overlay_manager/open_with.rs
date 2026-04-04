use crate::app::{App, FrameState, OpenWithHit};
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

pub(super) fn render_open_with_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let row_count = app.open_with_row_count().max(1);
    // border (2) + padding top/bottom (2) + one line per row
    let popup_height = (row_count as u16 + 4)
        .min(area.height.saturating_sub(2))
        .max(4);
    let popup_width = area.width.saturating_sub(8).clamp(48, 72);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height + 2),
        width: popup_width.min(area.width.saturating_sub(2)).max(10),
        height: popup_height,
    };
    state.open_with_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .title(Span::styled(
                format!(" {} ", app.open_with_title()),
                Style::default().fg(palette.muted),
            ))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .border_style(Style::default().fg(palette.border)),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);

    for index in 0..app.open_with_row_count() {
        let rect = Rect {
            x: inner.x,
            y: inner.y + index as u16,
            width: inner.width,
            height: 1,
        };
        if rect.y >= inner.y + inner.height {
            break;
        }

        let shortcut = app
            .open_with_row_shortcut(index)
            .unwrap_or('?')
            .to_ascii_lowercase();
        let label = helpers::clamp_label(
            app.open_with_row_label(index),
            inner.width.saturating_sub(6) as usize,
        );

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(shortcut.to_string(), Style::default().fg(palette.accent)),
                Span::styled(" -> ", Style::default().fg(palette.muted)),
                Span::styled(label, Style::default().fg(palette.text)),
            ]))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            rect,
        );

        state.open_with_hits.push(OpenWithHit { rect, index });
    }
}
