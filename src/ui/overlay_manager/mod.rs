use crate::app::{App, FrameState};
use crate::ui::theme::Palette;
use ratatui::{Frame, layout::Rect};

mod bulk_rename;
mod copy;
mod create;
mod help;
mod rename;
mod restore;
mod search;
mod trash;

pub(super) fn render_trash_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    trash::render_trash_overlay(frame, area, app, state, palette);
}

pub(super) fn render_restore_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    restore::render_restore_overlay(frame, area, app, state, palette);
}

pub(super) fn render_create_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    create::render_create_overlay(frame, area, app, state, palette);
}

pub(super) fn render_rename_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    rename::render_rename_overlay(frame, area, app, state, palette);
}

pub(super) fn render_bulk_rename_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    bulk_rename::render_bulk_rename_overlay(frame, area, app, state, palette);
}

pub(super) fn render_copy_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    copy::render_copy_overlay(frame, area, app, state, palette);
}

pub(super) fn render_search_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    search::render_search_overlay(frame, area, app, state, palette);
}

pub(super) fn render_help(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut FrameState,
    palette: Palette,
) {
    help::render_help(frame, area, state, palette);
}

fn compute_scroll_top(cursor_line: usize, visible: usize) -> usize {
    if cursor_line < visible {
        0
    } else {
        cursor_line - visible + 1
    }
}
