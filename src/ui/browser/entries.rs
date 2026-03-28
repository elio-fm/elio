use super::super::theme::Palette;
use super::super::{helpers, theme};
use super::{grid::render_grid, list::render_list};
use crate::app::{App, ClipOp, Entry, FrameState, format_size, format_time_ago};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};

pub(super) fn render_entries(
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

pub(super) fn browser_entry_detail(app: &App, entry: &Entry) -> Option<String> {
    if entry.is_dir() {
        app.directory_item_count_label(entry)
    } else {
        Some(format_size(entry.size))
    }
}

pub(super) fn browser_entry_modified(entry: &Entry) -> String {
    entry
        .modified
        .map(format_time_ago)
        .unwrap_or_else(|| "unknown".to_string())
}

pub(super) fn browser_directory_secondary(app: &App, entry: &Entry) -> String {
    match app.directory_item_count_label(entry) {
        Some(count) => format!("{count}  •  {}", browser_entry_modified(entry)),
        None => browser_entry_modified(entry),
    }
}

pub(super) fn render_compact_list_row(
    app: &App,
    entry: &Entry,
    selected: bool,
    row_width: u16,
    palette: Palette,
) -> Line<'static> {
    const COMPACT_PREFIX_WIDTH: u16 = 4;
    const COMPACT_NAME_MIN_WIDTH: usize = 18;
    const COMPACT_DETAIL_MAX_WIDTH: usize = 12;
    const COMPACT_MODIFIED_MAX_WIDTH: usize = 10;

    let multi_selected = app.is_selected(&entry.path);
    let clip_op = app.clipboard_op_for(&entry.path);
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
    let icon = theme::entry_symbol(entry);
    let icon_style = Style::default()
        .fg(theme::entry_color(entry, palette))
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
    let available_width = row_width.saturating_sub(COMPACT_PREFIX_WIDTH).max(1) as usize;
    let min_name_width = available_width.min(COMPACT_NAME_MIN_WIDTH);
    let detail_text = browser_entry_detail(app, entry).unwrap_or_default();
    let detail_width = if detail_text.is_empty() {
        0
    } else {
        helpers::display_width(&detail_text).min(COMPACT_DETAIL_MAX_WIDTH)
    };
    let modified_text = browser_entry_modified(entry);
    let modified_width = helpers::display_width(&modified_text).min(COMPACT_MODIFIED_MAX_WIDTH);

    let mut reserved_metadata_width = 0usize;
    let mut show_detail = false;
    let mut show_modified = false;

    if detail_width > 0 && available_width >= min_name_width.saturating_add(detail_width) {
        show_detail = true;
        reserved_metadata_width = reserved_metadata_width.saturating_add(detail_width);
    }
    if available_width
        >= min_name_width
            .saturating_add(reserved_metadata_width)
            .saturating_add(modified_width)
    {
        show_modified = true;
        reserved_metadata_width = reserved_metadata_width.saturating_add(modified_width);
    }

    let name_width = available_width
        .saturating_sub(reserved_metadata_width)
        .max(1);
    let name = helpers::clamp_label(&entry.name, name_width);
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
        Span::styled(pad_right(name, name_width), name_style),
    ];
    if show_detail {
        spans.push(Span::styled(
            pad_left(detail_text, detail_width),
            muted_style,
        ));
    }
    if show_modified {
        spans.push(Span::styled(
            pad_left(modified_text, modified_width),
            muted_style,
        ));
    }

    Line::from(spans)
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
