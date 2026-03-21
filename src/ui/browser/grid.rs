use super::super::theme::Palette;
use super::super::{helpers, theme};
use super::entries::{browser_entry_detail, browser_entry_modified};
use super::scrollbar::{render_browser_scrollbar, split_scrollbar_area};
use crate::app::{App, Entry, EntryHit, FrameState, ViewMetrics};
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

    let spec = helpers::grid_zoom_spec(app.zoom_level);
    let gap_x = spec.gap_x;
    let gap_y = spec.gap_y;
    let cols = ((content_area.width + gap_x) / (spec.tile_width_hint + gap_x)).max(1) as usize;
    let total_gap_x = gap_x.saturating_mul(cols.saturating_sub(1) as u16);
    let tile_width =
        (content_area.width.saturating_sub(total_gap_x) / cols as u16).max(spec.min_tile_width);
    let rows_visible = ((content_area.height + gap_y) / (spec.tile_height + gap_y)).max(1) as usize;
    state.metrics = ViewMetrics { cols, rows_visible };

    if app.entries.is_empty() {
        helpers::render_empty_state(frame, content_area, "This folder is empty", palette);
        return;
    }

    let start = app.scroll_row * cols;
    let limit = rows_visible * cols;

    for (visible_index, entry_index) in (start..app.entries.len()).take(limit).enumerate() {
        let row = visible_index / cols;
        let col = visible_index % cols;
        let rect = Rect {
            x: content_area.x + col as u16 * (tile_width + gap_x),
            y: content_area.y + row as u16 * (spec.tile_height + gap_y),
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

    if let Some(sb) = scrollbar_area {
        let total_rows = app.entries.len().div_ceil(cols);
        render_browser_scrollbar(frame, sb, total_rows, rows_visible, app.scroll_row, palette);
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
