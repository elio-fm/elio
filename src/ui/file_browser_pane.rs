use super::helpers;
use super::scrollbars::{render_browser_scrollbar, split_scrollbar_area};
use crate::app::{App, EntryHit, ScreenRegions, ViewMetrics};
use crate::file_browser::ViewMode;
use crate::file_operations::ClipOp;
use crate::filesystem::{
    Entry, format_size, format_size_parts, format_time_ago, sanitize_terminal_text,
    symlink_target_display_label,
};
use crate::theme::{self, Palette};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub(super) fn render_file_browser_pane(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut ScreenRegions,
    palette: Palette,
) {
    state.entries_panel = Some(area);
    let path_text = helpers::stable_path_label(
        &app.file_browser.cwd,
        area.width.saturating_sub(10) as usize,
    );
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

    if app.file_browser.view_mode == ViewMode::Grid {
        render_grid_view(frame, inner, app, state, palette);
    } else {
        render_list_view(frame, inner, app, state, palette);
    }
}

fn entry_detail(app: &App, entry: &Entry) -> Option<String> {
    if let Some(target) = symlink_target_detail(entry) {
        Some(target)
    } else if entry.is_dir() {
        app.directory_item_count_label(entry)
    } else {
        Some(format_size(entry.size))
    }
}

fn entry_modified(entry: &Entry) -> String {
    entry
        .modified
        .map(format_time_ago)
        .unwrap_or_else(|| "unknown".to_string())
}

fn directory_secondary(app: &App, entry: &Entry) -> String {
    if let Some(target) = symlink_target_detail(entry) {
        let mut parts = vec![target];
        if !entry.is_broken_symlink()
            && let Some(count) = app.directory_item_count_label(entry)
        {
            parts.push(count);
        }
        parts.push(entry_modified(entry));
        return parts.join("  •  ");
    }

    match app.directory_item_count_label(entry) {
        Some(count) => format!("{count}  •  {}", entry_modified(entry)),
        None => entry_modified(entry),
    }
}

pub(super) fn symlink_target_detail(entry: &Entry) -> Option<String> {
    let target = symlink_target_label(entry)?;
    Some(if entry.is_broken_symlink() {
        format!("broken -> {target}")
    } else {
        format!("-> {target}")
    })
}

fn symlink_target_label(entry: &Entry) -> Option<String> {
    entry.symlink.as_ref().map(symlink_target_display_label)
}

