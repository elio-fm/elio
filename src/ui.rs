use crate::app::{
    App, Entry, EntryHit, FrameState, PathHit, SearchHit, ViewMetrics, folder_color, format_size,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use std::{env, path::Path};

#[derive(Clone, Copy)]
struct Palette {
    bg: Color,
    chrome: Color,
    chrome_alt: Color,
    panel: Color,
    panel_alt: Color,
    surface: Color,
    elevated: Color,
    border: Color,
    text: Color,
    muted: Color,
    accent: Color,
    accent_soft: Color,
    accent_text: Color,
    selected_bg: Color,
    selected_border: Color,
    sidebar_active: Color,
    button_bg: Color,
    button_disabled_bg: Color,
    path_bg: Color,
}

pub fn render(frame: &mut Frame<'_>, app: &App, state: &mut FrameState) {
    let palette = palette();

    state.sidebar_hits.clear();
    state.entry_hits.clear();
    state.search_hits.clear();
    state.search_panel = None;
    state.back_button = None;
    state.forward_button = None;
    state.parent_button = None;
    state.refresh_button = None;
    state.hidden_button = None;
    state.view_button = None;

    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(palette.bg).fg(palette.text)),
        area,
    );

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(area);

    render_toolbar(frame, rows[0], app, state, palette);
    render_body(frame, rows[1], app, state, palette);
    render_status(frame, rows[2], app, palette);

    if app.search_is_open() {
        render_search_overlay(frame, area, app, state, palette);
    } else if app.help_open {
        render_help(frame, area, palette);
    }
}

fn render_toolbar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let block = Block::default()
        .style(Style::default().bg(palette.chrome).fg(palette.text))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(palette.border));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let control_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(34),
            Constraint::Min(2),
            Constraint::Length(39),
        ])
        .split(inner);
    let nav_buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(7),
            Constraint::Length(11),
        ])
        .split(control_row[0]);
    let meta = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),
            Constraint::Length(13),
            Constraint::Length(10),
        ])
        .split(control_row[2]);

    state.back_button = Some(nav_buttons[0]);
    state.forward_button = Some(nav_buttons[1]);
    state.parent_button = Some(nav_buttons[2]);
    state.refresh_button = Some(nav_buttons[3]);
    state.hidden_button = Some(meta[1]);
    state.view_button = Some(meta[2]);

    render_button(
        frame,
        nav_buttons[0],
        "Back",
        "󰁍",
        app.can_go_back(),
        palette,
    );
    render_button(
        frame,
        nav_buttons[1],
        "Next",
        "󰁔",
        app.can_go_forward(),
        palette,
    );
    render_button(frame, nav_buttons[2], "Up", "󰁝", true, palette);
    render_button(frame, nav_buttons[3], "Refresh", "󰑐", true, palette);
    frame.render_widget(
        Paragraph::new(Line::from(vec![chip_span(
            &format!("Sort: {}", app.sort_mode.label()),
            palette.button_bg,
            palette.text,
            true,
        )]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(palette.chrome).fg(palette.text)),
        meta[0],
    );
    render_button(
        frame,
        meta[1],
        if app.show_hidden {
            "Hidden On"
        } else {
            "Hidden Off"
        },
        "󰈉",
        true,
        palette,
    );
    render_button(frame, meta[2], app.view_mode.label(), "󰕮", true, palette);
}

fn render_body(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let columns = if area.width >= 126 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(24),
                Constraint::Min(50),
                Constraint::Length(34),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(22), Constraint::Min(42)])
            .split(area)
    };

    render_sidebar(frame, columns[0], app, state, palette);

    if columns.len() == 3 {
        render_entries(frame, columns[1], app, state, palette);
        render_preview(frame, columns[2], app, palette);
    } else {
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(12), Constraint::Length(11)])
            .split(columns[1]);
        render_entries(frame, right[0], app, state, palette);
        render_preview(frame, right[1], app, palette);
    }
}

