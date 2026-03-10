use super::theme::Palette;
use super::{helpers, theme};
use crate::app::{
    App, Entry, EntryHit, FrameState, PathHit, ViewMetrics, format_size, format_time_ago,
};
use crate::appearance;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

pub(super) fn render_body(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let columns = if area.width >= 126 {
        let outer = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(80)])
            .split(area);
        let content = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(54), Constraint::Percentage(46)])
            .split(outer[1]);
        vec![outer[0], content[0], content[1]]
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(22), Constraint::Min(42)])
            .split(area)
            .to_vec()
    };

    render_sidebar(frame, columns[0], app, state, palette);

    if columns.len() == 3 {
        render_entries(frame, columns[1], app, state, palette);
        render_preview(frame, columns[2], app, state, palette);
    } else {
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(12), Constraint::Length(11)])
            .split(columns[1]);
        render_entries(frame, right[0], app, state, palette);
        render_preview(frame, right[1], app, state, palette);
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
    let path_text = helpers::stable_path_label(&app.cwd, area.width.saturating_sub(10) as usize);
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " 󰉖 ",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                path_text,
                Style::default()
                    .fg(palette.accent_text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(palette.panel_alt).fg(palette.text))
        .border_style(Style::default().fg(palette.border));
    frame.render_widget(&block, area);
    let inner = block.inner(area);

    if app.view_mode == crate::app::ViewMode::Grid {
        render_grid(frame, inner, app, state, palette);
    } else {
        render_list(frame, inner, app, state, palette);
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
    let background = palette.surface;
    let content_bg = if selected {
        theme::mix_color(palette.selected_bg, icon_color, 22)
    } else {
        palette.surface
    };
    let band_bg = palette.elevated;
    let band_fg = palette.text;
    let band_icon = icon_color;

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
                Style::default().fg(band_fg).add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(band_bg).fg(band_fg)),
        band.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }),
    );

    let content = Rect {
        x: rect.x,
        y: rect.y.saturating_add(1),
        width: rect.width,
        height: rect.height.saturating_sub(1),
    };
    let content_inner = content.inner(Margin {
        horizontal: spec.padding_x,
        vertical: 0,
    });
    let content_text = Rect {
        x: content_inner.x,
        y: content_inner.y,
        width: content_inner.width,
        height: content_inner.height,
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
    if content.height > 0 {
        frame.render_widget(
            Block::default().style(Style::default().bg(content_bg).fg(palette.text)),
            content,
        );
        frame.render_widget(
            Paragraph::new(lines).style(Style::default().bg(content_bg).fg(palette.text)),
            content_text,
        );
    }
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
                Paragraph::new("▌").alignment(Alignment::Left).style(
                    Style::default()
                        .bg(bg)
                        .fg(if selected { palette.accent } else { bg }),
                ),
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
                Paragraph::new("▌").alignment(Alignment::Left).style(
                    Style::default()
                        .bg(bg)
                        .fg(if selected { palette.accent } else { bg }),
                ),
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
                            helpers::clamp_label(&entry.name, row.width.saturating_sub(8) as usize),
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

fn render_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    state.preview_panel = Some(area);

    let title_line = if let Some(entry) = app.selected_entry() {
        Line::from(vec![
            Span::styled(
                format!(" {} ", theme::entry_symbol(entry)),
                Style::default()
                    .fg(theme::entry_color(entry, palette))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                helpers::clamp_label(&entry.name, area.width.saturating_sub(10) as usize),
                Style::default()
                    .fg(palette.accent_text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                " Details ",
                Style::default()
                    .fg(palette.accent_text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ])
    };
    let block = Block::default()
        .title(title_line)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(palette.panel).fg(palette.text))
        .border_style(Style::default().fg(palette.border));
    frame.render_widget(block, area);
    let inner = helpers::inner_with_padding(area);

    let Some(entry) = app.selected_entry() else {
        helpers::render_empty_state(frame, inner, "Nothing selected", palette);
        return;
    };

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(inner.height.min(5)), Constraint::Min(0)])
        .split(inner);

    render_preview_details(frame, sections[0], app, entry, palette);
    if sections[1].height > 0 {
        state.preview_rows_visible = sections[1].height.saturating_sub(1) as usize;
        render_preview_body(frame, sections[1], app, state, entry, palette);
    }
}

