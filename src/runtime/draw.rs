use super::terminal::{AppTerminal, clear_for_full_repaint};
use crate::{
    app::{self, App},
    ui,
};
use anyhow::Result;
use crossterm::{
    cursor::{RestorePosition, SavePosition},
    execute,
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};
use ratatui::{
    backend::CrosstermBackend,
    buffer::{Buffer, Cell, CellDiffOption},
    layout::Rect,
};
use std::io::{self, Write};

pub(super) fn draw_terminal_frame(terminal: &mut AppTerminal, app: &mut App) -> Result<bool> {
    execute!(terminal.backend_mut(), BeginSynchronizedUpdate)?;

    let draw_result = (|| -> Result<bool> {
        if app.take_pending_resize_clear() {
            clear_for_full_repaint(terminal)?;
        }

        // Erase stale image cells before terminal.draw() so ratatui can
        // overpaint them with the correct panel background in the same pass.
        // - iTerm2: images are drawn at pixel level; erasing prevents ghost pixels.
        // - Kitty unicode placeholder: placeholder chars are terminal cells;
        //   ratatui's differential renderer skips "unchanged" cells leaving
        //   stale image content visible after navigation or resize.
        let pre_erase = app.iterm_pre_draw_erase();
        let kitty_erase = app.kitty_pre_draw_erase();
        if !pre_erase.is_empty() || !kitty_erase.is_empty() {
            terminal.backend_mut().write_all(&pre_erase)?;
            terminal.backend_mut().write_all(&kitty_erase)?;
        }
        let mut frame_state = app::FrameState::default();
        let (
            dirty,
            image_behind_modal,
            sixel_collision_erase,
            popup_restore,
            modal_erase,
            skip_overlay_present,
        ) = {
            let completed = terminal.draw(|frame| ui::render(frame, app, &mut frame_state))?;
            let dirty = app.set_frame_state(frame_state);
            let modal_rects = app.collect_popup_rects();
            if !app.browser_wheel_burst_active()
                && app.should_repaint_iterm_inline_under_modal(&modal_rects)
            {
                let image_behind_modal = app.present_preview_overlay_behind_modal()?;
                let popup_restore = collect_buffer_cells(&modal_rects, completed.buffer);
                let modal_erase = app.modal_image_post_draw_erase(&modal_rects, completed.buffer);
                (
                    dirty,
                    image_behind_modal,
                    Vec::new(),
                    popup_restore,
                    modal_erase,
                    true,
                )
            } else if !app.browser_wheel_burst_active()
                && app.should_repaint_sixel_under_modal(&modal_rects)
            {
                let image_behind_modal = app.present_preview_overlay_behind_modal()?;
                let (sixel_collision_rects, sixel_collision_erase) =
                    app.sixel_modal_collision_erase(&modal_rects);
                let popup_restore = collect_buffer_cells(&sixel_collision_rects, completed.buffer);
                let modal_erase = app.modal_image_post_draw_erase(&modal_rects, completed.buffer);
                (
                    dirty,
                    image_behind_modal,
                    sixel_collision_erase,
                    popup_restore,
                    modal_erase,
                    true,
                )
            } else {
                let (sixel_collision_rects, sixel_collision_erase) =
                    app.sixel_modal_collision_erase(&modal_rects);
                let popup_restore = collect_buffer_cells(&sixel_collision_rects, completed.buffer);
                let modal_erase = app.modal_image_post_draw_erase(&modal_rects, completed.buffer);
                (
                    dirty,
                    Vec::new(),
                    sixel_collision_erase,
                    popup_restore,
                    modal_erase,
                    false,
                )
            }
        };
        write_bytes_preserving_cursor(terminal.backend_mut(), &image_behind_modal)?;
        write_bytes_preserving_cursor(terminal.backend_mut(), &sixel_collision_erase)?;
        draw_cells_preserving_cursor(terminal.backend_mut(), &popup_restore)?;
        write_bytes_preserving_cursor(terminal.backend_mut(), &modal_erase)?;
        if !skip_overlay_present && !app.browser_wheel_burst_active() {
            let overlay_bytes = app.present_preview_overlay()?;
            write_bytes_preserving_cursor(terminal.backend_mut(), &overlay_bytes)?;
        }
        terminal.backend_mut().flush()?;
        Ok(dirty)
    })();

    let end_result = execute!(terminal.backend_mut(), EndSynchronizedUpdate);
    match (draw_result, end_result) {
        (Ok(dirty), Ok(())) => Ok(dirty),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
        (Err(error), Err(_)) => Err(error),
    }
}