fn render_list_view(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut ScreenRegions,
    palette: Palette,
) {
    let (content_area, scrollbar_area) = split_scrollbar_area(area);

    helpers::fill_area(frame, content_area, palette.panel_alt, palette.text);
    if let Some(sb) = scrollbar_area {
        helpers::fill_area(frame, sb, palette.panel_alt, palette.border);
    }

    const ROW_HEIGHT: u16 = 1;
    let row_height = ROW_HEIGHT;
    state.metrics = ViewMetrics {
        cols: 1,
        rows_visible: (content_area.height / row_height.max(1)).max(1) as usize,
    };

    if app.file_browser.entries.is_empty() {
        let message = if app.local_filter_has_query() {
            "No matches"
        } else {
            "This folder is empty"
        };
        helpers::render_empty_state(frame, content_area, message, palette);
        return;
    }

    for (visible_index, entry_index) in (app.file_browser.scroll_row
        ..app.file_browser.entries.len())
        .take(state.metrics.rows_visible)
        .enumerate()
    {
        let entry = &app.file_browser.entries[entry_index];
        let row = Rect {
            x: content_area.x,
            y: content_area.y + visible_index as u16 * row_height,
            width: content_area.width,
            height: row_height,
        };
        let selected = entry_index == app.file_browser.selected;
        let multi_selected = app.is_selected(&entry.path);
        let clip_op = app.file_operations.clipboard_op_for(&entry.path);
        let appearance = theme::resolve_browser_entry(entry);
        let icon_color = appearance.color;
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
            // All mark states take priority over the cursor colour for the bar —
            // the cursor position is already communicated by the row background.
            let bar_color = if clip_op == Some(ClipOp::Yank) {
                palette.yank_bar
            } else if clip_op == Some(ClipOp::Cut) {
                palette.cut_bar
            } else if multi_selected {
                palette.selection_bar
            } else if selected {
                palette.selected_border
            } else {
                bg
            };
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(row);
            frame.render_widget(
                Paragraph::new(if selected || multi_selected || clip_op.is_some() {
                    "▌"
                } else {
                    " "
                })
                .alignment(Alignment::Left)
                .style(Style::default().bg(bg).fg(bar_color)),
                columns[0],
            );
            let secondary = if entry.is_dir() {
                directory_secondary(app, entry)
            } else if row_height >= 3 {
                format!(
                    "{}  •  {}",
                    entry_detail(app, entry).unwrap_or_default(),
                    entry_modified(entry)
                )
            } else {
                entry_detail(app, entry).unwrap_or_default()
            };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(
                            appearance.icon,
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

    if let Some(sb) = scrollbar_area {
        render_browser_scrollbar(
            frame,
            sb,
            app.file_browser.entries.len(),
            state.metrics.rows_visible,
            app.file_browser.scroll_row,
            palette,
        );
    }
}

pub(super) fn render_compact_list_row(
    app: &App,
    entry: &Entry,
    selected: bool,
    row_width: u16,
    palette: Palette,
) -> Line<'static> {
    const COMPACT_PREFIX_WIDTH: usize = 4;
    const COMPACT_NAME_MIN_WIDTH: usize = 18;
    const COMPACT_NAME_SOFT_MAX_WIDTH: usize = 56;
    const COMPACT_DETAIL_SLOT_WIDTH: usize = 10;
    const COMPACT_MODIFIED_SLOT_WIDTH: usize = 10;
    const COMPACT_METADATA_LEADING_GAP: usize = 2;
    const COMPACT_METADATA_COLUMN_GAP: usize = 1;
    const COMPACT_MAX_TRAILING_GAP: usize = 1;
    const COMPACT_SYMLINK_INLINE_MIN_WIDTH: usize = 12;

    let multi_selected = app.is_selected(&entry.path);
    let clip_op = app.file_operations.clipboard_op_for(&entry.path);
    // All mark states take priority over the cursor colour for the bar — the
    // cursor position is already communicated by the row background.
    let marker_color = if clip_op == Some(ClipOp::Yank) {
        palette.yank_bar
    } else if clip_op == Some(ClipOp::Cut) {
        palette.cut_bar
    } else if multi_selected {
        palette.selection_bar
    } else if selected {
        palette.selected_border
    } else {
        palette.panel_alt
    };
    let appearance = theme::resolve_browser_entry(entry);
    let icon = appearance.icon;
    let icon_style = Style::default()
        .fg(appearance.color)
        .add_modifier(Modifier::BOLD);
    let name_style = if selected {
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else if clip_op == Some(ClipOp::Cut) {
        Style::default().fg(palette.muted)
    } else {
        Style::default().fg(palette.text)
    };
    let muted_style = Style::default().fg(palette.muted);
    let available_width = row_width.saturating_sub(COMPACT_PREFIX_WIDTH as u16).max(1) as usize;
    let min_name_width = available_width.min(COMPACT_NAME_MIN_WIDTH);
    let symlink_target = symlink_target_label(entry);
    let show_inline_symlink_target =
        symlink_target.is_some() && available_width >= COMPACT_SYMLINK_INLINE_MIN_WIDTH;
    let detail_text = if show_inline_symlink_target {
        String::new()
    } else {
        compact_entry_detail(app, entry, COMPACT_DETAIL_SLOT_WIDTH).unwrap_or_default()
    };
    let detail_slot_width = if detail_text.is_empty() {
        0
    } else {
        COMPACT_DETAIL_SLOT_WIDTH
    };
    let modified_text = entry_modified(entry);

    let mut reserved_metadata_width = 0usize;
    let mut show_detail = false;
    let mut show_modified = false;

    if !show_inline_symlink_target
        && detail_slot_width > 0
        && available_width
            >= min_name_width
                .saturating_add(COMPACT_METADATA_LEADING_GAP)
                .saturating_add(detail_slot_width)
    {
        show_detail = true;
        reserved_metadata_width = reserved_metadata_width
            .saturating_add(COMPACT_METADATA_LEADING_GAP)
            .saturating_add(detail_slot_width);
    }
    if !show_inline_symlink_target
        && available_width
            >= min_name_width
                .saturating_add(reserved_metadata_width)
                .saturating_add(if show_detail {
                    COMPACT_METADATA_COLUMN_GAP + 1
                } else {
                    COMPACT_METADATA_LEADING_GAP
                })
                .saturating_add(COMPACT_MODIFIED_SLOT_WIDTH)
    {
        show_modified = true;
        reserved_metadata_width = reserved_metadata_width
            .saturating_add(if show_detail {
                COMPACT_METADATA_COLUMN_GAP + 1
            } else {
                COMPACT_METADATA_LEADING_GAP
            })
            .saturating_add(COMPACT_MODIFIED_SLOT_WIDTH);
    }

    let max_name_width = available_width
        .saturating_sub(reserved_metadata_width)
        .max(1);
    let trailing_gap_width = max_name_width
        .saturating_sub(COMPACT_NAME_SOFT_MAX_WIDTH)
        .min(COMPACT_MAX_TRAILING_GAP);
    let name_width = max_name_width
        .saturating_sub(trailing_gap_width)
        .max(min_name_width);
    let mut spans = vec![
        Span::styled(
            if selected || multi_selected || clip_op.is_some() {
                "▌"
            } else {
                " "
            },
            Style::default().fg(marker_color),
        ),
        Span::raw(" "),
        Span::styled(icon.to_string(), icon_style),
        Span::raw(" "),
    ];
    spans.extend(compact_entry_name_spans(
        entry,
        name_width,
        symlink_target.as_deref(),
        show_inline_symlink_target,
        name_style,
        muted_style,
    ));
    if show_detail {
        spans.push(Span::raw(if show_modified { "   " } else { "  " }));
        spans.push(Span::styled(detail_text, muted_style));
    }
    if show_modified {
        spans.push(Span::raw(if show_detail { " " } else { "  " }));
        spans.push(Span::styled(
            pad_left(modified_text, COMPACT_MODIFIED_SLOT_WIDTH),
            muted_style,
        ));
    }

    Line::from(spans)
}

fn compact_entry_name_spans(
    entry: &Entry,
    width: usize,
    symlink_target: Option<&str>,
    show_symlink_target: bool,
    name_style: Style,
    muted_style: Style,
) -> Vec<Span<'static>> {
    if show_symlink_target && let Some(target) = symlink_target {
        // Broken links already carry the error state through icon/color, so keep
        // the inline suffix focused on the target path.
        let suffix = format!(" -> {target}");
        return compact_symlink_name_spans(&entry.name, &suffix, width, name_style, muted_style);
    }

    vec![Span::styled(
        pad_right(helpers::clamp_label(&entry.name, width), width),
        name_style,
    )]
}

fn compact_symlink_name_spans(
    name: &str,
    suffix: &str,
    width: usize,
    name_style: Style,
    muted_style: Style,
) -> Vec<Span<'static>> {
    const MIN_NAME_WIDTH: usize = 4;
    const MIN_SUFFIX_WIDTH: usize = 8;

    let name = sanitize_terminal_text(name);
    let suffix = sanitize_terminal_text(suffix);

    if width < MIN_NAME_WIDTH + MIN_SUFFIX_WIDTH {
        return vec![Span::styled(
            pad_right(
                helpers::truncate_middle(&format!("{name}{suffix}"), width),
                width,
            ),
            name_style,
        )];
    }

    let name_width = helpers::display_width(&name);
    let suffix_width = helpers::display_width(&suffix);
    let (name_text, suffix_text) = if name_width + suffix_width <= width {
        (name, suffix)
    } else {
        let suffix_slot = suffix_width
            .min((width / 2).max(MIN_SUFFIX_WIDTH))
            .min(width.saturating_sub(MIN_NAME_WIDTH));
        let name_slot = width.saturating_sub(suffix_slot).max(MIN_NAME_WIDTH);
        (
            helpers::clamp_label(&name, name_slot),
            clamp_symlink_suffix(&suffix, suffix_slot),
        )
    };

    let used_width = helpers::display_width(&name_text) + helpers::display_width(&suffix_text);
    let mut spans = vec![
        Span::styled(name_text, name_style),
        Span::styled(suffix_text, muted_style),
    ];
    if used_width < width {
        spans.push(Span::raw(" ".repeat(width - used_width)));
    }
    spans
}

