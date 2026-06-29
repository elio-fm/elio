use super::scrollbar::render_overlay_scrollbar;
use crate::app::{App, FrameState, SearchHit, SearchScope};
use crate::ui::{
    helpers,
    theme::{self, Palette},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

pub(super) fn render_search_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let popup_width = area.width.saturating_sub(8).clamp(48, 88);
    let popup_height = area.height.saturating_sub(6).clamp(12, 22);
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.search_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(" Fuzzy Find ", palette.chrome_alt, palette),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .split(inner);

    let scope_label = app
        .search_scope()
        .map(|scope| scope.label())
        .unwrap_or("Search");
    let summary_prefix = format!("{scope_label}  ");
    let summary_width = usize::from(rows[0].width)
        .saturating_sub(UnicodeWidthStr::width(summary_prefix.as_str()))
        .saturating_sub(2);
    let summary = build_search_summary(
        app.search_is_loading(),
        app.search_index_is_limited(),
        app.search_match_count(),
        app.search_scanned_count(),
        summary_width,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                scope_label,
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            helpers::chip_span(&summary, palette.accent_soft, palette.accent_text, true),
        ]))
        .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
        rows[0],
    );

    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[1],
    );
    let query = if app.search_query().is_empty() {
        match app.search_scope() {
            Some(SearchScope::Folders) => "type to filter folders".to_string(),
            Some(SearchScope::Files) => "type to filter files".to_string(),
            None => "type to filter results".to_string(),
        }
    } else {
        app.search_query().to_string()
    };
    let query_style = if app.search_query().is_empty() {
        Style::default().fg(palette.muted)
    } else {
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    };
    let query_area = rows[1].inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let (query_line, cursor_x) = if app.search_query().is_empty() {
        (
            Line::from(vec![
                Span::styled("󰍉", Style::default().fg(palette.accent)),
                Span::raw("  "),
                Span::styled(query, query_style),
            ]),
            query_area.x.saturating_add(3),
        )
    } else {
        render_query_line(
            app.search_query(),
            app.search_query_cursor(),
            query_area.width,
            query_area.x,
            palette,
        )
    };
    frame.render_widget(
        Paragraph::new(query_line).style(Style::default().bg(palette.path_bg).fg(palette.text)),
        query_area,
    );
    frame.set_cursor_position((cursor_x, query_area.y));

    let row_height = 2u16;
    let visible_rows = (rows[2].height / row_height).max(1) as usize;
    let needs_scrollbar = app.search_match_count() > visible_rows;
    let (results_area, scrollbar_area) = if needs_scrollbar && rows[2].width >= 6 {
        (
            Rect {
                width: rows[2].width.saturating_sub(1),
                ..rows[2]
            },
            Some(Rect {
                x: rows[2].x + rows[2].width.saturating_sub(1),
                width: 1,
                ..rows[2]
            }),
        )
    } else {
        (rows[2], None)
    };
    state.search_rows_visible = visible_rows;

    let rows_data = app.search_rows(visible_rows);
    if app.search_is_loading() && rows_data.is_empty() {
        helpers::render_empty_state_with_bg(
            frame,
            results_area,
            "Scanning current tree…",
            palette,
            palette.chrome_alt,
        );
    } else if let Some(error) = app.search_error() {
        helpers::render_empty_state_with_bg(
            frame,
            results_area,
            &helpers::truncate_middle(error, results_area.width.saturating_sub(4) as usize),
            palette,
            palette.chrome_alt,
        );
    } else if rows_data.is_empty() {
        helpers::render_empty_state_with_bg(
            frame,
            results_area,
            app.search_scope()
                .map(|scope| scope.empty_label())
                .unwrap_or("No matches in this folder tree"),
            palette,
            palette.chrome_alt,
        );
    } else {
        for (offset, row) in rows_data.iter().enumerate() {
            let rect = Rect {
                x: results_area.x,
                y: results_area.y + offset as u16 * row_height,
                width: results_area.width,
                height: row_height.min(
                    results_area
                        .height
                        .saturating_sub(offset as u16 * row_height),
                ),
            };

            let bg = if row.selected {
                palette.selected_bg
            } else {
                palette.chrome_alt
            };
            frame.render_widget(Block::default().style(Style::default().bg(bg)), rect);
            if row.selected {
                frame.render_widget(
                    Paragraph::new("▎").style(Style::default().bg(bg).fg(palette.selected_border)),
                    Rect {
                        x: rect.x,
                        y: rect.y,
                        width: 1,
                        height: rect.height,
                    },
                );
            }

            let row_path = std::path::Path::new(&row.relative);
            let icon = theme::path_symbol_with_symlink(row_path, row.is_dir, row.symlink.as_ref());
            let icon_color =
                theme::path_color_with_symlink(row_path, row.is_dir, row.symlink.as_ref(), palette);
            let name_width = rect.width.saturating_sub(6) as usize;
            let path_width = rect.width.saturating_sub(4) as usize;
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(icon, Style::default().fg(icon_color)),
                    Span::raw("  "),
                    Span::styled(
                        helpers::clamp_label(&row.name, name_width.max(8)),
                        Style::default()
                            .fg(palette.text)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
                .style(Style::default().bg(bg).fg(palette.text)),
                Rect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: 1,
                },
            );
            if rect.height > 1 {
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            helpers::stable_path_label(
                                std::path::Path::new(&row.relative),
                                path_width.max(10),
                            ),
                            Style::default().fg(palette.muted),
                        ),
                    ]))
                    .style(Style::default().bg(bg).fg(palette.muted)),
                    Rect {
                        x: rect.x,
                        y: rect.y + 1,
                        width: rect.width,
                        height: 1,
                    },
                );
            }

            state.search_hits.push(SearchHit {
                rect,
                index: row.index,
            });
        }
    }

    if let Some(scrollbar) = scrollbar_area {
        render_overlay_scrollbar(
            frame,
            scrollbar,
            app.search_match_count(),
            visible_rows,
            app.search_scroll_top(),
            palette,
        );
    }
}

