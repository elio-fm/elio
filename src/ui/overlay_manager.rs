use super::helpers;
use super::theme::{self, Palette};
use crate::app::{App, FrameState, SearchHit, SearchScope};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

pub(super) fn render_trash_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let block_title = format!(" {} ", app.trash_title());
    let count = app.trash_target_count();
    let list_rows = app.trash_visible_rows().max(1) as u16;
    // list box (border + rows + border) + buttons + inner_with_padding overhead
    let popup_height = (list_rows + 2) + 1 + 2;
    let popup_width = area.width.saturating_sub(8).clamp(40, 60);
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.trash_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(&block_title, palette.chrome_alt, palette),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(list_rows + 2), // list box (border + rows + border)
            Constraint::Length(1),             // buttons
        ])
        .split(inner);

    // Names list inside a rounded box
    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[0],
    );
    let list_area = rows[0].inner(Margin { horizontal: 1, vertical: 1 });
    let visible = app.trash_visible_rows().max(1);
    let scroll = app.trash_scroll();

    let show_scrollbar = count > visible;
    let thumb_size = if show_scrollbar { (visible * visible / count).max(1) } else { 0 };
    let max_scroll = count.saturating_sub(visible);
    let thumb_pos = if max_scroll == 0 { 0 } else { scroll * (visible - thumb_size) / max_scroll };
    let bar_x = list_area.x + list_area.width.saturating_sub(1);

    for row_offset in 0..visible {
        let item_index = scroll + row_offset;
        let Some(name) = app.trash_target_name_at(item_index) else { break };
        let is_dir = app.trash_target_is_dir_at(item_index);
        let (icon, icon_color) = app
            .trash_target_path_at(item_index)
            .map(|path| (theme::path_symbol(path, is_dir), theme::path_color(path, is_dir, palette)))
            .unwrap_or_else(|| if is_dir { ("󰉋", palette.accent) } else { ("󰈔", palette.muted) });
        let y = list_area.y + row_offset as u16;
        let row_width = list_area.width.saturating_sub(if show_scrollbar { 2 } else { 0 });
        let name_width = row_width.saturating_sub(2) as usize; // icon + 1 space
        let name_rect = Rect { x: list_area.x, y, width: row_width, height: 1 };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(icon, Style::default().fg(icon_color).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(
                    helpers::clamp_label(name, name_width),
                    Style::default().fg(palette.muted),
                ),
            ]))
            .style(Style::default().bg(palette.path_bg)),
            name_rect,
        );

        if show_scrollbar {
            let in_thumb = row_offset >= thumb_pos && row_offset < thumb_pos + thumb_size;
            let bar_char = if in_thumb { "▐" } else { " " };
            let bar_color = if in_thumb { palette.muted } else { palette.path_bg };
            frame.buffer_mut()[(bar_x, y)].set_symbol(bar_char);
            frame.buffer_mut()[(bar_x, y)]
                .set_style(Style::default().bg(palette.path_bg).fg(bar_color));
        }
    }

    // Buttons: [ Confirm ]  [ Cancel ] — confirm left, cancel right
    let confirmed = app.trash_confirmed();
    let confirm_style = if confirmed {
        Style::default().bg(palette.selected_bg).fg(palette.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(palette.chrome_alt).fg(palette.muted)
    };
    let cancel_style = if !confirmed {
        Style::default().bg(palette.selected_bg).fg(palette.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(palette.chrome_alt).fg(palette.muted)
    };
    let confirm_w = 11u16; // "  Confirm  "
    let cancel_w = 10u16;  // "  Cancel  "
    let gap = 3u16;
    let total_btn_width = confirm_w + gap + cancel_w;
    let left_pad = rows[1].width.saturating_sub(total_btn_width) / 2;
    let btn_y = rows[1].y;
    let confirm_x = rows[1].x + left_pad;
    let cancel_x = confirm_x + confirm_w + gap;
    state.trash_confirm_btn = Some(Rect { x: confirm_x, y: btn_y, width: confirm_w, height: 1 });
    state.trash_cancel_btn  = Some(Rect { x: cancel_x,  y: btn_y, width: cancel_w,  height: 1 });
    let pad = " ".repeat(left_pad as usize);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(pad, Style::default().bg(palette.chrome_alt)),
            Span::styled("  Confirm  ", confirm_style),
            Span::styled("   ", Style::default().bg(palette.chrome_alt)),
            Span::styled("  Cancel  ", cancel_style),
        ]))
        .style(Style::default().bg(palette.chrome_alt)),
        rows[1],
    );
}

