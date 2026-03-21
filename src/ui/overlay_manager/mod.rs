use super::helpers;
use super::theme::{self, Palette};
use crate::app::{App, FrameState, SearchHit, SearchScope};

mod bulk_rename;
mod create;
mod help;
mod rename;
mod restore;
mod search;
mod trash;

pub(super) use bulk_rename::render_bulk_rename_overlay;
pub(super) use create::render_create_overlay;
pub(super) use help::render_help;
pub(super) use rename::render_rename_overlay;
pub(super) use restore::render_restore_overlay;
pub(super) use search::render_search_overlay;
pub(super) use trash::render_trash_overlay;

fn compute_create_scroll_top(cursor_line: usize, line_count: usize, visible: usize) -> usize {
    let _ = line_count;
    if cursor_line < visible {
        0
    } else {
        cursor_line - visible + 1
    }
}