fn clamp_symlink_suffix(suffix: &str, width: usize) -> String {
    const ARROW: &str = " -> ";

    if helpers::display_width(suffix) <= width {
        return suffix.to_string();
    }

    let arrow_width = helpers::display_width(ARROW);
    if width <= arrow_width + 1 {
        return helpers::clamp_label(suffix, width);
    }

    let target = suffix.strip_prefix(ARROW).unwrap_or(suffix);
    format!(
        "{ARROW}{}",
        helpers::truncate_middle(target, width.saturating_sub(arrow_width))
    )
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

fn compact_entry_detail(app: &App, entry: &Entry, width: usize) -> Option<String> {
    if entry.is_broken_symlink() {
        Some(helpers::clamp_label("broken", width))
    } else if entry.is_symlink() {
        Some(helpers::clamp_label("link", width))
    } else if entry.is_dir() {
        app.directory_item_count_value(entry)
            .map(|count| format_compact_directory_count(count, width))
    } else {
        Some(format_compact_file_size(entry.size, width))
    }
}

fn format_compact_directory_count(count: usize, width: usize) -> String {
    const NOUN_WIDTH: usize = 5;
    let quantity_width = width.saturating_sub(NOUN_WIDTH + 1);
    let quantity = format_compact_directory_quantity(count, quantity_width);
    let noun = if count == 1 { "item" } else { "items" };
    format_compact_measure(quantity, noun, width, NOUN_WIDTH)
}

fn format_compact_directory_quantity(count: usize, width: usize) -> String {
    let count = count as u64;
    let exact = count.to_string();
    if helpers::display_width(&exact) <= width {
        return exact;
    }

    for (divisor, suffix) in [
        (1_000_000_000_000u64, "T"),
        (1_000_000_000u64, "B"),
        (1_000_000u64, "M"),
        (1_000u64, "K"),
    ] {
        if count < divisor {
            continue;
        }

        let whole = count / divisor;
        if whole < 10 && width >= 4 {
            let tenth = (count % divisor) * 10 / divisor;
            if tenth > 0 {
                let decimal = format!("{whole}.{tenth}{suffix}");
                if helpers::display_width(&decimal) <= width {
                    return decimal;
                }
            }
        }

        let compact = format!("{whole}{suffix}");
        if helpers::display_width(&compact) <= width {
            return compact;
        }
    }

    helpers::clamp_label(&exact, width)
}

fn format_compact_file_size(size: u64, width: usize) -> String {
    const UNIT_WIDTH: usize = 2;

    let (quantity, unit) = format_size_parts(size);
    format_compact_measure(quantity, unit, width, UNIT_WIDTH)
}

fn format_compact_measure(
    quantity: String,
    suffix: &str,
    width: usize,
    suffix_width: usize,
) -> String {
    if width <= suffix_width + 1 {
        return helpers::clamp_label(&format!("{quantity} {suffix}"), width);
    }

    let quantity_width = width - suffix_width - 1;
    let quantity = if helpers::display_width(&quantity) > quantity_width {
        helpers::clamp_label(&quantity, quantity_width)
    } else {
        pad_left(quantity, quantity_width)
    };

    format!("{quantity} {}", pad_right(suffix.to_string(), suffix_width))
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
            tile_width_hint: 16,
            min_tile_width: 14,
            tile_height: 2,
            gap_x: 1,
            gap_y: 1,
            padding_x: 1,
            emphasize_icon: false,
            show_kind_hint: false,
        },
        1 => GridZoomSpec {
            tile_width_hint: 20,
            min_tile_width: 18,
            tile_height: 3,
            gap_x: 1,
            gap_y: 1,
            padding_x: 1,
            emphasize_icon: false,
            show_kind_hint: false,
        },
        2 => GridZoomSpec {
            tile_width_hint: 24,
            min_tile_width: 21,
            tile_height: 5,
            gap_x: 2,
            gap_y: 1,
            padding_x: 2,
            emphasize_icon: true,
            show_kind_hint: false,
        },
        _ => GridZoomSpec {
            tile_width_hint: 24,
            min_tile_width: 21,
            tile_height: 5,
            gap_x: 2,
            gap_y: 1,
            padding_x: 2,
            emphasize_icon: true,
            show_kind_hint: false,
        },
    }
}

