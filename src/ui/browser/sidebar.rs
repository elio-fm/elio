use super::super::{helpers, theme::Palette};
use crate::app::{App, FrameState, PathHit, SidebarRow};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub(super) fn render_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let block = helpers::panel_block(" Places ", palette.panel, palette);
    frame.render_widget(block, area);
    let inner = helpers::inner_with_padding(area);
    helpers::fill_area(frame, inner, palette.panel, palette.text);
    let mut y = inner.y;
    let row_height = 1u16;
    for item in &app.sidebar {
        if y.saturating_add(row_height) > inner.y.saturating_add(inner.height) {
            break;
        }
        let row = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: row_height,
        };
        match item {
            SidebarRow::Section { title } => {
                let title_width = row.width.saturating_sub(1) as usize;
                let line = Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        helpers::clamp_label(title, title_width),
                        Style::default()
                            .fg(palette.muted)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]);
                frame.render_widget(
                    Paragraph::new(vec![line])
                        .style(Style::default().bg(palette.panel).fg(palette.muted)),
                    row,
                );
            }
            SidebarRow::Item(item) => {
                let active = helpers::path_is_active(&app.cwd, &item.path);
                let bg = if active {
                    palette.sidebar_active
                } else {
                    palette.panel
                };
                let title_width = row.width.saturating_sub(
                    1u16.saturating_add(helpers::display_width(item.icon.as_str()) as u16)
                        .saturating_add(2),
                ) as usize;
                let top_line = Line::from(vec![
                    Span::styled(
                        if active { "▌" } else { " " },
                        Style::default().fg(if active { palette.accent } else { bg }),
                    ),
                    Span::styled(
                        item.icon.as_str(),
                        Style::default()
                            .fg(palette.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        helpers::clamp_label(&item.title, title_width),
                        Style::default()
                            .fg(palette.text)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]);
                frame.render_widget(
                    Paragraph::new(vec![top_line]).style(Style::default().bg(bg).fg(palette.text)),
                    row,
                );
                state.sidebar_hits.push(PathHit {
                    rect: row,
                    path: item.path.clone(),
                });
            }
        }
        y = y.saturating_add(row_height);
    }
}
