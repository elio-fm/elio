use super::tui_event_loop::clear_for_full_repaint;
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
    Terminal,
    backend::CrosstermBackend,
    buffer::{Buffer, Cell, CellDiffOption},
    layout::Rect,
};
use std::{
    io::{self, Write},
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread,
};

/// The TUI terminal, backed by [`ThreadedWriter`] so frame output never blocks
/// the event loop.
pub(super) type AppTerminal = Terminal<CrosstermBackend<ThreadedWriter>>;

/// State shared between the event-loop side of [`ThreadedWriter`] and its
/// background thread. `buf` accumulates flushed frames; the writer thread swaps
/// it out and performs the blocking write outside the lock. `idle` is true while
/// the writer thread is parked with nothing queued and nothing mid-write — the
/// condition drain barriers wait for.
struct WriterShared {
    buf: Vec<u8>,
    idle: bool,
    dead: bool,
    shutdown: bool,
}

struct WriterChannel {
    state: Mutex<WriterShared>,
    cond: Condvar,
}

impl WriterChannel {
    fn lock(&self) -> MutexGuard<'_, WriterShared> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A [`Write`] that hands each flushed frame to a background thread which
/// performs the blocking write to stdout.
///
/// The TUI event loop must never block on terminal output. A single large frame
/// — above all a multi-megabyte Kitty image payload — written synchronously
/// stalls the entire loop (no input, no resize handling) for as long as the
/// terminal/tmux takes to drain it. Through tmux that drain backs up whenever
/// the outer terminal is busy (e.g. repainting after a resize), which froze the
/// UI for seconds. Flushes append to a shared buffer under a mutex — never
/// waiting on the terminal — so the loop stays responsive and the visible output
/// catches up a beat later. The buffer is unbounded by design: frames are only
/// produced on events/timers, so even a stalled terminal accumulates output far
/// slower than memory matters, and bounding it would reintroduce the very
/// backpressure stall this exists to remove.
pub(super) struct ThreadedWriter {
    channel: Arc<WriterChannel>,
    pending: Vec<u8>,
}

impl ThreadedWriter {
    pub(super) fn new(output: Box<dyn Write + Send>) -> Self {
        let channel = Arc::new(WriterChannel {
            state: Mutex::new(WriterShared {
                buf: Vec::new(),
                idle: true,
                dead: false,
                shutdown: false,
            }),
            cond: Condvar::new(),
        });
        let writer_channel = Arc::clone(&channel);
        thread::spawn(move || {
            let mut out = output;
            let mut batch = Vec::new();
            loop {
                {
                    let mut state = writer_channel.lock();
                    while state.buf.is_empty() && !state.shutdown {
                        state.idle = true;
                        writer_channel.cond.notify_all();
                        state = writer_channel
                            .cond
                            .wait(state)
                            .unwrap_or_else(PoisonError::into_inner);
                    }
                    if state.buf.is_empty() {
                        state.idle = true;
                        writer_channel.cond.notify_all();
                        return;
                    }
                    state.idle = false;
                    std::mem::swap(&mut state.buf, &mut batch);
                }
                if out.write_all(&batch).is_err() || out.flush().is_err() {
                    let mut state = writer_channel.lock();
                    state.dead = true;
                    state.idle = true;
                    state.buf.clear();
                    writer_channel.cond.notify_all();
                    return;
                }
                batch.clear();
            }
        });
        Self {
            channel,
            pending: Vec::new(),
        }
    }

    pub(super) fn drainer(&self) -> Drainer {
        Drainer {
            channel: Arc::clone(&self.channel),
        }
    }

    fn drain(&mut self) {
        let _ = self.flush();
        self.drainer().drain();
    }
}

/// Blocks until the [`ThreadedWriter`] has flushed everything queued before it.
pub(super) struct Drainer {
    channel: Arc<WriterChannel>,
}

impl Drainer {
    pub(super) fn drain(&self) {
        let mut state = self.channel.lock();
        while !(state.dead || state.buf.is_empty() && state.idle) {
            state = self
                .channel
                .cond
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

impl Write for ThreadedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut state = self.channel.lock();
        if state.dead {
            self.pending.clear();
            return Ok(());
        }
        state.buf.append(&mut self.pending);
        self.channel.cond.notify_all();
        Ok(())
    }
}

impl Drop for ThreadedWriter {
    fn drop(&mut self) {
        self.drain();
        let mut state = self.channel.lock();
        state.shutdown = true;
        self.channel.cond.notify_all();
    }
}

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
#[path = "tests/tui_drawing.rs"]
mod tests;