fn render_grid_view(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut ScreenRegions,
    palette: Palette,
) {
    let (content_area, scrollbar_area) = split_scrollbar_area(area);

    helpers::fill_area(frame, content_area, palette.panel_alt, palette.text);
    if let Some(sb) = scrollbar_area {
        helpers::fill_area(frame, sb, palette.panel_alt, palette.border);
    }

    let spec = grid_zoom_spec(app.file_browser.zoom_level);
    let gap_x = spec.gap_x;
    let gap_y = spec.gap_y;
    let cols = ((content_area.width + gap_x) / (spec.tile_width_hint + gap_x)).max(1) as usize;
    let total_gap_x = gap_x.saturating_mul(cols.saturating_sub(1) as u16);
    let tile_width =
        (content_area.width.saturating_sub(total_gap_x) / cols as u16).max(spec.min_tile_width);
    let rows_visible = ((content_area.height + gap_y) / (spec.tile_height + gap_y)).max(1) as usize;
    state.metrics = ViewMetrics { cols, rows_visible };

    if app.file_browser.entries.is_empty() {
        let message = if app.local_filter_has_query() {
            "No matches"
        } else {
            "This folder is empty"
        };
        helpers::render_empty_state(frame, content_area, message, palette);
        return;
    }

    let start = app.file_browser.scroll_row * cols;
    let limit = rows_visible * cols;

    for (visible_index, entry_index) in (start..app.file_browser.entries.len())
        .take(limit)
        .enumerate()
    {
        let row = visible_index / cols;
        let col = visible_index % cols;
        let tile_x = content_area.x + col as u16 * (tile_width + gap_x);
        let tile_y = content_area.y + row as u16 * (spec.tile_height + gap_y);
        // Last column in each row absorbs the integer-division remainder so there
        // is no dead pixel strip along the right edge of the content area.
        let actual_tile_width = if col == cols - 1 {
            (content_area.x + content_area.width).saturating_sub(tile_x)
        } else {
            tile_width
        };
        let rect = Rect {
            x: tile_x,
            y: tile_y,
            width: actual_tile_width,
            height: spec.tile_height,
        };
        let entry = &app.file_browser.entries[entry_index];
        let tile_state = TileState {
            selected: entry_index == app.file_browser.selected,
            multi_selected: app.is_selected(&entry.path),
            clip_op: app.file_operations.clipboard_op_for(&entry.path),
        };
        render_tile(frame, rect, app, entry, tile_state, palette, spec);
        state.entry_hits.push(EntryHit {
            rect,
            index: entry_index,
        });
    }

    if let Some(sb) = scrollbar_area {
        let total_rows = app.file_browser.entries.len().div_ceil(cols);
        render_browser_scrollbar(
            frame,
            sb,
            total_rows,
            rows_visible,
            app.file_browser.scroll_row,
            palette,
        );
    }
}

