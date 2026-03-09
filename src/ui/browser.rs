use super::{helpers, theme};
use super::theme::Palette;
use crate::app::{
    App, Entry, EntryHit, FrameState, PathHit, ViewMetrics, format_size, format_time_ago,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

pub(super) fn render_body(
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
    let block = helpers::panel_block(" Places ", palette.panel, palette);
    frame.render_widget(block, area);
    let inner = helpers::inner_with_padding(area);

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
        let active = helpers::path_is_active(&app.cwd, &item.path);
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
    let block = helpers::panel_block(" Directory ", palette.panel_alt, palette);
    frame.render_widget(block, area);
    let inner = helpers::inner_with_padding(area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(4),
        ])
        .split(inner);

    let path_text = helpers::stable_path_label(&app.cwd, rows[0].width.saturating_sub(6) as usize);
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
    let spec = helpers::grid_zoom_spec(app.zoom_level);
    let gap_x = spec.gap_x;
    let gap_y = spec.gap_y;
    let cols = ((area.width + gap_x) / (spec.tile_width_hint + gap_x)).max(1) as usize;
    let total_gap_x = gap_x.saturating_mul(cols.saturating_sub(1) as u16);
    let tile_width =
        (area.width.saturating_sub(total_gap_x) / cols as u16).max(spec.min_tile_width);
    let rows_visible = ((area.height + gap_y) / (spec.tile_height + gap_y)).max(1) as usize;
    state.metrics = ViewMetrics { cols, rows_visible };

    if app.entries.is_empty() {
        helpers::render_empty_state(frame, area, "This folder is empty", palette);
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
    spec: helpers::GridZoomSpec,
) {
    let icon_color = theme::entry_color(entry, palette);
    let background = if selected {
        theme::mix_color(palette.selected_bg, icon_color, 22)
    } else {
        palette.surface
    };
    let band_bg = if selected {
        theme::mix_color(palette.chrome_alt, icon_color, 90)
    } else {
        palette.elevated
    };
    let band_fg = palette.text;
    let band_icon = if selected { band_fg } else { icon_color };

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
                theme::entry_symbol(entry),
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
                helpers::clamp_label(&entry.name, band.width.saturating_sub(5) as usize),
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
        .map(format_time_ago)
        .unwrap_or_else(|| "unknown".to_string());
    let mut lines = Vec::new();
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
    let row_height = helpers::list_row_height(app.zoom_level);
    state.metrics = ViewMetrics {
        cols: 1,
        rows_visible: (area.height / row_height.max(1)).max(1) as usize,
    };

    if app.entries.is_empty() {
        helpers::render_empty_state(frame, area, "This folder is empty", palette);
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
        let icon_color = theme::entry_color(entry, palette);
        let bg = if selected {
            palette.selected_bg
        } else {
            palette.panel_alt
        };
        if row_height == 1 {
            frame.render_widget(
                Block::default().style(Style::default().bg(bg).fg(palette.text)),
                row,
            );
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(3),
                    Constraint::Min(8),
                    Constraint::Length(12),
                    Constraint::Length(10),
                ])
                .split(row);

            frame.render_widget(
                Paragraph::new("▌")
                    .alignment(Alignment::Left)
                    .style(Style::default().bg(bg).fg(if selected { palette.accent } else { bg })),
                columns[0],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        theme::entry_symbol(entry),
                        Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                    ),
                ]))
                .style(Style::default().bg(bg).fg(palette.text)),
                columns[1],
            );
            frame.render_widget(
                Paragraph::new(helpers::clamp_label(
                    &entry.name,
                    columns[2].width.saturating_sub(1) as usize,
                ))
                .style(if selected {
                    Style::default()
                        .bg(bg)
                        .fg(palette.text)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().bg(bg).fg(palette.text)
                }),
                columns[2],
            );
            frame.render_widget(
                Paragraph::new(if entry.is_dir() {
                    String::new()
                } else {
                    format_size(entry.size)
                })
                .alignment(Alignment::Right)
                .style(Style::default().bg(bg).fg(palette.muted)),
                columns[3],
            );
            frame.render_widget(
                Paragraph::new(
                    entry
                        .modified
                        .map(format_time_ago)
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .alignment(Alignment::Right)
                .style(Style::default().bg(bg).fg(palette.muted)),
                columns[4],
            );
        } else {
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(row);
            frame.render_widget(
                Paragraph::new("▌")
                    .alignment(Alignment::Left)
                    .style(Style::default().bg(bg).fg(if selected { palette.accent } else { bg })),
                columns[0],
            );
            let secondary = if row_height >= 3 {
                if entry.is_dir() {
                    entry
                        .modified
                        .map(format_time_ago)
                        .unwrap_or_else(|| "unknown".to_string())
                } else {
                    format!(
                        "{}  •  {}",
                        format_size(entry.size),
                        entry
                            .modified
                            .map(format_time_ago)
                            .unwrap_or_else(|| "unknown".to_string())
                    )
                }
            } else if entry.is_dir() {
                String::new()
            } else {
                format_size(entry.size)
            };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(
                            theme::entry_symbol(entry),
                            Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            helpers::clamp_label(
                                &entry.name,
                                row.width.saturating_sub(8) as usize,
                            ),
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
                columns[1],
            );
        }
        state.entry_hits.push(EntryHit {
            rect: row,
            index: entry_index,
        });
    }
}

fn render_preview(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
    let block = helpers::panel_block(" Details ", palette.panel, palette);
    frame.render_widget(block, area);
    let inner = helpers::inner_with_padding(area);
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
            helpers::chip_span("selected", palette.accent_soft, palette.accent_text, false),
            Span::styled("  ", Style::default()),
            Span::styled(
                helpers::clamp_label(&title, rows[0].width as usize),
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
