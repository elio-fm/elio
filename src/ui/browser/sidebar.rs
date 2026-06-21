use super::super::{helpers, theme::Palette};
use crate::app::{App, FrameState, PathHit, SidebarRow};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

const ICON_ONLY_SIDEBAR_WIDTH: u16 = 5;

pub(super) fn render_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let inner = helpers::inner_with_padding(area);
    let icon_only = area.width <= ICON_ONLY_SIDEBAR_WIDTH;
    let title = if icon_only { "" } else { " Places " };
    let block = helpers::panel_block(title, palette.panel, palette);
    frame.render_widget(block, area);
    helpers::fill_area(frame, inner, palette.panel, palette.text);
    let mut y = inner.y;
    let row_height = 1u16;
    for item in &app.navigation.sidebar {
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
                if icon_only {
                    continue;
                }
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
                let active = helpers::path_is_active(&app.navigation.cwd, &item.identity_path);
                let bg = if active {
                    palette.sidebar_active
                } else {
                    palette.panel
                };
                let title_width = row.width.saturating_sub(
                    1u16.saturating_add(helpers::display_width(item.icon.as_str()) as u16)
                        .saturating_add(2),
                ) as usize;
                let mut spans = vec![
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
                ];
                if !icon_only {
                    spans.extend([
                        Span::raw("  "),
                        Span::styled(
                            helpers::clamp_label(&item.title, title_width),
                            Style::default()
                                .fg(palette.text)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]);
                }
                let top_line = Line::from(spans);
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