pub(super) fn render_restore_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let block_title = format!(" {} ", app.restore_title());
    let count = app.restore_target_count();
    let list_rows = app.restore_visible_rows().max(1) as u16;
    let popup_height = (list_rows + 2) + 1 + 2;
    let popup_width = area.width.saturating_sub(8).clamp(40, 60);
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.restore_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(&block_title, palette.chrome_alt, palette),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(list_rows + 2),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[0],
    );
    let list_area = rows[0].inner(Margin { horizontal: 1, vertical: 1 });
    let visible = app.restore_visible_rows().max(1);
    let scroll = app.restore_scroll();

    let show_scrollbar = count > visible;
    let thumb_size = if show_scrollbar { (visible * visible / count).max(1) } else { 0 };
    let max_scroll = count.saturating_sub(visible);
    let thumb_pos = if max_scroll == 0 { 0 } else { scroll * (visible - thumb_size) / max_scroll };
    let bar_x = list_area.x + list_area.width.saturating_sub(1);

    for row_offset in 0..visible {
        let item_index = scroll + row_offset;
        let Some(name) = app.restore_target_name_at(item_index) else { break };
        let is_dir = app.restore_target_is_dir_at(item_index);
        let (icon, icon_color) = app
            .restore_target_path_at(item_index)
            .map(|path| (theme::path_symbol(path, is_dir), theme::path_color(path, is_dir, palette)))
            .unwrap_or_else(|| if is_dir { ("󰉋", palette.accent) } else { ("󰈔", palette.muted) });
        let y = list_area.y + row_offset as u16;
        let row_width = list_area.width.saturating_sub(if show_scrollbar { 2 } else { 0 });
        let name_width = row_width.saturating_sub(2) as usize; // icon + 1 space
        let name_rect = Rect { x: list_area.x, y, width: row_width, height: 1 };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(icon, Style::default().fg(icon_color).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(
                    helpers::clamp_label(name, name_width),
                    Style::default().fg(palette.muted),
                ),
            ]))
            .style(Style::default().bg(palette.path_bg)),
            name_rect,
        );

        if show_scrollbar {
            let in_thumb = row_offset >= thumb_pos && row_offset < thumb_pos + thumb_size;
            let bar_char = if in_thumb { "▐" } else { " " };
            let bar_color = if in_thumb { palette.muted } else { palette.path_bg };
            frame.buffer_mut()[(bar_x, y)].set_symbol(bar_char);
            frame.buffer_mut()[(bar_x, y)]
                .set_style(Style::default().bg(palette.path_bg).fg(bar_color));
        }
    }

    let confirmed = app.restore_confirmed();
    let confirm_style = if confirmed {
        Style::default().bg(palette.selected_bg).fg(palette.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(palette.chrome_alt).fg(palette.muted)
    };
    let cancel_style = if !confirmed {
        Style::default().bg(palette.selected_bg).fg(palette.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(palette.chrome_alt).fg(palette.muted)
    };
    let confirm_w = 11u16;
    let cancel_w = 10u16;
    let gap = 3u16;
    let total_btn_width = confirm_w + gap + cancel_w;
    let left_pad = rows[1].width.saturating_sub(total_btn_width) / 2;
    let btn_y = rows[1].y;
    let confirm_x = rows[1].x + left_pad;
    let cancel_x = confirm_x + confirm_w + gap;
    state.restore_confirm_btn = Some(Rect { x: confirm_x, y: btn_y, width: confirm_w, height: 1 });
    state.restore_cancel_btn  = Some(Rect { x: cancel_x,  y: btn_y, width: cancel_w,  height: 1 });
    let pad = " ".repeat(left_pad as usize);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(pad, Style::default().bg(palette.chrome_alt)),
            Span::styled("  Confirm  ", confirm_style),
            Span::styled("   ", Style::default().bg(palette.chrome_alt)),
            Span::styled("  Cancel  ", cancel_style),
        ]))
        .style(Style::default().bg(palette.chrome_alt)),
        rows[1],
    );
}

