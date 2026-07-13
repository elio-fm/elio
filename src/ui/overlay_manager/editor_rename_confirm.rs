use super::scrollbar::render_overlay_scrollbar_on_bg;
use crate::app::{App, FrameState};
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

pub(super) fn render_editor_rename_confirm_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let item_count = app.editor_rename_confirm_count();
    let visible_lines = area
        .height
        .saturating_sub(7)
        .max(1)
        .min(item_count.clamp(1, 12) as u16);
    let popup_width = area.width.saturating_sub(8).clamp(48, 88);
    let popup_height = visible_lines + 5;
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.rename_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(
            &format!(" {} ", app.editor_rename_confirm_title()),
            palette.chrome_alt,
            palette,
        ),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(visible_lines + 2), Constraint::Length(1)])
        .split(inner);

    frame.render_widget(
        helpers::rounded_block(palette.path_bg, palette.border),
        rows[0],
    );
    let list_area = rows[0].inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    state.bulk_rename_list_area = Some(list_area);

    let max_scroll = item_count.saturating_sub(visible_lines as usize);
    let scroll_top = app.editor_rename_confirm_scroll().min(max_scroll);
    state.bulk_rename_scroll_top = scroll_top;
    let show_scrollbar = item_count > visible_lines as usize;
    let row_width = list_area
        .width
        .saturating_sub(if show_scrollbar { 1 } else { 0 });

    for row_offset in 0..visible_lines as usize {
        let index = scroll_top + row_offset;
        let Some((old, new)) = app.editor_rename_confirm_row(index) else {
            break;
        };
        let available = row_width as usize;
        let arrow_width = 4usize;
        let side_width = available.saturating_sub(arrow_width) / 2;
        let old = helpers::clamp_label(&old, side_width);
        let new = helpers::clamp_label(&new, available.saturating_sub(arrow_width + side_width));
        let row_rect = Rect {
            x: list_area.x,
            y: list_area.y + row_offset as u16,
            width: row_width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(old, Style::default().fg(palette.muted)),
                Span::styled(" -> ", Style::default().fg(palette.accent)),
                Span::styled(new, Style::default().fg(palette.text)),
            ]))
            .style(Style::default().bg(palette.path_bg).fg(palette.text)),
            row_rect,
        );
    }

    if show_scrollbar {
        let scrollbar_area = Rect {
            x: list_area.x + list_area.width.saturating_sub(1),
            width: 1,
            ..list_area
        };
        render_overlay_scrollbar_on_bg(
            frame,
            scrollbar_area,
            item_count,
            visible_lines as usize,
            scroll_top,
            palette,
            palette.path_bg,
        );
    }

    let confirmed = app.editor_rename_confirmed();
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
    state.editor_rename_confirm_btn = Some(Rect {
        x: confirm_x,
        y: btn_y,
        width: confirm_w,
        height: 1,
    });
    state.editor_rename_cancel_btn = Some(Rect {
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