fn render_sidebar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let block = panel_block(" Places ", palette.panel, palette);
    frame.render_widget(block, area);
    let inner = inner_with_padding(area);

    let header = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("󰙅", Style::default().fg(palette.accent)),
            Span::raw(" "),
            Span::styled(
                "Pinned Places",
                Style::default()
                    .bg(palette.panel)
                    .fg(palette.muted)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(palette.panel).fg(palette.muted)),
        header,
    );

    let mut y = inner.y.saturating_add(2);
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
        let active = path_is_active(&app.cwd, &item.path);
        let bg = if active {
            palette.sidebar_active
        } else {
            palette.panel
        };
        let top_line = Line::from(vec![
            Span::styled(
                if active { "▌" } else { " " },
                Style::default().fg(if active { palette.accent } else { bg }),
            ),
            Span::raw(" "),
            Span::styled(
                item.icon,
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                item.title.clone(),
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
        y = y.saturating_add(row_height);
    }
}

fn render_entries(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let block = panel_block(" Directory ", palette.panel_alt, palette);
    frame.render_widget(block, area);
    let inner = inner_with_padding(area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(4),
        ])
        .split(inner);

    let path_text = stable_path_label(&app.cwd, rows[0].width.saturating_sub(6) as usize);
    frame.render_widget(
        Block::default().style(Style::default().bg(palette.path_bg).fg(palette.text)),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "󰉖",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                path_text,
                Style::default()
                    .fg(palette.accent_text)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(palette.path_bg).fg(palette.text)),
        rows[0].inner(Margin {
            horizontal: 1,
            vertical: 0,
        }),
    );
    frame.render_widget(
        Block::default().style(Style::default().bg(palette.panel_alt).fg(palette.text)),
        rows[1],
    );

    if app.view_mode == crate::app::ViewMode::Grid {
        render_grid(frame, rows[2], app, state, palette);
    } else {
        render_list(frame, rows[2], app, state, palette);
    }
}

fn render_grid(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let spec = grid_zoom_spec(app.zoom_level);
    let gap_x = spec.gap_x;
    let gap_y = spec.gap_y;
    let cols = ((area.width + gap_x) / (spec.tile_width_hint + gap_x)).max(1) as usize;
    let total_gap_x = gap_x.saturating_mul(cols.saturating_sub(1) as u16);
    let tile_width =
        (area.width.saturating_sub(total_gap_x) / cols as u16).max(spec.min_tile_width);
    let rows_visible = ((area.height + gap_y) / (spec.tile_height + gap_y)).max(1) as usize;
    state.metrics = ViewMetrics { cols, rows_visible };

    if app.entries.is_empty() {
        render_empty_state(frame, area, "This folder is empty", palette);
        return;
    }

    let start = app.scroll_row * cols;
    let limit = rows_visible * cols;

    for (visible_index, entry_index) in (start..app.entries.len()).take(limit).enumerate() {
        let row = visible_index / cols;
        let col = visible_index % cols;
        let rect = Rect {
            x: area.x + col as u16 * (tile_width + gap_x),
            y: area.y + row as u16 * (spec.tile_height + gap_y),
            width: tile_width,
            height: spec.tile_height,
        };
        let entry = &app.entries[entry_index];
        render_tile(
            frame,
            rect,
            entry,
            entry_index == app.selected,
            palette,
            spec,
        );
        state.entry_hits.push(EntryHit {
            rect,
            index: entry_index,
        });
    }
}