pub(super) fn render_create_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let line_count = app.create_line_count().max(1);
    let visible_lines = line_count.min(8) as u16;
    // Layout inside inner (inner_with_padding removes 2 from height):
    //   header(1) + list_box(visible_lines+2) + error(1) + footer(1) = visible_lines+5
    // popup_height = inner + 2 (inner_with_padding) = visible_lines+7
    let popup_width = area.width.saturating_sub(8).clamp(36, 64);
    let popup_height = visible_lines + 6;
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.create_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(&format!(" {} ", app.create_title()), palette.chrome_alt, palette),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                      // header hint
            Constraint::Length(visible_lines + 2),      // input box (border + lines)
            Constraint::Length(1),                      // error / spacer
        ])
        .split(inner);

    // -----------------------------------------------------------------------
    // Header — left: type hint, right: keybinding
    // -----------------------------------------------------------------------
    let header_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(18)])
        .split(rows[0]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("󰜄", Style::default().fg(palette.accent)),
            Span::raw("  "),
            Span::styled("/ → folder", Style::default().fg(palette.muted)),
        ]))
        .style(Style::default().bg(palette.chrome_alt)),
        header_cols[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Alt+Enter", Style::default().fg(palette.text)),
            Span::styled(" new line", Style::default().fg(palette.muted)),
        ]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(palette.chrome_alt)),
        header_cols[1],
    );

    // -----------------------------------------------------------------------
    // Input box (rounded border + lines inside)
    // -----------------------------------------------------------------------
    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[1],
    );
    let list_area = rows[1].inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    let cursor_line = app.create_cursor_line();
    let cursor_col = app.create_cursor_col();

    // Compute scroll_top so cursor stays visible.
    let scroll_top = compute_create_scroll_top(cursor_line, line_count, visible_lines as usize);
    state.create_list_area = Some(list_area);
    state.create_scroll_top = scroll_top;

    let show_scrollbar = line_count > visible_lines as usize;
    let thumb_size = if show_scrollbar {
        (visible_lines as usize * visible_lines as usize / line_count).max(1)
    } else {
        0
    };
    let max_scroll = line_count.saturating_sub(visible_lines as usize);
    let thumb_pos = if max_scroll == 0 {
        0
    } else {
        scroll_top * (visible_lines as usize - thumb_size) / max_scroll
    };
    let bar_x = list_area.x + list_area.width.saturating_sub(1);

    let mut cursor_screen_pos: Option<(u16, u16)> = None;

    for row_offset in 0..visible_lines as usize {
        let line_idx = scroll_top + row_offset;
        if line_idx >= line_count {
            break;
        }
        let line_text = app.create_line(line_idx);
        let is_cursor_line = line_idx == cursor_line;

        // Choose icon based on content (/ prefix or suffix → folder).
        let is_dir = line_text.starts_with('/') || line_text.ends_with('/');
        let clean_name = line_text.trim_matches('/');
        let (icon, icon_color) = if clean_name.is_empty() {
            // Empty input — show generic placeholder icon.
            if is_dir { ("󰉋", palette.accent) } else { ("󰈔", palette.muted) }
        } else {
            let path = app.cwd.join(clean_name);
            (theme::path_symbol(&path, is_dir), theme::path_color(&path, is_dir, palette))
        };

        // Icon takes 3 chars (icon + 2 spaces); scrollbar takes 2 when visible.
        let text_width = list_area.width
            .saturating_sub(3)
            .saturating_sub(if show_scrollbar { 2 } else { 0 }) as usize;
        let chars: Vec<char> = line_text.chars().collect();
        let col = if is_cursor_line {
            cursor_col.min(chars.len())
        } else {
            0
        };
        let h_start = if col > text_width { col - text_width } else { 0 };

        let mut visible_text: String = chars.iter().skip(h_start).take(text_width).collect();
        if h_start > 0 && !visible_text.is_empty() {
            visible_text.remove(0);
            visible_text.insert(0, '…');
        }

        let text_style = if is_cursor_line {
            Style::default().fg(palette.text).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text)
        };

        let line_widget = if line_text.is_empty() && is_cursor_line {
            Line::from(vec![
                Span::styled(icon, Style::default().fg(icon_color)),
                Span::raw("  "),
                Span::styled("name…", Style::default().fg(palette.muted)),
            ])
        } else {
            Line::from(vec![
                Span::styled(icon, Style::default().fg(icon_color)),
                Span::raw("  "),
                Span::styled(visible_text, text_style),
            ])
        };

        let row_rect = Rect {
            x: list_area.x,
            y: list_area.y + row_offset as u16,
            width: list_area.width.saturating_sub(if show_scrollbar { 2 } else { 0 }),
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(line_widget)
                .style(Style::default().bg(palette.path_bg).fg(palette.text)),
            row_rect,
        );

        if show_scrollbar {
            let y = list_area.y + row_offset as u16;
            let in_thumb = row_offset >= thumb_pos && row_offset < thumb_pos + thumb_size;
            let bar_char = if in_thumb { "▐" } else { " " };
            let bar_color = if in_thumb { palette.muted } else { palette.path_bg };
            frame.buffer_mut()[(bar_x, y)].set_symbol(bar_char);
            frame.buffer_mut()[(bar_x, y)]
                .set_style(Style::default().bg(palette.path_bg).fg(bar_color));
        }

        if is_cursor_line {
            let visible_col = col.saturating_sub(h_start);
            let cursor_x = row_rect.x + 3 + visible_col as u16;
            let cursor_x = cursor_x.min(row_rect.x + row_rect.width.saturating_sub(1));
            cursor_screen_pos = Some((cursor_x, row_rect.y));
        }
    }

    if let Some((cx, cy)) = cursor_screen_pos {
        frame.set_cursor_position((cx, cy));
    }

    // -----------------------------------------------------------------------
    // Error row (current cursor line's error, if any)
    // -----------------------------------------------------------------------
    if let Some(error) = app.create_line_error(cursor_line) {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                helpers::clamp_label(error, rows[2].width.saturating_sub(2) as usize),
                Style::default().fg(palette.accent),
            )]))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            rows[2],
        );
    }

}

