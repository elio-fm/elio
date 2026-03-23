use super::super::theme::Palette;
use super::super::{helpers, theme};
use super::entries::{
    browser_directory_secondary, browser_entry_detail, browser_entry_modified,
    render_compact_list_row,
};
use super::scrollbar::{render_browser_scrollbar, split_scrollbar_area};
use crate::app::{App, ClipOp, EntryHit, FrameState, ViewMetrics};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub(super) fn render_list(
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

    let row_height = helpers::list_row_height(app.zoom_level);
    state.metrics = ViewMetrics {
        cols: 1,
        rows_visible: (content_area.height / row_height.max(1)).max(1) as usize,
    };

    if app.entries.is_empty() {
        helpers::render_empty_state(frame, content_area, "This folder is empty", palette);
        return;
    }

    for (visible_index, entry_index) in (app.scroll_row..app.entries.len())
        .take(state.metrics.rows_visible)
        .enumerate()
    {
        let entry = &app.entries[entry_index];
        let row = Rect {
            x: content_area.x,
            y: content_area.y + visible_index as u16 * row_height,
            width: content_area.width,
            height: row_height,
        };
        let selected = entry_index == app.selected;
        let multi_selected = app.is_selected(&entry.path);
        let clip_op = app.clipboard_op_for(&entry.path);
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

    if let Some(sb) = scrollbar_area {
        render_browser_scrollbar(
            frame,
            sb,
            app.entries.len(),
            state.metrics.rows_visible,
            app.scroll_row,
            palette,
        );
    }
}