struct TileState {
    selected: bool,
    multi_selected: bool,
    clip_op: Option<ClipOp>,
}

fn render_tile(
    frame: &mut Frame<'_>,
    rect: Rect,
    app: &App,
    entry: &Entry,
    tile_state: TileState,
    palette: Palette,
    spec: GridZoomSpec,
) {
    let TileState {
        selected,
        multi_selected,
        clip_op,
    } = tile_state;
    let appearance = theme::resolve_browser_entry(entry);
    let icon_color = appearance.color;
    let background = palette.surface;
    let content_bg = if selected {
        theme::mix_color(palette.selected_bg, icon_color, 22)
    } else {
        palette.surface
    };
    // Band background carries the clipboard/selection state.  The cursor position
    // (selected) is already communicated by the content background tint and does
    // not change the band so tiles stay visually consistent while navigating.
    let band_bg = if clip_op == Some(ClipOp::Yank) {
        palette.grid_yank_band
    } else if clip_op == Some(ClipOp::Cut) {
        palette.grid_cut_band
    } else if multi_selected {
        palette.grid_selection_band
    } else {
        palette.elevated
    };
    let band_fg = palette.text;
    let band_icon = icon_color;
    let band_name_fg = band_fg;

    frame.render_widget(
        Block::default().style(Style::default().bg(background).fg(palette.text)),
        rect,
    );

    // ── Band (top row: icon + filename) ──────────────────────────────────────
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
                appearance.icon,
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
                    .fg(band_name_fg)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(band_bg).fg(band_fg)),
        band.inner(Margin {
            horizontal: 1,
            vertical: 0,
        }),
    );

    // ── Content body (below band) ─────────────────────────────────────────────
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
    let detail = entry_detail(app, entry);
    let modified = entry_modified(entry);
    let mut lines = Vec::new();
    if spec.show_kind_hint {
        lines.push(Line::from(Span::styled(
            entry_kind_hint(entry),
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
            content_inner,
        );
    }
}

fn entry_kind_hint(entry: &Entry) -> &'static str {
    if entry.is_broken_symlink() {
        "Broken link"
    } else if entry.is_symlink() {
        "Open link"
    } else if entry.is_dir() {
        "Open folder"
    } else {
        "Open file"
    }
}