fn compute_create_scroll_top(cursor_line: usize, line_count: usize, visible: usize) -> usize {
    let _ = line_count;
    if cursor_line < visible {
        0
    } else {
        cursor_line - visible + 1
    }
}

pub(super) fn render_bulk_rename_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let item_count = app.bulk_rename_item_count();
    let visible_lines = item_count.min(8) as u16;
    // inner layout: list_box(visible_lines+2) + error(1) = visible_lines+3
    // popup_height = visible_lines+3 + 2 (inner_with_padding margins) = visible_lines+5
    let popup_width = area.width.saturating_sub(8).clamp(40, 68);
    let popup_height = visible_lines + 5;
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    // Reuse rename_panel — single rename and bulk rename are mutually exclusive.
    state.rename_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(
            &format!(" {} ", app.bulk_rename_title()),
            palette.chrome_alt,
            palette,
        ),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(visible_lines + 2), // list box (border + rows + border)
            Constraint::Length(1),                 // error / spacer
        ])
        .split(inner);

    // -----------------------------------------------------------------------
    // List box
    // -----------------------------------------------------------------------
    frame.render_widget(helpers::rounded_block(palette.path_bg, palette.border), rows[0]);
    let list_area = rows[0].inner(Margin { horizontal: 1, vertical: 1 });

    let cursor_line = app.bulk_rename_cursor_line();
    let cursor_col = app.bulk_rename_cursor_col();

    let scroll_top = compute_create_scroll_top(cursor_line, item_count, visible_lines as usize);
    state.bulk_rename_list_area = Some(list_area);
    state.bulk_rename_scroll_top = scroll_top;

    let show_scrollbar = item_count > visible_lines as usize;
    let thumb_size = if show_scrollbar {
        (visible_lines as usize * visible_lines as usize / item_count).max(1)
    } else {
        0
    };
    let max_scroll = item_count.saturating_sub(visible_lines as usize);
    let thumb_pos = if max_scroll == 0 {
        0
    } else {
        scroll_top * (visible_lines as usize - thumb_size) / max_scroll
    };
    let bar_x = list_area.x + list_area.width.saturating_sub(1);

    let mut cursor_screen_pos: Option<(u16, u16)> = None;

    for row_offset in 0..visible_lines as usize {
        let line_idx = scroll_top + row_offset;
        if line_idx >= item_count {
            break;
        }

        let new_name = app.bulk_rename_new_name(line_idx);
        let is_dir = app.bulk_rename_item_is_dir(line_idx);
        let is_cursor_line = line_idx == cursor_line;

        // Resolve icon from the live edited name so it updates as the user types.
        let live_path = app.cwd.join(new_name);
        let (icon, icon_color) = (
            theme::path_symbol(&live_path, is_dir),
            theme::path_color(&live_path, is_dir, palette),
        );

        // Horizontal scrolling to keep cursor visible.
        let text_width = list_area
            .width
            .saturating_sub(3) // icon prefix
            .saturating_sub(if show_scrollbar { 2 } else { 0 })
            as usize;
        let chars: Vec<char> = new_name.chars().collect();
        let col = if is_cursor_line {
            cursor_col.min(chars.len())
        } else {
            0
        };
        let h_start = if col > text_width { col - text_width } else { 0 };
        let mut visible_text: String =
            chars.iter().skip(h_start).take(text_width).collect();
        if h_start > 0 && !visible_text.is_empty() {
            visible_text.remove(0);
            visible_text.insert(0, '…');
        }

        let text_style = if is_cursor_line {
            Style::default().fg(palette.text).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text)
        };

        let row_width =
            list_area.width.saturating_sub(if show_scrollbar { 2 } else { 0 });
        let row_rect = Rect {
            x: list_area.x,
            y: list_area.y + row_offset as u16,
            width: row_width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    icon,
                    Style::default()
                        .fg(icon_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(visible_text, text_style),
            ]))
            .style(Style::default().bg(palette.path_bg).fg(palette.text)),
            row_rect,
        );

        if show_scrollbar {
            let y = list_area.y + row_offset as u16;
            let in_thumb = row_offset >= thumb_pos && row_offset < thumb_pos + thumb_size;
            let bar_char = if in_thumb { "▐" } else { " " };
            let bar_color = if in_thumb { palette.muted } else { palette.path_bg };
            frame.buffer_mut()[(bar_x, y)].set_symbol(bar_char);
            frame.buffer_mut()[(bar_x, y)]
                .set_style(Style::default().bg(palette.path_bg).fg(bar_color));
        }

        if is_cursor_line {
            let visible_col = col.saturating_sub(h_start);
            let cursor_x = (row_rect.x + 3 + visible_col as u16)
                .min(row_rect.x + row_rect.width.saturating_sub(1));
            cursor_screen_pos = Some((cursor_x, row_rect.y));
        }
    }

    if let Some((cx, cy)) = cursor_screen_pos {
        frame.set_cursor_position((cx, cy));
    }

    // -----------------------------------------------------------------------
    // Error row
    // -----------------------------------------------------------------------
    if let Some(error) = app.bulk_rename_line_error(cursor_line) {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                helpers::clamp_label(error, rows[1].width.saturating_sub(2) as usize),
                Style::default().fg(palette.accent),
            )]))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            rows[1],
        );
    }
}