fn render_tile(
    frame: &mut Frame<'_>,
    rect: Rect,
    entry: &Entry,
    selected: bool,
    palette: Palette,
    spec: GridZoomSpec,
) {
    let icon_color = entry_color(entry, palette);
    let background = if selected {
        mix_color(palette.selected_bg, icon_color, 22)
    } else {
        palette.surface
    };
    let band_bg = if selected {
        mix_color(palette.chrome_alt, icon_color, 90)
    } else {
        palette.elevated
    };
    let band_fg = palette.text;
    let band_icon = if selected {
        band_fg
    } else {
        icon_color
    };

    frame.render_widget(
        Block::default().style(Style::default().bg(background).fg(palette.text)),
        rect,
    );

    let band = Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: 1,
    };
    frame.render_widget(
        Block::default().style(Style::default().bg(band_bg).fg(band_fg)),
        band,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                entry_symbol(entry),
                Style::default().fg(band_icon).add_modifier(
                    Modifier::BOLD
                        | if spec.emphasize_icon {
                            Modifier::ITALIC
                        } else {
                            Modifier::empty()
                        },
                ),
            ),
            Span::raw(" "),
            Span::styled(
                clamp_label(&entry.name, band.width.saturating_sub(5) as usize),
                Style::default()
                    .fg(band_fg)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(band_bg).fg(band_fg)),
        band.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }),
    );

    let inner = rect.inner(Margin {
        horizontal: spec.padding_x,
        vertical: 0,
    });
    let content = Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    let detail = (!entry.is_dir()).then(|| format_size(entry.size));
    let modified = entry
        .modified
        .map(crate::app::format_time_ago)
        .unwrap_or_else(|| "unknown".to_string());
    let mut lines = vec![Line::from(Span::styled(
        entry.detail_label(),
        Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
    ))];
    if spec.show_kind_hint {
        lines.push(Line::from(Span::styled(
            if entry.is_dir() {
                "Open folder"
            } else {
                "Open file"
            },
            Style::default().fg(icon_color),
        )));
    }
    if let Some(detail) = detail {
        lines.push(Line::from(Span::styled(
            detail,
            Style::default().fg(palette.muted),
        )));
    }
    lines.push(Line::from(Span::styled(
        modified,
        Style::default().fg(palette.muted),
    )));
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(background).fg(palette.text)),
        content,
    );
}

fn render_list(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let row_height = list_row_height(app.zoom_level);
    state.metrics = ViewMetrics {
        cols: 1,
        rows_visible: (area.height / row_height.max(1)).max(1) as usize,
    };

    if app.entries.is_empty() {
        render_empty_state(frame, area, "This folder is empty", palette);
        return;
    }

    for (visible_index, entry_index) in (app.scroll_row..app.entries.len())
        .take(state.metrics.rows_visible)
        .enumerate()
    {
        let entry = &app.entries[entry_index];
        let row = Rect {
            x: area.x,
            y: area.y + visible_index as u16 * row_height,
            width: area.width,
            height: row_height,
        };
        let selected = entry_index == app.selected;
        let icon_color = entry_color(entry, palette);
        let bg = if selected {
            palette.selected_bg
        } else {
            palette.panel_alt
        };
        if row_height == 1 {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "▌",
                        Style::default().fg(if selected {
                            palette.accent
                        } else {
                            palette.panel_alt
                        }),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        entry_symbol(entry),
                        Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        clamp_label(&entry.name, 28),
                        if selected {
                            Style::default()
                                .fg(palette.text)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(palette.text)
                        },
                    ),
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        if entry.is_dir() {
                            "folder".to_string()
                        } else {
                            format_size(entry.size)
                        },
                        Style::default().fg(palette.muted),
                    ),
                    Span::styled("  •  ", Style::default().fg(palette.muted)),
                    Span::styled(
                        entry
                            .modified
                            .map(crate::app::format_time_ago)
                            .unwrap_or_else(|| "unknown".to_string()),
                        Style::default().fg(palette.muted),
                    ),
                ]))
                .style(Style::default().bg(bg).fg(palette.text)),
                row,
            );
        } else {
            let secondary = if row_height >= 3 {
                format!(
                    "{}  •  {}",
                    if entry.is_dir() {
                        "folder".to_string()
                    } else {
                        format_size(entry.size)
                    },
                    entry
                        .modified
                        .map(crate::app::format_time_ago)
                        .unwrap_or_else(|| "unknown".to_string())
                )
            } else if entry.is_dir() {
                "folder".to_string()
            } else {
                format_size(entry.size)
            };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(
                            "▌",
                            Style::default().fg(if selected { palette.accent } else { bg }),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            entry_symbol(entry),
                            Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            clamp_label(&entry.name, row.width.saturating_sub(8) as usize),
                            Style::default()
                                .fg(palette.text)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(secondary, Style::default().fg(palette.muted)),
                    ]),
                ])
                .style(Style::default().bg(bg).fg(palette.text)),
                row,
            );
        }
        state.entry_hits.push(EntryHit {
            rect: row,
            index: entry_index,
        });
    }
}

