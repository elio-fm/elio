use super::super::theme::Palette;
use super::super::{helpers, theme};
use super::entries::{browser_entry_detail, browser_entry_modified};
use super::scrollbar::{render_browser_scrollbar, split_scrollbar_area};
use crate::app::{App, ClipOp, Entry, EntryHit, FrameState, ViewMetrics};
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

pub(super) fn render_grid(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let (content_area, scrollbar_area) = split_scrollbar_area(area);

    helpers::fill_area(frame, content_area, palette.panel_alt, palette.text);
    if let Some(sb) = scrollbar_area {
        helpers::fill_area(frame, sb, palette.panel_alt, palette.border);
    }

    let spec = helpers::grid_zoom_spec(app.navigation.zoom_level);
    let gap_x = spec.gap_x;
    let gap_y = spec.gap_y;
    let cols = ((content_area.width + gap_x) / (spec.tile_width_hint + gap_x)).max(1) as usize;
    let total_gap_x = gap_x.saturating_mul(cols.saturating_sub(1) as u16);
    let tile_width =
        (content_area.width.saturating_sub(total_gap_x) / cols as u16).max(spec.min_tile_width);
    let rows_visible = ((content_area.height + gap_y) / (spec.tile_height + gap_y)).max(1) as usize;
    state.metrics = ViewMetrics { cols, rows_visible };

    if app.navigation.entries.is_empty() {
        helpers::render_empty_state(frame, content_area, "This folder is empty", palette);
        return;
    }

    let start = app.navigation.scroll_row * cols;
    let limit = rows_visible * cols;

    for (visible_index, entry_index) in (start..app.navigation.entries.len())
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
        let entry = &app.navigation.entries[entry_index];
        let tile_state = TileState {
            selected: entry_index == app.navigation.selected,
            multi_selected: app.is_selected(&entry.path),
            clip_op: app.clipboard_op_for(&entry.path),
        };
        render_tile(frame, rect, app, entry, tile_state, palette, spec);
        state.entry_hits.push(EntryHit {
            rect,
            index: entry_index,
        });
    }

    if let Some(sb) = scrollbar_area {
        let total_rows = app.navigation.entries.len().div_ceil(cols);
        render_browser_scrollbar(
            frame,
            sb,
            total_rows,
            rows_visible,
            app.navigation.scroll_row,
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
    spec: helpers::GridZoomSpec,
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
            content_inner,
        );
    }
}
