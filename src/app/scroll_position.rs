pub(crate) fn scroll_top_for_cursor(cursor_line: usize, visible_rows: usize) -> usize {
    if cursor_line < visible_rows {
        0
    } else {
        cursor_line - visible_rows + 1
    }
}

pub(crate) fn scroll_top_for_cursor_with_override(
    cursor_line: usize,
    visible_rows: usize,
    total_rows: usize,
    scroll_top_override: Option<usize>,
) -> usize {
    let visible_rows = visible_rows.max(1);
    let Some(scroll_top) = scroll_top_override else {
        return scroll_top_for_cursor(cursor_line, visible_rows);
    };

    let max_scroll = total_rows.saturating_sub(visible_rows);
    let scroll_top = scroll_top.min(max_scroll);
    if cursor_line < scroll_top {
        cursor_line.min(max_scroll)
    } else if cursor_line >= scroll_top.saturating_add(visible_rows) {
        cursor_line
            .saturating_add(1)
            .saturating_sub(visible_rows)
            .min(max_scroll)
    } else {
        scroll_top
    }
}