fn render_preview(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
    let block = panel_block(" Details ", palette.panel, palette);
    frame.render_widget(block, area);
    let inner = inner_with_padding(area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(inner);

    let title = app
        .selected_entry()
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| "Nothing selected".to_string());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            chip_span("selected", palette.accent_soft, palette.accent_text, false),
            Span::styled("  ", Style::default()),
            Span::styled(
                clamp_label(&title, rows[0].width as usize),
                Style::default()
                    .fg(palette.text)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(palette.panel).fg(palette.text)),
        rows[0],
    );

    let preview = Paragraph::new(app.preview_lines(rows[1].height.saturating_sub(1) as usize))
        .style(Style::default().bg(palette.panel).fg(palette.text))
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, rows[1]);
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(24), Constraint::Length(34)])
        .split(area);

    let right_text = if app.status_message().is_empty() {
        "f folders  Ctrl+F files  ? help".to_string()
    } else {
        truncate_middle(app.status_message(), sections[1].width as usize)
    };
    frame.render_widget(
        Paragraph::new(app.selection_summary()).style(
            Style::default()
                .bg(palette.chrome)
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ),
        sections[0],
    );
    frame.render_widget(
        Paragraph::new(right_text)
            .alignment(Alignment::Right)
            .style(Style::default().bg(palette.chrome).fg(palette.muted)),
        sections[1],
    );
}

fn render_search_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let popup_width = area.width.saturating_sub(8).clamp(48, 88);
    let popup_height = area.height.saturating_sub(6).clamp(12, 22);
    let popup = centered_rect(area, popup_width, popup_height);
    state.search_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        panel_block(" Fuzzy Find ", palette.chrome_alt, palette),
        popup,
    );

    let inner = inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(inner);

    let scope_label = app
        .search_scope()
        .map(|scope| scope.label())
        .unwrap_or("Search");
    let summary = if app.search_is_loading() {
        "indexing…".to_string()
    } else {
        format!(
            "{} results  •  {} scanned",
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
            chip_span(&summary, palette.accent_soft, palette.accent_text, true),
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
            Some(crate::app::SearchScope::Folders) => "type to filter folders".to_string(),
            Some(crate::app::SearchScope::Files) => "type to filter files".to_string(),
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
        render_empty_state(
            frame,
            results_area,
            "Indexing current folder tree…",
            palette,
        );
    } else if let Some(error) = app.search_error() {
        render_empty_state(
            frame,
            results_area,
            &truncate_middle(error, results_area.width.saturating_sub(4) as usize),
            palette,
        );
    } else if rows_data.is_empty() {
        render_empty_state(
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
            let kind = if row.is_dir { "Folder" } else { "File" };
            let name_width = rect.width.saturating_sub(18) as usize;
            let path_width = rect.width.saturating_sub(4) as usize;
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(icon, Style::default().fg(palette.accent)),
                    Span::raw("  "),
                    Span::styled(
                        clamp_label(&row.name, name_width.max(8)),
                        Style::default()
                            .fg(palette.text)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    chip_span(kind, palette.button_bg, palette.muted, false),
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
                            truncate_path_tail(&row.relative, path_width.max(10)),
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

fn render_help(frame: &mut Frame<'_>, area: Rect, palette: Palette) {
    let popup = centered_rect(area, 82, 19);
    frame.render_widget(
        Block::default().style(Style::default().bg(palette.bg).fg(palette.text)),
        area,
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(panel_block(" Controls ", palette.chrome_alt, palette), popup);
    let inner = inner_with_padding(popup);
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
                chip_span("navigate", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                chip_span("search", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                chip_span("mouse", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                chip_span("view", palette.accent_soft, palette.accent_text, true),
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
        Line::from(vec![
            Span::styled("Navigation", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
        ]),
        help_row("Arrows / jkl", "move selection", palette),
        help_row("Enter", "open folder or file", palette),
        help_row("Backspace", "parent directory", palette),
        help_row("Alt+Left", "previous folder", palette),
        help_row("Alt+Right", "next folder", palette),
        help_row("h", "jump to home", palette),
        Line::from(""),
        Line::from(vec![
            Span::styled("Search", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
        ]),
        help_row("f", "search folders", palette),
        help_row("Ctrl+F", "search files", palette),
    ];
    frame.render_widget(
        Paragraph::new(left)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[0],
    );

    let right = vec![
        Line::from(vec![
            Span::styled("Mouse + View", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
        ]),
        help_row("Click", "select item", palette),
        help_row("Double click", "open folder or file", palette),
        help_row("Wheel", "scroll selection", palette),
        help_row("v", "toggle grid/list view", palette),
        help_row(".", "show or hide dotfiles", palette),
        help_row("s", "cycle sort mode", palette),
        help_row("r / Ctrl+R", "refresh current folder", palette),
        help_row("o", "open selected item externally", palette),
    ];
    frame.render_widget(
        Paragraph::new(right)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("? / Esc", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled("close help or quit", Style::default().fg(palette.muted)),
        ]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(palette.chrome_alt).fg(palette.muted)),
        rows[2],
    );
}

fn help_row(key: &str, action: &str, palette: Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{key:<12}"),
            Style::default()
                .fg(palette.accent_text)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(action.to_string(), Style::default().fg(palette.muted)),
    ])
}

fn render_empty_state(frame: &mut Frame<'_>, area: Rect, label: &str, palette: Palette) {
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Center)
            .style(Style::default().bg(palette.panel_alt).fg(palette.muted)),
        area,
    );
}

fn render_button(
    frame: &mut Frame<'_>,
    rect: Rect,
    label: &str,
    icon: &str,
    enabled: bool,
    palette: Palette,
) {
    let bg = if enabled {
        palette.button_bg
    } else {
        palette.button_disabled_bg
    };
    let fg = if enabled { palette.text } else { palette.muted };
    frame.render_widget(Block::default().style(Style::default().bg(bg).fg(fg)), rect);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                icon,
                Style::default().fg(if enabled {
                    palette.accent
                } else {
                    palette.muted
                }),
            ),
            Span::raw(" "),
            Span::styled(
                label.to_string(),
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center)
        .style(Style::default().bg(bg).fg(fg)),
        rect,
    );
}

fn panel_block<'a>(title: &'a str, bg: Color, palette: Palette) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(palette.accent_text)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(bg).fg(palette.text))
        .border_style(Style::default().fg(palette.border))
}