fn render_query_line(
    query: &str,
    cursor: usize,
    width: u16,
    origin_x: u16,
    palette: Palette,
) -> (Line<'static>, u16) {
    let chars = query.chars().collect::<Vec<_>>();
    let cursor = cursor.min(chars.len());
    let available = width.saturating_sub(3) as usize;

    let mut start = 0usize;
    if cursor > available {
        start = cursor - available;
    }
    let mut visible = chars.iter().skip(start).take(available).collect::<String>();
    if start > 0 && !visible.is_empty() {
        visible.remove(0);
        visible.insert(0, '…');
    }

    let visible_chars = visible.chars().collect::<Vec<_>>();
    let visible_cursor = cursor.saturating_sub(start).min(visible_chars.len());

    let mut spans = vec![
        Span::styled("󰍉", Style::default().fg(palette.accent)),
        Span::raw("  "),
    ];
    spans.push(Span::styled(
        visible,
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD),
    ));

    let cursor_x = origin_x
        .saturating_add(3)
        .saturating_add(visible_cursor as u16)
        .min(origin_x.saturating_add(width.saturating_sub(1)));
    (Line::from(spans), cursor_x)
}

fn format_search_count(count: usize) -> String {
    let digits = count.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped
}

fn build_search_summary(
    loading: bool,
    limited: bool,
    result_count: usize,
    scanned_count: usize,
    max_width: usize,
) -> String {
    let results = format_search_count(result_count);
    let scanned = format_search_count(scanned_count);
    let variants = if loading && scanned_count == 0 {
        vec!["scanning…".to_string()]
    } else if loading {
        vec![
            format!("{results} results  •  {scanned} scanned  •  scanning…"),
            format!("scanning…  •  {scanned} scanned"),
            "scanning…".to_string(),
        ]
    } else if limited {
        vec![
            format!("scan limit reached  •  {scanned} scanned  •  {results} results"),
            format!("scan limit reached  •  {scanned} scanned"),
            "scan limit reached".to_string(),
            "limit reached".to_string(),
        ]
    } else {
        vec![
            format!("{results} results  •  {scanned} scanned"),
            format!("{scanned} scanned"),
        ]
    };

    variants
        .iter()
        .find(|variant| UnicodeWidthStr::width(variant.as_str()) <= max_width)
        .cloned()
        .unwrap_or_else(|| variants.last().cloned().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limited_search_summary_keeps_limit_status_when_width_is_tight() {
        let summary = build_search_summary(false, true, 127_053, 5_000_000, 45);

        assert_eq!(summary, "scan limit reached  •  5,000,000 scanned");
    }

    #[test]
    fn loading_search_summary_prioritizes_scanning_status_when_width_is_tight() {
        let summary = build_search_summary(true, false, 127_053, 1_000_000, 40);

        assert_eq!(summary, "scanning…  •  1,000,000 scanned");
    }

    #[test]
    fn loading_search_summary_keeps_count_order_when_width_allows() {
        let summary = build_search_summary(true, false, 86_472, 518_144, 60);

        assert_eq!(summary, "86,472 results  •  518,144 scanned  •  scanning…");
    }
}
