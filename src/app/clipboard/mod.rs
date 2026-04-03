mod copy;

use super::{
    App,
    jobs::PasteRequest,
    state::{Clipboard, PasteProgress, QueuedPaste},
    types::ClipOp,
};
use anyhow::Result;
use std::path::{Path, PathBuf};

impl App {
    /// Returns `(count, op)` for the current clipboard, or `None` if empty.
    pub fn clipboard_info(&self) -> Option<(usize, ClipOp)> {
        self.jobs.clipboard.as_ref().map(|c| (c.paths.len(), c.op))
    }

    /// Returns `(completed, total, op)` for an in-progress paste, or `None`.
    pub fn paste_progress(&self) -> Option<(usize, usize, ClipOp)> {
        self.jobs
            .paste_progress
            .as_ref()
            .map(|p| (p.completed, p.total, p.op))
    }

    pub fn queued_paste_count(&self) -> usize {
        self.jobs.queued_pastes.len()
    }

    /// Returns the clipboard operation for a specific path, if it is in the
    /// clipboard.
    pub fn clipboard_op_for(&self, path: &Path) -> Option<ClipOp> {
        self.jobs
            .clipboard
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
        self.jobs.clipboard = Some(Clipboard {
            paths,
            op: ClipOp::Yank,
        });
        self.navigation.selected_paths.clear();
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
        self.jobs.clipboard = Some(Clipboard {
            paths,
            op: ClipOp::Cut,
        });
        self.navigation.selected_paths.clear();
        self.status = if count == 1 {
            "Cut 1 item".to_string()
        } else {
            format!("Cut {count} items")
        };
    }

    /// Paste the clipboard contents into the current directory (async with
    /// progress reporting).
    pub(in crate::app) fn paste(&mut self) -> Result<()> {
        if self.jobs.paste_progress.is_some() && self.jobs.clipboard.is_none() {
            self.status = "Paste in progress — yank or cut another item to queue it".to_string();
            return Ok(());
        }

        let Some(request) = self.take_clipboard_paste() else {
            self.status = "Nothing to paste".to_string();
            return Ok(());
        };

        if self.jobs.paste_progress.is_some() {
            self.jobs.queued_pastes.push_back(request);
            let pending = self.jobs.queued_pastes.len();
            self.status = if pending == 1 {
                "Queued paste (1 pending)".to_string()
            } else {
                format!("Queued paste ({pending} pending)")
            };
            return Ok(());
        }

        self.start_paste_request(request);

        Ok(())
    }

    pub(super) fn clear_queued_pastes(&mut self) -> usize {
        let queued = self.jobs.queued_pastes.len();
        self.jobs.queued_pastes.clear();
        queued
    }

    pub(super) fn start_next_queued_paste(&mut self) -> bool {
        let Some(request) = self.jobs.queued_pastes.pop_front() else {
            return false;
        };
        self.start_paste_request(request);
        true
    }

    fn take_clipboard_paste(&mut self) -> Option<QueuedPaste> {
        let clipboard = self.jobs.clipboard.take()?;
        if clipboard.paths.is_empty() {
            return None;
        }
        Some(QueuedPaste {
            dest_dir: self.navigation.cwd.clone(),
            paths: clipboard.paths,
            op: clipboard.op,
        })
    }

    fn start_paste_request(&mut self, request: QueuedPaste) {
        let token = self.jobs.paste_token.wrapping_add(1);
        self.jobs.paste_token = token;
        self.jobs.paste_progress = Some(PasteProgress {
            completed: 0,
            total: request.paths.len(),
            op: request.op,
        });
        self.jobs.paste_dest_dir = Some(request.dest_dir.clone());

        self.jobs.scheduler.submit_paste(PasteRequest {
            token,
            dest_dir: request.dest_dir,
            paths: request.paths,
            op: request.op,
        });
    }

    /// Collect the paths that y/x should act on: all space-selected paths if
    /// any exist (sorted for stable ordering), otherwise the focused entry.
    pub(super) fn clipboard_target_paths(&self) -> Vec<PathBuf> {
        if !self.navigation.selected_paths.is_empty() {
            let mut paths: Vec<PathBuf> = self.navigation.selected_paths.iter().cloned().collect();
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