fn write_bytes_preserving_cursor<W: Write>(writer: &mut W, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    execute!(writer, SavePosition)?;
    writer.write_all(bytes)?;
    execute!(writer, RestorePosition)?;
    Ok(())
}

fn draw_cells_preserving_cursor<W: Write>(
    backend: &mut CrosstermBackend<W>,
    cells: &[(u16, u16, Cell)],
) -> io::Result<()> {
    if cells.is_empty() {
        return Ok(());
    }
    execute!(backend, SavePosition)?;
    ratatui::backend::Backend::draw(backend, cells.iter().map(|(x, y, cell)| (*x, *y, cell)))?;
    execute!(backend, RestorePosition)?;
    Ok(())
}

fn collect_buffer_cells(rects: &[Rect], buffer: &Buffer) -> Vec<(u16, u16, Cell)> {
    let bounds = *buffer.area();
    let mut cells = Vec::new();
    for rect in rects {
        let Some(area) = intersect_rect(*rect, bounds) else {
            continue;
        };
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                let Some(cell) = buffer.cell((x, y)) else {
                    continue;
                };
                if matches!(cell.diff_option, CellDiffOption::Skip) {
                    continue;
                }
                cells.push((x, y, cell.clone()));
            }
        }
    }
    cells
}

fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = a.x.saturating_add(a.width).min(b.x.saturating_add(b.width));
    let y2 =
        a.y.saturating_add(a.height)
            .min(b.y.saturating_add(b.height));
    (x2 > x1 && y2 > y1).then_some(Rect {
        x: x1,
        y: y1,
        width: x2.saturating_sub(x1),
        height: y2.saturating_sub(y1),
    })
}

#[cfg(test)]
mod tests {
    use super::collect_buffer_cells;
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::{Color, Modifier, Style},
    };

    #[test]
    fn ratatui_diff_preserves_positions_beyond_u16_max_cells() {
        let area = Rect::new(0, 0, 400, 200);
        let previous = Buffer::empty(area);
        let mut next = Buffer::empty(area);
        next.set_string(123, 180, "X", Style::default());

        let diff = previous.diff(&next);

        assert!(
            diff.iter()
                .any(|(x, y, cell)| *x == 123 && *y == 180 && cell.symbol() == "X"),
            "expected diff to keep the changed cell at (123, 180), got: {:?}",
            diff.iter()
                .map(|(x, y, cell)| (*x, *y, cell.symbol().to_string()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn collect_buffer_cells_captures_popup_cells_with_styles() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 4));
        buffer.set_string(
            2,
            1,
            "OK",
            Style::default()
                .fg(Color::LightGreen)
                .bg(Color::Rgb(1, 2, 3))
                .add_modifier(Modifier::BOLD),
        );

        let cells = collect_buffer_cells(&[Rect::new(2, 1, 2, 1)], &buffer);

        assert_eq!(cells.len(), 2);
        assert_eq!((cells[0].0, cells[0].1, cells[0].2.symbol()), (2, 1, "O"));
        assert_eq!((cells[1].0, cells[1].1, cells[1].2.symbol()), (3, 1, "K"));
        assert_eq!(cells[0].2.fg, Color::LightGreen);
        assert_eq!(cells[0].2.bg, Color::Rgb(1, 2, 3));
        assert!(cells[0].2.modifier.contains(Modifier::BOLD));
    }
}