pub(super) fn render_rename_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let original = app.rename_original_name();
    let block_title = format!(" Rename \"{}\" ", helpers::clamp_label(original, 30));
    // Layout inside inner (inner_with_padding removes 2 from height):
    //   input_box(3) + error(1) = 4  →  popup_height = 4 + 2 = 6
    let popup_width = area.width.saturating_sub(8).clamp(40, 64);
    let popup_height = 6u16;
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.rename_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(&block_title, palette.chrome_alt, palette),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // input box (border + 1 line + border)
            Constraint::Length(1), // error / spacer
        ])
        .split(inner);

    // Input box
    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[0],
    );
    let input_area = rows[0].inner(Margin { horizontal: 1, vertical: 1 });

    let input = app.rename_input();
    let cursor_col = app.rename_cursor_col();
    let chars: Vec<char> = input.chars().collect();
    let col = cursor_col.min(chars.len());
    let available = input_area.width.saturating_sub(3) as usize; // 3 for icon prefix
    let h_start = if col > available { col - available } else { 0 };

    let mut visible_text: String = chars.iter().skip(h_start).take(available).collect();
    if h_start > 0 && !visible_text.is_empty() {
        visible_text.remove(0);
        visible_text.insert(0, '…');
    }

    let is_dir = app.rename_item_is_dir();
    let live_path = app.cwd.join(app.rename_input());
    let (icon, icon_color) = (
        theme::path_symbol(&live_path, is_dir),
        theme::path_color(&live_path, is_dir, palette),
    );

    let line = if input.is_empty() {
        Line::from(vec![
            Span::styled(icon, Style::default().fg(icon_color).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled("name…", Style::default().fg(palette.muted)),
        ])
    } else {
        Line::from(vec![
            Span::styled(icon, Style::default().fg(icon_color).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(
                visible_text,
                Style::default().fg(palette.text).add_modifier(Modifier::BOLD),
            ),
        ])
    };
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(palette.path_bg).fg(palette.text)),
        input_area,
    );

    let visible_cursor_col = col.saturating_sub(h_start);
    let cursor_x = (input_area.x + 3 + visible_cursor_col as u16)
        .min(input_area.x + input_area.width.saturating_sub(1));
    frame.set_cursor_position((cursor_x, input_area.y));

    // Error row
    if let Some(error) = app.rename_error() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                helpers::clamp_label(error, rows[1].width.saturating_sub(2) as usize),
                Style::default().fg(palette.accent),
            )]))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            rows[1],
        );
    }
}

