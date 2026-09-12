use crate::app::App;
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClipOp {
    Yank,
    Cut,
}

#[derive(Clone, Debug)]
pub(crate) struct Clipboard {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) op: ClipOp,
}

#[derive(Clone, Debug)]
pub(crate) struct PasteProgress {
    pub(crate) completed: usize,
    pub(crate) total: usize,
    pub(crate) op: ClipOp,
    pub(crate) origin: PasteOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PasteOrigin {
    Clipboard,
    Drop,
}

#[derive(Clone, Debug)]
pub(crate) struct QueuedPaste {
    pub(crate) dest_dir: PathBuf,
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) op: ClipOp,
    pub(crate) origin: PasteOrigin,
}

#[derive(Clone, Debug)]
pub(crate) struct PasteRequest {
    pub(crate) token: u64,
    pub(crate) dest_dir: PathBuf,
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) op: ClipOp,
}

impl App {
    /// Returns `(count, op)` for the current clipboard, or `None` if empty.
    pub fn clipboard_info(&self) -> Option<(usize, ClipOp)> {
        self.file_operations
            .clipboard
            .as_ref()
            .map(|c| (c.paths.len(), c.op))
    }

    /// Returns `(completed, total, op)` for an in-progress paste, or `None`.
    pub fn paste_progress(&self) -> Option<(usize, usize, ClipOp)> {
        self.file_operations
            .paste_progress
            .as_ref()
            .map(|p| (p.completed, p.total, p.op))
    }

    pub fn queued_paste_count(&self) -> usize {
        self.file_operations.queued_pastes.len()
    }

    /// Returns the clipboard operation for a specific path, if it is in the
    /// clipboard.
    pub fn clipboard_op_for(&self, path: &Path) -> Option<ClipOp> {
        self.file_operations
            .clipboard
            .as_ref()
            .filter(|c| c.paths.iter().any(|p| p == path))
            .map(|c| c.op)
    }

    /// Yank (copy-mark) the current selection or the focused entry.
    pub(crate) fn yank(&mut self) {
        let paths = self.clipboard_target_paths();
        if paths.is_empty() {
            return;
        }
        self.file_operations.clipboard = Some(Clipboard {
            paths,
            op: ClipOp::Yank,
        });
        self.file_browser.selected_paths.clear();
        self.status.clear();
    }

    /// Cut-mark the current selection or the focused entry.
    pub(crate) fn cut(&mut self) {
        let paths = self.clipboard_target_paths();
        if paths.is_empty() {
            return;
        }
        self.file_operations.clipboard = Some(Clipboard {
            paths,
            op: ClipOp::Cut,
        });
        self.file_browser.selected_paths.clear();
        self.status.clear();
    }

    /// Paste the clipboard contents into the current directory (async with
    /// progress reporting).
    pub(crate) fn paste(&mut self) -> Result<()> {
        if self.file_operations.paste_progress.is_some() && self.file_operations.clipboard.is_none()
        {
            self.status = "Paste in progress — yank or cut another item to queue it".to_string();
            return Ok(());
        }

        let Some(request) = self.take_clipboard_paste() else {
            self.status = "Nothing to paste".to_string();
            return Ok(());
        };

        if paste_would_copy_directory_into_itself(&request) {
            self.file_operations.clipboard = Some(Clipboard {
                paths: request.paths,
                op: request.op,
            });
            self.status = "Cannot paste a folder into itself".to_string();
            return Ok(());
        }

        if self.file_operations.paste_progress.is_some() {
            self.file_operations.queued_pastes.push_back(request);
            let pending = self.file_operations.queued_pastes.len();
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

    #[cfg(any(unix, test))]
    pub(crate) fn drop_external_paths(&mut self, paths: Vec<PathBuf>, op: ClipOp) -> Result<bool> {
        let paths: Vec<PathBuf> = paths.into_iter().filter(|path| path.exists()).collect();
        if paths.is_empty() {
            self.status = "Drop contains no local files".to_string();
            return Ok(false);
        }
        if op == ClipOp::Cut
            && paths.iter().any(|path| {
                path.file_name()
                    .map(|file_name| self.file_browser.cwd.join(file_name) == *path)
                    .unwrap_or(false)
            })
        {
            self.status = "Already here".to_string();
            return Ok(false);
        }

        let request = QueuedPaste {
            dest_dir: self.file_browser.cwd.clone(),
            paths,
            op,
            origin: PasteOrigin::Drop,
        };
        if paste_would_copy_directory_into_itself(&request) {
            self.status = "Cannot drop a folder into itself".to_string();
            return Ok(false);
        }
        if self.file_operations.paste_progress.is_some() {
            self.file_operations.queued_pastes.push_back(request);
            let pending = self.file_operations.queued_pastes.len();
            self.status = if pending == 1 {
                "Queued drop (1 pending)".to_string()
            } else {
                format!("Queued drop ({pending} pending)")
            };
        } else {
            self.start_paste_request(request);
            self.status = "Dropping files…".to_string();
        }
        Ok(true)
    }

    pub(crate) fn clear_queued_pastes(&mut self) -> usize {
        let queued = self.file_operations.queued_pastes.len();
        self.file_operations.queued_pastes.clear();
        queued
    }

    pub(crate) fn start_next_queued_paste(&mut self) -> bool {
        let Some(request) = self.file_operations.queued_pastes.pop_front() else {
            return false;
        };
        self.start_paste_request(request);
        true
    }

    fn take_clipboard_paste(&mut self) -> Option<QueuedPaste> {
        let clipboard = self.file_operations.clipboard.take()?;
        if clipboard.paths.is_empty() {
            return None;
        }
        Some(QueuedPaste {
            dest_dir: self.file_browser.cwd.clone(),
            paths: clipboard.paths,
            op: clipboard.op,
            origin: PasteOrigin::Clipboard,
        })
    }

    fn start_paste_request(&mut self, request: QueuedPaste) {
        let token = self.file_operations.paste_token.wrapping_add(1);
        self.file_operations.paste_token = token;
        self.file_operations.paste_progress = Some(PasteProgress {
            completed: 0,
            total: request.paths.len(),
            op: request.op,
            origin: request.origin.clone(),
        });
        self.file_operations.paste_dest_dir = Some(request.dest_dir.clone());

        self.job_scheduler.submit_paste(PasteRequest {
            token,
            dest_dir: request.dest_dir,
            paths: request.paths,
            op: request.op,
        });
    }

    /// Collect the paths that y/x should act on: all space-selected paths if
    /// any exist (sorted for stable ordering), otherwise the focused entry.
    pub(super) fn clipboard_target_paths(&self) -> Vec<PathBuf> {
        if !self.file_browser.selected_paths.is_empty() {
            let mut paths: Vec<PathBuf> =
                self.file_browser.selected_paths.iter().cloned().collect();
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

fn paste_would_copy_directory_into_itself(request: &QueuedPaste) -> bool {
    request
        .paths
        .iter()
        .any(|path| path.is_dir() && request.dest_dir.starts_with(path))
}