fn render_preview_details(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    entry: &Entry,
    palette: Palette,
) {
    let mut lines = vec![
        preview_stat_line(
            "Type",
            appearance::type_label_for_path(&entry.path, entry.kind).to_string(),
            palette,
        ),
        preview_stat_line(
            "Size",
            if entry.is_dir() {
                "folder".to_string()
            } else {
                format_size(entry.size)
            },
            palette,
        ),
        preview_stat_line(
            "Modified",
            entry
                .modified
                .map(format_time_ago)
                .unwrap_or_else(|| "unknown".to_string()),
            palette,
        ),
        preview_stat_line(
            "Access",
            if entry.readonly {
                "readonly".to_string()
            } else {
                "read/write".to_string()
            },
            palette,
        ),
    ];

    if let Some((items, _folders, _files)) = app.preview_directory_counts() {
        lines[1] = preview_stat_line("Items", items.to_string(), palette);
    }

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(palette.panel).fg(palette.text))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_preview_body(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    _entry: &Entry,
    palette: Palette,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    let body = if sections[1].width >= 6 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(sections[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0)])
            .split(sections[1])
    };
    let text_area = body[0];
    let scrollbar_area = body.get(1).copied();
    let visible_rows = text_area.height as usize;
    state.preview_cols_visible = text_area.width as usize;
    let header_detail = app.preview_header_detail(visible_rows);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                app.preview_section_label().to_string(),
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default().fg(palette.muted)),
            Span::styled(
                header_detail.unwrap_or_default(),
                Style::default().fg(palette.muted),
            ),
        ]))
        .style(Style::default().bg(palette.panel).fg(palette.text)),
        sections[0],
    );

    let mut paragraph = Paragraph::new(app.preview_lines())
        .style(Style::default().bg(palette.panel).fg(palette.text))
        .scroll((
            app.preview_scroll_offset().min(u16::MAX as usize) as u16,
            app.preview_horizontal_scroll_offset()
                .min(u16::MAX as usize) as u16,
        ));
    if app.preview_wraps() {
        paragraph = paragraph.wrap(Wrap { trim: false });
    }
    frame.render_widget(paragraph, text_area);

    if let Some(scrollbar_area) = scrollbar_area {
        render_preview_scrollbar(
            frame,
            scrollbar_area,
            app,
            visible_rows,
            text_area.width as usize,
            palette,
        );
    }
}

fn preview_stat_line(label: &str, value: String, palette: Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<9}"), Style::default().fg(palette.muted)),
        Span::styled(value, Style::default().fg(palette.text)),
    ])
}

fn render_preview_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    visible_rows: usize,
    visible_cols: usize,
    palette: Palette,
) {
    let total = app.preview_total_lines(visible_cols);
    if area.height == 0 || total <= visible_rows.max(1) {
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(palette.panel).fg(palette.border)),
            area,
        );
        return;
    }

    let track = vec![
        Line::from(Span::styled("│", Style::default().fg(palette.border),));
        area.height as usize
    ];
    frame.render_widget(
        Paragraph::new(track).style(Style::default().bg(palette.panel)),
        area,
    );

    let thumb_height = ((visible_rows.max(1) * area.height as usize) / total)
        .max(1)
        .min(area.height as usize);
    let max_scroll = total.saturating_sub(visible_rows.max(1));
    let thumb_max_top = area.height as usize - thumb_height;
    let thumb_top = if max_scroll == 0 {
        0
    } else {
        (app.preview_scroll_offset() * thumb_max_top) / max_scroll
    };

    let thumb = Rect {
        x: area.x,
        y: area.y + thumb_top as u16,
        width: area.width,
        height: thumb_height as u16,
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "┃",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            thumb.height as usize
        ])
        .style(Style::default().bg(palette.panel)),
        thumb,
    );
}
