use super::theme::Palette;
use super::{helpers, theme};
use crate::app::{
    App, Entry, EntryHit, FrameState, PathHit, ViewMetrics, format_size, format_time_ago,
};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
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
    state.entries_panel = Some(area);
    let path_text = helpers::stable_path_label(&app.cwd, area.width.saturating_sub(10) as usize);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(palette.panel_alt).fg(palette.text))
        .border_style(Style::default().fg(palette.border));
    frame.render_widget(&block, area);
    helpers::render_panel_title(
        frame,
        area,
        Line::from(vec![
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
        ]),
    );
    let inner = block.inner(area);
    helpers::fill_area(frame, inner, palette.panel_alt, palette.text);

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
    helpers::fill_area(frame, area, palette.panel_alt, palette.text);
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
            app,
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
    app: &App,
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
    let detail = browser_entry_detail(app, entry);
    let modified = browser_entry_modified(entry);
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
    helpers::fill_area(frame, area, palette.panel_alt, palette.text);
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
        let multi_selected = app.is_selected(&entry.path);
        let icon_color = theme::entry_color(entry, palette);
        let bg = if selected {
            palette.selected_bg
        } else {
            palette.panel_alt
        };
        if row_height == 1 {
            frame.render_widget(
                Paragraph::new(render_compact_list_row(
                    app, entry, selected, row.width, palette,
                ))
                .style(Style::default().bg(bg).fg(palette.text)),
                row,
            );
        } else {
            let bar_color = if selected {
                palette.selected_border
            } else if multi_selected {
                palette.selection_bar
            } else {
                bg
            };
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(row);
            frame.render_widget(
                Paragraph::new(if selected || multi_selected { "▌" } else { " " })
                    .alignment(Alignment::Left)
                    .style(Style::default().bg(bg).fg(bar_color)),
                columns[0],
            );
            let secondary = if entry.is_dir() {
                browser_directory_secondary(app, entry)
            } else if row_height >= 3 {
                format!(
                    "{}  •  {}",
                    browser_entry_detail(app, entry).unwrap_or_default(),
                    browser_entry_modified(entry)
                )
            } else {
                browser_entry_detail(app, entry).unwrap_or_default()
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
                " Preview ",
                Style::default()
                    .fg(palette.accent_text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ])
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(palette.panel).fg(palette.text))
        .border_style(Style::default().fg(palette.border));
    frame.render_widget(block, area);
    helpers::render_panel_title(frame, area, title_line);
    let inner = helpers::inner_with_padding(area);
    helpers::fill_area(frame, inner, palette.panel, palette.text);

    let Some(entry) = app.selected_entry() else {
        helpers::render_empty_state(frame, inner, "Nothing selected", palette);
        return;
    };

    if inner.height > 0 {
        render_preview_body(frame, inner, app, state, entry, palette);
    }
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
    state.preview_rows_visible = sections[1].height as usize;
    helpers::fill_area(frame, sections[0], palette.panel, palette.text);
    if sections[1].height > 0 {
        helpers::fill_area(frame, sections[1], palette.panel, palette.text);
    }
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
    let body_area = body[0];
    let scrollbar_area = body.get(1).copied();
    let (media_area, text_area) = if let Some(media_rows) = app.preview_visual_rows(body_area)
    {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(media_rows), Constraint::Min(0)])
            .split(body_area);
        (Some(split[0]), split[1])
    } else {
        (None, body_area)
    };
    state.preview_media_area = media_area;
    state.preview_content_area = Some(text_area);
    if let Some(media_area) = media_area {
        helpers::fill_area(frame, media_area, palette.panel, palette.text);
    }
    helpers::fill_area(frame, text_area, palette.panel, palette.text);
    if let Some(scrollbar_area) = scrollbar_area {
        helpers::fill_area(frame, scrollbar_area, palette.panel, palette.border);
    }
    let visible_rows = text_area.height as usize;
    state.preview_cols_visible = text_area.width as usize;
    let section_label = app.preview_section_label();
    let header_detail_width = sections[0]
        .width
        .saturating_sub(section_label.len() as u16 + 2) as usize;
    let header_detail = app
        .preview_header_detail_for_width(visible_rows, header_detail_width)
        .as_deref()
        .map(|detail| {
            if header_detail_width == 0 {
                String::new()
            } else {
                helpers::clamp_label(detail, header_detail_width)
            }
        })
        .unwrap_or_default();

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                section_label.to_string(),
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default().fg(palette.muted)),
            Span::styled(header_detail, Style::default().fg(palette.muted)),
        ]))
        .style(Style::default().bg(palette.panel).fg(palette.text)),
        sections[0],
    );

    if app.browser_wheel_burst_active() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Scrolling...",
                Style::default().fg(palette.muted),
            )))
            .style(Style::default().bg(palette.panel).fg(palette.text))
            .alignment(Alignment::Center),
            text_area,
        );
        return;
    }

    if app.preview_prefers_image_surface() {
        if let Some(message) = app.preview_overlay_placeholder_message() {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    message,
                    Style::default().fg(palette.muted),
                )))
                .style(Style::default().bg(palette.panel).fg(palette.text))
                .alignment(Alignment::Center),
                text_area,
            );
        }
        return;
    }

    if app.preview_uses_image_overlay() {
        return;
    }

    if let Some(media_area) = media_area
        && let Some(message) = app.preview_visual_placeholder_message()
    {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                message,
                Style::default().fg(palette.muted),
            )))
            .style(Style::default().bg(palette.panel).fg(palette.text))
            .alignment(Alignment::Center),
            media_area,
        );
    }

    if app.preview_wraps() {
        let wrapped_lines = app.preview_wrapped_lines(text_area.width as usize);
        frame.render_widget(
            PreviewLinesWidget::new(
                wrapped_lines.as_ref(),
                app.preview_scroll_offset(),
                Style::default().bg(palette.panel).fg(palette.text),
            ),
            text_area,
        );
    } else {
        let paragraph = Paragraph::new(app.preview_lines())
            .style(Style::default().bg(palette.panel).fg(palette.text))
            .scroll((
                app.preview_scroll_offset().min(u16::MAX as usize) as u16,
                app.preview_horizontal_scroll_offset()
                    .min(u16::MAX as usize) as u16,
            ));
        frame.render_widget(paragraph, text_area);
    }

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

