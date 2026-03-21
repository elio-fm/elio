use crate::app::{App, FrameState};
use crate::ui::{
    helpers,
    theme::{self, Palette},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

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
        .constraints([Constraint::Length(list_rows + 2), Constraint::Length(1)])
        .split(inner);

    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[0],
    );
    let list_area = rows[0].inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let visible = app.restore_visible_rows().max(1);
    let scroll = app.restore_scroll();

    let show_scrollbar = count > visible;
    let thumb_size = if show_scrollbar {
        (visible * visible / count).max(1)
    } else {
        0
    };
    let max_scroll = count.saturating_sub(visible);
    let thumb_pos = if max_scroll == 0 {
        0
    } else {
        scroll * (visible - thumb_size) / max_scroll
    };
    let bar_x = list_area.x + list_area.width.saturating_sub(1);

    for row_offset in 0..visible {
        let item_index = scroll + row_offset;
        let Some(name) = app.restore_target_name_at(item_index) else {
            break;
        };
        let is_dir = app.restore_target_is_dir_at(item_index);
        let (icon, icon_color) = app
            .restore_target_path_at(item_index)
            .map(|path| {
                (
                    theme::path_symbol(path, is_dir),
                    theme::path_color(path, is_dir, palette),
                )
            })
            .unwrap_or_else(|| {
                if is_dir {
                    ("󰉋", palette.accent)
                } else {
                    ("󰈔", palette.muted)
                }
            });
        let y = list_area.y + row_offset as u16;
        let row_width = list_area
            .width
            .saturating_sub(if show_scrollbar { 2 } else { 0 });
        let name_width = row_width.saturating_sub(2) as usize;
        let name_rect = Rect {
            x: list_area.x,
            y,
            width: row_width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    icon,
                    Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                ),
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
            let bar_color = if in_thumb {
                palette.muted
            } else {
                palette.path_bg
            };
            frame.buffer_mut()[(bar_x, y)].set_symbol(bar_char);
            frame.buffer_mut()[(bar_x, y)]
                .set_style(Style::default().bg(palette.path_bg).fg(bar_color));
        }
    }

    let confirmed = app.restore_confirmed();
    let confirm_style = if confirmed {
        Style::default()
            .bg(palette.selected_bg)
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(palette.chrome_alt).fg(palette.muted)
    };
    let cancel_style = if !confirmed {
        Style::default()
            .bg(palette.selected_bg)
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
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
    state.restore_confirm_btn = Some(Rect {
        x: confirm_x,
        y: btn_y,
        width: confirm_w,
        height: 1,
    });
    state.restore_cancel_btn = Some(Rect {
        x: cancel_x,
        y: btn_y,
        width: cancel_w,
        height: 1,
    });
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
