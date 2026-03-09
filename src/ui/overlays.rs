use super::helpers;
use super::theme::{self, Palette};
use crate::app::{App, FrameState, SearchHit, SearchScope};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Wrap},
};

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
            Constraint::Length(1),
        ])
        .split(inner);

    let scope_label = app.search_scope().map(|scope| scope.label()).unwrap_or("Search");
    let summary = if app.search_is_loading() {
        "indexing…".to_string()
    } else {
        format!(
            "{} results  •  {} indexed",
            app.search_match_count(),
            app.search_candidate_count()
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                scope_label,
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  current folder tree", Style::default().fg(palette.muted)),
            Span::raw("  "),
            helpers::chip_span(&summary, palette.accent_soft, palette.accent_text, true),
        ]))
        .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
        rows[0],
    );

    frame.render_widget(
        Block::default().style(Style::default().bg(palette.path_bg).fg(palette.text)),
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
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("󰍉", Style::default().fg(palette.accent)),
            Span::raw("  "),
            Span::styled(query, query_style),
        ]))
        .style(Style::default().bg(palette.path_bg).fg(palette.text)),
        rows[1].inner(Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );

    let results_area = rows[2];
    let row_height = 2u16;
    let visible_rows = (results_area.height / row_height).max(1) as usize;
    state.search_rows_visible = visible_rows;

    let rows_data = app.search_rows(visible_rows);
    if app.search_is_loading() {
        helpers::render_empty_state(frame, results_area, "Indexing current folder tree…", palette);
    } else if let Some(error) = app.search_error() {
        helpers::render_empty_state(
            frame,
            results_area,
            &helpers::truncate_middle(error, results_area.width.saturating_sub(4) as usize),
            palette,
        );
    } else if rows_data.is_empty() {
        helpers::render_empty_state(
            frame,
            results_area,
            app.search_scope()
                .map(|scope| scope.empty_label())
                .unwrap_or("No matches in this folder tree"),
            palette,
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
                palette.surface
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

            let icon = if row.is_dir { "󰉋" } else { "󰈔" };
            let icon_color = theme::path_color(std::path::Path::new(&row.relative), row.is_dir, palette);
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
                            helpers::stable_path_label(std::path::Path::new(&row.relative), path_width.max(10)),
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

    frame.render_widget(
        Paragraph::new("Enter open  Esc close  ↑↓ move")
            .alignment(Alignment::Right)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.muted)),
        rows[3],
    );
}

pub(super) fn render_help(frame: &mut Frame<'_>, area: Rect, palette: Palette) {
    let popup = helpers::centered_rect(area, 82, 19);
    frame.render_widget(
        Block::default().style(Style::default().bg(palette.bg).fg(palette.text)),
        area,
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(" Controls ", palette.chrome_alt, palette),
        popup,
    );
    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "󰘳",
                    Style::default()
                        .fg(palette.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    "Keyboard and mouse controls",
                    Style::default()
                        .fg(palette.text)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                helpers::chip_span("navigate", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                helpers::chip_span("search", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                helpers::chip_span("mouse", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                helpers::chip_span("view", palette.accent_soft, palette.accent_text, true),
            ]),
        ])
        .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    let left = vec![
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        helpers::help_row("Arrows / jkl", "move selection", palette),
        helpers::help_row("Enter", "open folder or file", palette),
        helpers::help_row("Backspace", "parent directory", palette),
        helpers::help_row("Alt+Left", "previous folder", palette),
        helpers::help_row("Alt+Right", "next folder", palette),
        helpers::help_row("h", "jump to home", palette),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Search",
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        helpers::help_row("f", "search folders", palette),
        helpers::help_row("Ctrl+F", "search files", palette),
    ];
    frame.render_widget(
        Paragraph::new(left)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[0],
    );

    let right = vec![
        Line::from(vec![Span::styled(
            "Mouse + View",
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        helpers::help_row("Click", "select item", palette),
        helpers::help_row("Double click", "open folder or file", palette),
        helpers::help_row("Wheel", "scroll selection", palette),
        helpers::help_row("v", "toggle grid/list view", palette),
        helpers::help_row(".", "show or hide dotfiles", palette),
        helpers::help_row("s", "cycle sort mode", palette),
        helpers::help_row("r / Ctrl+R", "refresh current folder", palette),
        helpers::help_row("o", "open selected item externally", palette),
    ];
    frame.render_widget(
        Paragraph::new(right)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "? / Esc",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("close help or quit", Style::default().fg(palette.muted)),
        ]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(palette.chrome_alt).fg(palette.muted)),
        rows[2],
    );
}