struct PreviewLinesWidget<'a> {
    lines: &'a [Line<'static>],
    scroll: usize,
    style: Style,
}

impl<'a> PreviewLinesWidget<'a> {
    fn new(lines: &'a [Line<'static>], scroll: usize, style: Style) -> Self {
        Self {
            lines,
            scroll,
            style,
        }
    }
}

impl Widget for PreviewLinesWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }

        buf.set_style(area, self.style);
        for (line, row) in self.lines.iter().skip(self.scroll).zip(area.rows()) {
            let line_width = line.width();
            let offset = match line.alignment.unwrap_or(Alignment::Left) {
                Alignment::Center => row.width.saturating_sub(line_width as u16) / 2,
                Alignment::Right => row.width.saturating_sub(line_width as u16),
                Alignment::Left => 0,
            };
            if offset >= row.width {
                continue;
            }

            let x = row.x.saturating_add(offset);
            let max_width = row.width.saturating_sub(offset);
            buf.set_line(x, row.y, line, max_width);
        }
    }
}

fn browser_entry_detail(app: &App, entry: &Entry) -> Option<String> {
    if entry.is_dir() {
        app.directory_item_count_label(entry)
    } else {
        Some(format_size(entry.size))
    }
}

fn browser_entry_modified(entry: &Entry) -> String {
    entry
        .modified
        .map(format_time_ago)
        .unwrap_or_else(|| "unknown".to_string())
}

fn browser_directory_secondary(app: &App, entry: &Entry) -> String {
    match app.directory_item_count_label(entry) {
        Some(count) => format!("{count}  •  {}", browser_entry_modified(entry)),
        None => browser_entry_modified(entry),
    }
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

fn render_compact_list_row(
    app: &App,
    entry: &Entry,
    selected: bool,
    row_width: u16,
    palette: Palette,
) -> Line<'static> {
    let detail_width = 12usize;
    let modified_width = 10usize;
    let multi_selected = app.is_selected(&entry.path);
    let marker_color = if selected {
        palette.selected_border
    } else if multi_selected {
        palette.selection_bar
    } else {
        palette.panel_alt
    };
    let icon = theme::entry_symbol(entry);
    let icon_style = Style::default()
        .fg(theme::entry_color(entry, palette))
        .add_modifier(Modifier::BOLD);
    let name_style = if selected {
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.text)
    };
    let muted_style = Style::default().fg(palette.muted);
    let name_width = row_width
        .saturating_sub(1 + 3 + detail_width as u16 + modified_width as u16)
        .max(1) as usize;
    let name = helpers::clamp_label(&entry.name, name_width);
    let detail = pad_left(
        browser_entry_detail(app, entry).unwrap_or_default(),
        detail_width,
    );
    let modified = pad_left(browser_entry_modified(entry), modified_width);

    Line::from(vec![
        Span::styled(
            if selected || multi_selected { "▌" } else { " " },
            Style::default().fg(marker_color),
        ),
        Span::raw(" "),
        Span::styled(icon.to_string(), icon_style),
        Span::raw(" "),
        Span::styled(pad_right(name, name_width), name_style),
        Span::styled(detail, muted_style),
        Span::styled(modified, muted_style),
    ])
}