pub(super) fn render_search_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let popup_width = area.width.saturating_sub(8).clamp(48, 88);
    let popup_height = area.height.saturating_sub(6).clamp(12, 22);
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.search_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(" Fuzzy Find ", palette.chrome_alt, palette),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(inner);

    let scope_label = app
        .search_scope()
        .map(|scope| scope.label())
        .unwrap_or("Search");
    let summary = if app.search_is_loading() {
        "indexing…".to_string()
    } else {
        format!(
            "{} results  •  {} indexed",
            app.search_match_count(),
            app.search_candidate_count()
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                scope_label,
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  current folder tree", Style::default().fg(palette.muted)),
            Span::raw("  "),
            helpers::chip_span(&summary, palette.accent_soft, palette.accent_text, true),
        ]))
        .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
        rows[0],
    );

    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[1],
    );
    let query = if app.search_query().is_empty() {
        match app.search_scope() {
            Some(SearchScope::Folders) => "type to filter folders".to_string(),
            Some(SearchScope::Files) => "type to filter files".to_string(),
            None => "type to filter results".to_string(),
        }
    } else {
        app.search_query().to_string()
    };
    let query_style = if app.search_query().is_empty() {
        Style::default().fg(palette.muted)
    } else {
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    };
    let query_area = rows[1].inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let (query_line, cursor_x) = if app.search_query().is_empty() {
        (
            Line::from(vec![
                Span::styled("󰍉", Style::default().fg(palette.accent)),
                Span::raw("  "),
                Span::styled(query, query_style),
            ]),
            query_area.x.saturating_add(3),
        )
    } else {
        render_query_line(
            app.search_query(),
            app.search_query_cursor(),
            query_area.width,
            query_area.x,
            palette,
        )
    };
    frame.render_widget(
        Paragraph::new(query_line).style(Style::default().bg(palette.path_bg).fg(palette.text)),
        query_area,
    );
    frame.set_cursor_position((cursor_x, query_area.y));

    let results_area = rows[2];
    let row_height = 2u16;
    let visible_rows = (results_area.height / row_height).max(1) as usize;
    state.search_rows_visible = visible_rows;

    let rows_data = app.search_rows(visible_rows);
    if app.search_is_loading() {
        helpers::render_empty_state(
            frame,
            results_area,
            "Indexing current folder tree…",
            palette,
        );
    } else if let Some(error) = app.search_error() {
        helpers::render_empty_state(
            frame,
            results_area,
            &helpers::truncate_middle(error, results_area.width.saturating_sub(4) as usize),
            palette,
        );
    } else if rows_data.is_empty() {
        helpers::render_empty_state(
            frame,
            results_area,
            app.search_scope()
                .map(|scope| scope.empty_label())
                .unwrap_or("No matches in this folder tree"),
            palette,
        );
    } else {
        for (offset, row) in rows_data.iter().enumerate() {
            let rect = Rect {
                x: results_area.x,
                y: results_area.y + offset as u16 * row_height,
                width: results_area.width,
                height: row_height.min(
                    results_area
                        .height
                        .saturating_sub(offset as u16 * row_height),
                ),
            };

            let bg = if row.selected {
                palette.selected_bg
            } else {
                palette.surface
            };
            frame.render_widget(Block::default().style(Style::default().bg(bg)), rect);
            if row.selected {
                frame.render_widget(
                    Paragraph::new("▎").style(Style::default().bg(bg).fg(palette.selected_border)),
                    Rect {
                        x: rect.x,
                        y: rect.y,
                        width: 1,
                        height: rect.height,
                    },
                );
            }

            let icon = theme::path_symbol(std::path::Path::new(&row.relative), row.is_dir);
            let icon_color =
                theme::path_color(std::path::Path::new(&row.relative), row.is_dir, palette);
            let name_width = rect.width.saturating_sub(6) as usize;
            let path_width = rect.width.saturating_sub(4) as usize;
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(icon, Style::default().fg(icon_color)),
                    Span::raw("  "),
                    Span::styled(
                        helpers::clamp_label(&row.name, name_width.max(8)),
                        Style::default()
                            .fg(palette.text)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
                .style(Style::default().bg(bg).fg(palette.text)),
                Rect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: 1,
                },
            );
            if rect.height > 1 {
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            helpers::stable_path_label(
                                std::path::Path::new(&row.relative),
                                path_width.max(10),
                            ),
                            Style::default().fg(palette.muted),
                        ),
                    ]))
                    .style(Style::default().bg(bg).fg(palette.muted)),
                    Rect {
                        x: rect.x,
                        y: rect.y + 1,
                        width: rect.width,
                        height: 1,
                    },
                );
            }

            state.search_hits.push(SearchHit {
                rect,
                index: row.index,
            });
        }
    }

    frame.render_widget(
        Paragraph::new("Enter open  Esc close  ↑↓ move")
            .alignment(Alignment::Right)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.muted)),
        rows[3],
    );
}

