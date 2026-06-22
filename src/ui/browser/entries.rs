use super::super::theme::Palette;
use super::super::{helpers, theme};
use super::{grid::render_grid, list::render_list};
use crate::app::{
    App, ClipOp, Entry, FrameState, format_size, format_size_parts, format_time_ago,
    sanitize_terminal_text,
};
use crate::fs::symlink_target_display_label;
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
    let path_text =
        helpers::stable_path_label(&app.navigation.cwd, area.width.saturating_sub(10) as usize);
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

    if app.navigation.view_mode == crate::app::ViewMode::Grid {
        render_grid(frame, inner, app, state, palette);
    } else {
        render_list(frame, inner, app, state, palette);
    }
}

pub(super) fn browser_entry_detail(app: &App, entry: &Entry) -> Option<String> {
    if let Some(target) = browser_symlink_target_detail(entry) {
        Some(target)
    } else if entry.is_dir() {
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
    if let Some(target) = browser_symlink_target_detail(entry) {
        let mut parts = vec![target];
        if !entry.is_broken_symlink()
            && let Some(count) = app.directory_item_count_label(entry)
        {
            parts.push(count);
        }
        parts.push(browser_entry_modified(entry));
        return parts.join("  •  ");
    }

    match app.directory_item_count_label(entry) {
        Some(count) => format!("{count}  •  {}", browser_entry_modified(entry)),
        None => browser_entry_modified(entry),
    }
}

pub(super) fn browser_symlink_target_detail(entry: &Entry) -> Option<String> {
    let target = symlink_target_label(entry)?;
    Some(if entry.is_broken_symlink() {
        format!("broken -> {target}")
    } else {
        format!("-> {target}")
    })
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
    let clip_op = app.clipboard_op_for(&entry.path);
    // Reserve a fixed 2-column git slot (badge letter + gap) for every row when
    // inside a repository, so file names stay aligned whether or not a given
    // file has changes.
    let git_active = app.git_is_active();
    let git_status = if git_active {
        app.git_entry_status(entry)
    } else {
        None
    };
    let git_reserved = if git_active { 2u16 } else { 0 };
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
    let available_width = row_width
        .saturating_sub(COMPACT_PREFIX_WIDTH as u16)
        .saturating_sub(git_reserved)
        .max(1) as usize;
    let min_name_width = available_width.min(COMPACT_NAME_MIN_WIDTH);
    let symlink_target = symlink_target_label(entry);
    let show_inline_symlink_target =
        symlink_target.is_some() && available_width >= COMPACT_SYMLINK_INLINE_MIN_WIDTH;
    let detail_text = if show_inline_symlink_target {
        String::new()
    } else {
        compact_browser_entry_detail(app, entry, COMPACT_DETAIL_SLOT_WIDTH).unwrap_or_default()
    };
    let detail_slot_width = if detail_text.is_empty() {
        0
    } else {
        COMPACT_DETAIL_SLOT_WIDTH
    };
    let modified_text = browser_entry_modified(entry);

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
    if git_active {
        match git_status {
            Some(status) => {
                let (badge, color) = theme::git_status_badge(status);
                spans.push(Span::styled(
                    badge.to_string(),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::raw(" "));
            }
            None => spans.push(Span::raw("  ")),
        }
    }
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

fn symlink_target_label(entry: &Entry) -> Option<String> {
    entry.symlink.as_ref().map(symlink_target_display_label)
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

fn compact_browser_entry_detail(app: &App, entry: &Entry, width: usize) -> Option<String> {
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
    let exact = count.to_string();
    if helpers::display_width(&exact) <= width {
        return exact;
    }

    for (divisor, suffix) in [
        (1_000_000_000_000usize, "T"),
        (1_000_000_000usize, "B"),
        (1_000_000usize, "M"),
        (1_000usize, "K"),
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