fn palette() -> Palette {
    Palette {
        bg: Color::Rgb(10, 14, 20),
        chrome: Color::Rgb(16, 21, 30),
        chrome_alt: Color::Rgb(24, 32, 43),
        panel: Color::Rgb(18, 25, 35),
        panel_alt: Color::Rgb(14, 20, 28),
        surface: Color::Rgb(22, 30, 41),
        elevated: Color::Rgb(27, 37, 50),
        border: Color::Rgb(49, 67, 87),
        text: Color::Rgb(238, 243, 248),
        muted: Color::Rgb(158, 172, 189),
        accent: Color::Rgb(102, 186, 255),
        accent_soft: Color::Rgb(34, 57, 79),
        accent_text: Color::Rgb(207, 234, 255),
        selected_bg: Color::Rgb(36, 56, 78),
        selected_border: Color::Rgb(112, 196, 255),
        sidebar_active: Color::Rgb(31, 47, 65),
        button_bg: Color::Rgb(29, 39, 52),
        button_disabled_bg: Color::Rgb(20, 27, 37),
        path_bg: Color::Rgb(28, 37, 49),
    }
}

fn mix_color(base: Color, tint: Color, tint_weight: u8) -> Color {
    match (base, tint) {
        (Color::Rgb(br, bg, bb), Color::Rgb(tr, tg, tb)) => {
            let weight = u16::from(tint_weight);
            let base_weight = 255 - weight;
            Color::Rgb(
                ((u16::from(br) * base_weight + u16::from(tr) * weight) / 255) as u8,
                ((u16::from(bg) * base_weight + u16::from(tg) * weight) / 255) as u8,
                ((u16::from(bb) * base_weight + u16::from(tb) * weight) / 255) as u8,
            )
        }
        _ => base,
    }
}

fn entry_color(entry: &Entry, palette: Palette) -> Color {
    if entry.is_dir() {
        palette.accent
    } else {
        folder_color(entry)
    }
}

fn entry_symbol(entry: &Entry) -> &'static str {
    if entry.is_dir() { "󰉋" } else { "󰈔" }
}