fn pad_left(mut text: String, width: usize) -> String {
    let visible = helpers::display_width(&text);
    if visible >= width {
        return helpers::clamp_label(&text, width);
    }
    text = format!("{}{}", " ".repeat(width - visible), text);
    text
}

fn pad_right(text: String, width: usize) -> String {
    let visible = helpers::display_width(&text);
    if visible >= width {
        return helpers::clamp_label(&text, width);
    }
    format!("{text}{}", " ".repeat(width - visible))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui;
    use crossterm::event::{Event, KeyCode, KeyEvent};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-browser-{label}-{unique}"))
    }

    fn draw_ui(terminal: &mut Terminal<TestBackend>, app: &mut App) -> FrameState {
        let mut frame_state = FrameState::default();
        terminal
            .draw(|frame| ui::render(frame, app, &mut frame_state))
            .expect("ui should render");
        app.set_frame_state(frame_state.clone());
        frame_state
    }

    fn wait_for_directory_counts(app: &mut App) {
        for _ in 0..100 {
            if app.process_background_jobs() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("timed out waiting for directory counts");
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }

    fn buffer_text(buffer: &Buffer) -> String {
        (0..buffer.area.height)
            .map(|y| row_text(buffer, y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn preview_title_row_is_cleared_when_switching_to_shorter_names() {
        let root = temp_path("preview-title");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(
            root.join("a-this-is-a-very-long-preview-marker-name.txt"),
            "first\n",
        )
        .expect("failed to write long file");
        fs::write(root.join("b.txt"), "second\n").expect("failed to write short file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        let initial_state = draw_ui(&mut terminal, &mut app);
        let preview_panel = initial_state
            .preview_panel
            .expect("preview panel should be rendered");
        let initial_title = row_text(terminal.backend().buffer(), preview_panel.y);
        assert!(
            initial_title.contains("preview-marker-name"),
            "expected initial preview title row to show the long file name, got: {initial_title:?}"
        );

        app.handle_event(Event::Key(KeyEvent::from(KeyCode::Down)))
            .expect("selection change should succeed");
        let second_state = draw_ui(&mut terminal, &mut app);
        let second_title = row_text(
            terminal.backend().buffer(),
            second_state.preview_panel.unwrap().y,
        );

        assert!(
            second_title.contains("b.txt"),
            "expected second preview title row to show the shorter file name, got: {second_title:?}"
        );
        assert!(
            !second_title.contains("preview-marker-name"),
            "stale preview title text remained after rerender: {second_title:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn filenames_with_control_characters_are_rendered_safely() {
        let root = temp_path("control-char-name");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("bad\rname.c"), "int main(void) { return 0; }\n")
            .expect("failed to write control-char file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        draw_ui(&mut terminal, &mut app);
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("bad^Mname.c"),
            "expected control characters to be sanitized in the UI, got: {rendered:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn preview_panel_does_not_repeat_generic_metadata() {
        let root = temp_path("preview-details");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("report.txt"), "hello\n").expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        draw_ui(&mut terminal, &mut app);
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(
            !rendered.contains("Type     "),
            "preview panel should not repeat generic type metadata, got: {rendered:?}"
        );
        assert!(
            !rendered.contains("Size     "),
            "preview panel should not repeat generic size metadata, got: {rendered:?}"
        );
        assert!(
            !rendered.contains("Modified "),
            "preview panel should not repeat generic modified metadata, got: {rendered:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn help_overlay_keeps_controls_readable_and_drops_auto_reload_row() {
        let root = temp_path("help-overlay-format");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        app.help_open = true;
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        draw_ui(&mut terminal, &mut app);
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(
            rendered.contains("Double-click"),
            "expected help overlay to keep the double-click label readable, got: {rendered:?}"
        );
        assert!(
            rendered.contains("open item"),
            "expected help overlay to keep the action text readable, got: {rendered:?}"
        );
        assert!(
            rendered.contains("Ctrl+F"),
            "expected help overlay to keep the file search shortcut visible, got: {rendered:?}"
        );
        assert!(
            !rendered.contains("Double clickopen"),
            "help overlay fused the key and action labels together: {rendered:?}"
        );
        assert!(
            !rendered.contains("current folder reloads itself"),
            "help overlay should not list auto-reload as a control: {rendered:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn entries_and_preview_panels_keep_top_border_segments() {
        let root = temp_path("panel-top-borders");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("report.txt"), "hello\nworld\n").expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        let state = draw_ui(&mut terminal, &mut app);
        let entries_panel = state
            .entries_panel
            .expect("entries panel should be rendered");
        let preview_panel = state
            .preview_panel
            .expect("preview panel should be rendered");

        let entries_top = row_text(terminal.backend().buffer(), entries_panel.y);
        let preview_top = row_text(terminal.backend().buffer(), preview_panel.y);

        assert!(
            entries_top.contains("─"),
            "expected entries panel to keep top border segments, got: {entries_top:?}"
        );
        assert!(
            preview_top.contains("─"),
            "expected preview panel to keep top border segments, got: {preview_top:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn preview_header_detail_uses_compact_labels_before_final_clamp() {
        let root = temp_path("preview-header-clamp");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let contents = (1..=300)
            .map(|index| format!("line {index} {}", "word ".repeat(30)))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(root.join("report.txt"), contents).expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("terminal should init");

        let state = draw_ui(&mut terminal, &mut app);
        let preview_panel = state
            .preview_panel
            .expect("preview panel should be rendered");
        let header_row = row_text(terminal.backend().buffer(), preview_panel.y + 1);

        assert!(
            header_row.contains("Text"),
            "expected preview header row to contain the section label, got: {header_row:?}"
        );
        assert!(
            header_row.contains("240 / 300 lines shown"),
            "expected preview header row to show semantic line coverage, got: {header_row:?}"
        );
        assert!(
            !header_row.contains("240-line cap"),
            "expected preview header row to avoid internal cap wording, got: {header_row:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn visible_directory_rows_show_cached_item_counts() {
        let root = temp_path("directory-counts");
        let photos = root.join("photos");
        fs::create_dir_all(&photos).expect("failed to create folder");
        fs::write(photos.join("one.jpg"), "a").expect("failed to write first file");
        fs::write(photos.join("two.jpg"), "b").expect("failed to write second file");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        draw_ui(&mut terminal, &mut app);
        wait_for_directory_counts(&mut app);
        draw_ui(&mut terminal, &mut app);

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(
            rendered.contains("2 items"),
            "expected visible directory rows to show cached item counts, got: {rendered:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn compact_list_rows_keep_metadata_visible_for_wide_names() {
        let root = temp_path("wide-list-metadata");
        let series = root.join("北斗の拳究極版北斗の拳究極版北斗の拳究極版北斗の拳究極版");
        fs::create_dir_all(&series).expect("failed to create series folder");
        for index in 0..10 {
            fs::write(series.join(format!("chapter-{index}.txt")), "x")
                .expect("failed to write child file");
        }

        let epub_path =
            root.join("北斗の拳究極版北斗の拳究極版北斗の拳究極版北斗の拳究極版13.epub");
        let epub = fs::File::create(&epub_path).expect("failed to create epub");
        epub.set_len(13_000_000).expect("failed to size epub");

        let mut app = App::new_at(root.clone()).expect("app should load temp directory");
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).expect("terminal should init");

        draw_ui(&mut terminal, &mut app);
        wait_for_directory_counts(&mut app);
        draw_ui(&mut terminal, &mut app);

        let rows = (0..terminal.backend().buffer().area.height)
            .map(|y| row_text(terminal.backend().buffer(), y))
            .collect::<Vec<_>>();
        let rendered = rows.join("\n");
        let folder_row = rows
            .iter()
            .find(|row| row.contains("10 items"))
            .expect("folder row should keep its item count visible");
        let epub_row = rows
            .iter()
            .find(|row| row.contains("13 MB"))
            .expect("epub row should keep its size visible");

        assert!(
            folder_row.contains("ago"),
            "expected wide directory rows to keep modified timestamps visible, got: {folder_row:?}"
        );
        assert!(
            epub_row.contains("ago"),
            "expected wide epub rows to keep modified timestamps visible, got: {epub_row:?}"
        );
        assert!(
            rendered.contains("10 items") && rendered.contains("13 MB"),
            "expected wide-name rows to keep full metadata visible, got: {rendered:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