pub(super) fn render_help(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut FrameState,
    palette: Palette,
) {
    const NAVIGATION_ENTRIES: &[HelpEntry<'_>] = &[
        HelpEntry {
            key: "↑↓ / jk",
            action: "move selection",
        },
        HelpEntry {
            key: "← / h",
            action: "parent folder",
        },
        HelpEntry {
            key: "→ / l",
            action: "enter folder",
        },
        HelpEntry {
            key: "Enter",
            action: "open item",
        },
        HelpEntry {
            key: "Tab",
            action: "next pinned place",
        },
        HelpEntry {
            key: "Shift+Tab",
            action: "previous pinned place",
        },
        HelpEntry {
            key: "Backspace",
            action: "parent folder",
        },
        HelpEntry {
            key: "Alt+Left",
            action: "back",
        },
        HelpEntry {
            key: "Alt+Right",
            action: "forward",
        },
    ];
    const SEARCH_ENTRIES: &[HelpEntry<'_>] = &[
        HelpEntry {
            key: "f",
            action: "search folders",
        },
        HelpEntry {
            key: "Ctrl+F",
            action: "search files",
        },
        HelpEntry {
            key: "Ctrl+←→",
            action: "move by word",
        },
        HelpEntry {
            key: "Ctrl+Backspace",
            action: "delete previous word",
        },
        HelpEntry {
            key: "Ctrl+Del",
            action: "delete next word",
        },
        HelpEntry {
            key: "Ctrl+W / Alt+D",
            action: "fallback word delete",
        },
    ];
    const MOUSE_VIEW_ENTRIES: &[HelpEntry<'_>] = &[
        HelpEntry {
            key: "Click",
            action: "select item",
        },
        HelpEntry {
            key: "Double-click",
            action: "open item",
        },
        HelpEntry {
            key: "Wheel",
            action: "move selection",
        },
        HelpEntry {
            key: "Shift+Wheel",
            action: "scroll code sideways",
        },
        HelpEntry {
            key: "v",
            action: "toggle grid/list",
        },
        HelpEntry {
            key: ".",
            action: "toggle dotfiles",
        },
        HelpEntry {
            key: "s",
            action: "cycle sort",
        },
        HelpEntry {
            key: "o",
            action: "open externally",
        },
        HelpEntry {
            key: "Space",
            action: "toggle selection",
        },
        HelpEntry {
            key: "Ctrl+A",
            action: "select all",
        },
        HelpEntry {
            key: "Esc",
            action: "clear selection",
        },
        HelpEntry {
            key: "a",
            action: "create file or folder",
        },
        HelpEntry {
            key: "d",
            action: "trash (delete if in trash)",
        },
        HelpEntry {
            key: "r / F2",
            action: "rename",
        },
        HelpEntry {
            key: "r (in trash)",
            action: "restore from trash",
        },
    ];
    const LEFT_SECTIONS: &[HelpSection<'_>] = &[
        HelpSection {
            title: "Navigation",
            entries: NAVIGATION_ENTRIES,
        },
        HelpSection {
            title: "Search",
            entries: SEARCH_ENTRIES,
        },
    ];
    const RIGHT_SECTIONS: &[HelpSection<'_>] = &[HelpSection {
        title: "Mouse + View",
        entries: MOUSE_VIEW_ENTRIES,
    }];

    let popup = helpers::centered_rect(area, 82, 22);
    state.help_panel = Some(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(" Controls ", palette.chrome_alt, palette),
        popup,
    );
    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "󰘳",
                    Style::default()
                        .fg(palette.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    "Keyboard and mouse controls",
                    Style::default()
                        .fg(palette.text)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                helpers::chip_span("navigate", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                helpers::chip_span("search", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                helpers::chip_span("mouse", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                helpers::chip_span("view", palette.accent_soft, palette.accent_text, true),
            ]),
        ])
        .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    frame.render_widget(
        Paragraph::new(help_column_lines(cols[0].width, LEFT_SECTIONS, palette))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[0],
    );

    frame.render_widget(
        Paragraph::new(help_column_lines(cols[1].width, RIGHT_SECTIONS, palette))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "? / Esc",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("close help", Style::default().fg(palette.muted)),
        ]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(palette.chrome_alt).fg(palette.muted)),
        rows[2],
    );
}