fn chip_span<'a>(label: &'a str, bg: Color, fg: Color, bold: bool) -> Span<'a> {
    let style = if bold {
        Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(bg).fg(fg)
    };
    Span::styled(format!(" {label} "), style)
}

#[derive(Clone, Copy)]
struct GridZoomSpec {
    tile_width_hint: u16,
    min_tile_width: u16,
    tile_height: u16,
    gap_x: u16,
    gap_y: u16,
    padding_x: u16,
    emphasize_icon: bool,
    show_kind_hint: bool,
}

fn grid_zoom_spec(zoom: u8) -> GridZoomSpec {
    match zoom {
        0 => GridZoomSpec {
            tile_width_hint: 18,
            min_tile_width: 17,
            tile_height: 4,
            gap_x: 1,
            gap_y: 0,
            padding_x: 1,
            emphasize_icon: false,
            show_kind_hint: false,
        },
        1 => GridZoomSpec {
            tile_width_hint: 22,
            min_tile_width: 20,
            tile_height: 5,
            gap_x: 2,
            gap_y: 1,
            padding_x: 1,
            emphasize_icon: false,
            show_kind_hint: false,
        },
        2 => GridZoomSpec {
            tile_width_hint: 26,
            min_tile_width: 22,
            tile_height: 6,
            gap_x: 2,
            gap_y: 1,
            padding_x: 2,
            emphasize_icon: false,
            show_kind_hint: false,
        },
        3 => GridZoomSpec {
            tile_width_hint: 30,
            min_tile_width: 26,
            tile_height: 7,
            gap_x: 2,
            gap_y: 1,
            padding_x: 2,
            emphasize_icon: true,
            show_kind_hint: true,
        },
        _ => GridZoomSpec {
            tile_width_hint: 36,
            min_tile_width: 30,
            tile_height: 8,
            gap_x: 3,
            gap_y: 1,
            padding_x: 3,
            emphasize_icon: true,
            show_kind_hint: true,
        },
    }
}

fn list_row_height(zoom: u8) -> u16 {
    match zoom {
        0 | 1 => 1,
        2 | 3 => 2,
        _ => 3,
    }
}

fn stable_path_label(path: &Path, max_chars: usize) -> String {
    let display = if let Some(home) = env::var_os("HOME") {
        let home = std::path::PathBuf::from(home);
        if let Ok(stripped) = path.strip_prefix(&home) {
            if stripped.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", stripped.display())
            }
        } else {
            path.display().to_string()
        }
    } else {
        path.display().to_string()
    };

    truncate_path_tail(&display, max_chars.max(8))
}

fn path_is_active(current: &Path, candidate: &Path) -> bool {
    current == candidate
}

fn truncate_path_tail(path: &str, max_chars: usize) -> String {
    if path.chars().count() <= max_chars {
        return path.to_string();
    }

    let prefix = if path.starts_with("~/") {
        "~/"
    } else if path.starts_with('/') {
        "/"
    } else {
        ""
    };
    let mut parts = path
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let mut kept = Vec::new();
    let mut used = prefix.chars().count() + 2;

    while let Some(part) = parts.pop() {
        let extra = part.chars().count() + usize::from(!kept.is_empty());
        if used + extra > max_chars {
            break;
        }
        kept.push(part);
        used += extra;
    }

    kept.reverse();
    if kept.is_empty() {
        return truncate_middle(path, max_chars);
    }

    let tail = kept.join("/");
    let compact = if prefix.is_empty() {
        format!("…/{}", tail)
    } else {
        format!("{}…/{}", prefix, tail)
    };

    if compact.chars().count() <= max_chars {
        compact
    } else {
        truncate_middle(&compact, max_chars)
    }
}

fn truncate_middle(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }

    let left = (max_chars - 1) / 2;
    let right = max_chars - 1 - left;
    let start = text.chars().take(left).collect::<String>();
    let end = text
        .chars()
        .rev()
        .take(right)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{start}…{end}")
}

fn clamp_label(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars {
        return label.to_string();
    }
    let take = max_chars.saturating_sub(1);
    let mut shortened = label.chars().take(take).collect::<String>();
    shortened.push('…');
    shortened
}

fn inner_with_padding(rect: Rect) -> Rect {
    rect.inner(Margin {
        horizontal: 1,
        vertical: 1,
    })
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
