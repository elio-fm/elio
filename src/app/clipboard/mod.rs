use super::{
    App,
    jobs::PasteRequest,
    state::{Clipboard, PasteProgress},
    types::ClipOp,
};
use anyhow::Result;
use std::path::{Path, PathBuf};

impl App {
    /// Returns `(count, op)` for the current clipboard, or `None` if empty.
    pub fn clipboard_info(&self) -> Option<(usize, ClipOp)> {
        self.clipboard.as_ref().map(|c| (c.paths.len(), c.op))
    }

    /// Returns `(completed, total, op)` for an in-progress paste, or `None`.
    pub fn paste_progress(&self) -> Option<(usize, usize, ClipOp)> {
        self.paste_progress
            .as_ref()
            .map(|p| (p.completed, p.total, p.op))
    }

    /// Returns the clipboard operation for a specific path, if it is in the
    /// clipboard.
    pub fn clipboard_op_for(&self, path: &Path) -> Option<ClipOp> {
        self.clipboard
            .as_ref()
            .filter(|c| c.paths.iter().any(|p| p == path))
            .map(|c| c.op)
    }

    /// Yank (copy-mark) the current selection or the focused entry.
    pub(in crate::app) fn yank(&mut self) {
        let paths = self.clipboard_target_paths();
        if paths.is_empty() {
            return;
        }
        let count = paths.len();
        self.clipboard = Some(Clipboard {
            paths,
            op: ClipOp::Yank,
        });
        self.selected_paths.clear();
        self.status = if count == 1 {
            "Yanked 1 item".to_string()
        } else {
            format!("Yanked {count} items")
        };
    }

    /// Cut-mark the current selection or the focused entry.
    pub(in crate::app) fn cut(&mut self) {
        let paths = self.clipboard_target_paths();
        if paths.is_empty() {
            return;
        }
        let count = paths.len();
        self.clipboard = Some(Clipboard {
            paths,
            op: ClipOp::Cut,
        });
        self.selected_paths.clear();
        self.status = if count == 1 {
            "Cut 1 item".to_string()
        } else {
            format!("Cut {count} items")
        };
    }

    /// Paste the clipboard contents into the current directory (async with
    /// progress reporting).
    pub(in crate::app) fn paste(&mut self) -> Result<()> {
        if self.paste_progress.is_some() {
            self.status = "Paste in progress — press Esc to cancel".to_string();
            return Ok(());
        }

        let Some(clipboard) = self.clipboard.take() else {
            self.status = "Nothing to paste".to_string();
            return Ok(());
        };

        if clipboard.paths.is_empty() {
            return Ok(());
        }

        let token = self.paste_token.wrapping_add(1);
        self.paste_token = token;
        self.paste_progress = Some(PasteProgress {
            completed: 0,
            total: clipboard.paths.len(),
            op: clipboard.op,
        });

        self.scheduler.submit_paste(PasteRequest {
            token,
            dest_dir: self.cwd.clone(),
            paths: clipboard.paths,
            op: clipboard.op,
        });

        Ok(())
    }

    /// Collect the paths that y/x should act on: all space-selected paths if
    /// any exist (sorted for stable ordering), otherwise the focused entry.
    fn clipboard_target_paths(&self) -> Vec<PathBuf> {
        if !self.selected_paths.is_empty() {
            let mut paths: Vec<PathBuf> = self.selected_paths.iter().cloned().collect();
            paths.sort();
            paths
        } else {
            match self.selected_entry() {
                Some(entry) => vec![entry.path.clone()],
                None => Vec::new(),
            }
        }
    }
}

#[cfg(test)]
mod tests;