#[derive(Clone, Copy)]
struct HelpEntry<'a> {
    key: &'a str,
    action: &'a str,
}

#[derive(Clone, Copy)]
struct HelpSection<'a> {
    title: &'a str,
    entries: &'a [HelpEntry<'a>],
}

fn help_column_lines(
    width: u16,
    sections: &[HelpSection<'_>],
    palette: Palette,
) -> Vec<Line<'static>> {
    let content_width = width.max(1) as usize;
    let max_key_width = sections
        .iter()
        .flat_map(|section| section.entries.iter())
        .map(|entry| UnicodeWidthStr::width(entry.key))
        .max()
        .unwrap_or(0);
    let gap_width = 2usize;
    let mut key_width = max_key_width.min(16);
    let min_action_width = 14usize.min(content_width.saturating_sub(gap_width + 1));
    if key_width + gap_width + min_action_width > content_width {
        key_width = content_width.saturating_sub(gap_width + min_action_width);
    }
    key_width = key_width
        .max(4)
        .min(content_width.saturating_sub(gap_width + 1));
    let action_width = content_width.saturating_sub(key_width + gap_width).max(1);

    let mut lines = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let _ = index;
        lines.push(help_section_title(section.title, palette));
        for entry in section.entries {
            lines.extend(help_entry_lines(*entry, key_width, action_width, palette));
        }
    }
    lines
}

fn help_section_title(title: &str, palette: Palette) -> Line<'static> {
    Line::from(vec![Span::styled(
        title.to_string(),
        Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD),
    )])
}

fn help_entry_lines(
    entry: HelpEntry<'_>,
    key_width: usize,
    action_width: usize,
    palette: Palette,
) -> Vec<Line<'static>> {
    let mut wrapped_action = wrap_help_action(entry.action, action_width);
    if wrapped_action.is_empty() {
        wrapped_action.push(String::new());
    }

    let key_padding = " ".repeat(key_width.saturating_sub(UnicodeWidthStr::width(entry.key)));
    let continuation = " ".repeat(key_width + 2);
    let mut lines = Vec::with_capacity(wrapped_action.len());

    lines.push(Line::from(vec![
        Span::styled(
            format!("{}{}", entry.key, key_padding),
            Style::default()
                .fg(palette.accent_text)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(wrapped_action.remove(0), Style::default().fg(palette.muted)),
    ]));

    for line in wrapped_action {
        lines.push(Line::from(vec![
            Span::raw(continuation.clone()),
            Span::styled(line, Style::default().fg(palette.muted)),
        ]));
    }

    lines
}

fn wrap_help_action(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() || width == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for word in text.split_whitespace() {
        let word_width = UnicodeWidthStr::width(word);
        let separator_width = usize::from(!current.is_empty());
        if !current.is_empty() && current_width + separator_width + word_width > width {
            lines.push(current);
            current = word.to_string();
            current_width = word_width;
            continue;
        }

        if !current.is_empty() {
            current.push(' ');
            current_width += 1;
        }
        current.push_str(word);
        current_width += word_width;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn render_query_line(
    query: &str,
    cursor: usize,
    width: u16,
    origin_x: u16,
    palette: Palette,
) -> (Line<'static>, u16) {
    let chars = query.chars().collect::<Vec<_>>();
    let cursor = cursor.min(chars.len());
    let available = width.saturating_sub(3) as usize;

    let mut start = 0usize;
    if cursor > available {
        start = cursor - available;
    }
    let mut visible = chars.iter().skip(start).take(available).collect::<String>();
    if start > 0 && !visible.is_empty() {
        visible.remove(0);
        visible.insert(0, '…');
    }

    let visible_chars = visible.chars().collect::<Vec<_>>();
    let visible_cursor = cursor.saturating_sub(start).min(visible_chars.len());

    let mut spans = vec![
        Span::styled("󰍉", Style::default().fg(palette.accent)),
        Span::raw("  "),
    ];
    spans.push(Span::styled(
        visible,
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD),
    ));

    let cursor_x = origin_x
        .saturating_add(3)
        .saturating_add(visible_cursor as u16)
        .min(origin_x.saturating_add(width.saturating_sub(1)));
    (Line::from(spans), cursor_x)
}
