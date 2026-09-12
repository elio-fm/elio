use super::FileOperationsState;
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

pub(crate) struct PastePreparation {
    pub(crate) request: Option<PasteRequest>,
    pub(crate) status: Option<String>,
}

pub(crate) struct DropPreparation {
    pub(crate) accepted: bool,
    pub(crate) request: Option<PasteRequest>,
    pub(crate) status: String,
}

impl FileOperationsState {
    /// Yank (copy-mark) the current selection or the focused entry.
    pub(crate) fn set_clipboard(&mut self, paths: Vec<PathBuf>, op: ClipOp) -> bool {
        if paths.is_empty() {
            return false;
        }
        self.clipboard = Some(Clipboard { paths, op });
        true
    }

    /// Paste the clipboard contents into the current directory (async with
    /// progress reporting).
    pub(crate) fn prepare_paste(&mut self, cwd: &Path) -> PastePreparation {
        if self.paste_progress.is_some() && self.clipboard.is_none() {
            return PastePreparation {
                request: None,
                status: Some(
                    "Paste in progress — yank or cut another item to queue it".to_string(),
                ),
            };
        }

        let Some(request) = self.take_clipboard_paste(cwd) else {
            return PastePreparation {
                request: None,
                status: Some("Nothing to paste".to_string()),
            };
        };

        if paste_would_copy_directory_into_itself(&request) {
            self.clipboard = Some(Clipboard {
                paths: request.paths,
                op: request.op,
            });
            return PastePreparation {
                request: None,
                status: Some("Cannot paste a folder into itself".to_string()),
            };
        }

        if self.paste_progress.is_some() {
            self.queued_pastes.push_back(request);
            let pending = self.queued_pastes.len();
            let status = if pending == 1 {
                "Queued paste (1 pending)".to_string()
            } else {
                format!("Queued paste ({pending} pending)")
            };
            return PastePreparation {
                request: None,
                status: Some(status),
            };
        }

        PastePreparation {
            request: Some(self.start_paste_request(request)),
            status: None,
        }
    }

    #[cfg(any(unix, test))]
    pub(crate) fn prepare_drop(
        &mut self,
        cwd: &Path,
        paths: Vec<PathBuf>,
        op: ClipOp,
    ) -> DropPreparation {
        let paths: Vec<PathBuf> = paths.into_iter().filter(|path| path.exists()).collect();
        if paths.is_empty() {
            return DropPreparation {
                accepted: false,
                request: None,
                status: "Drop contains no local files".to_string(),
            };
        }
        if op == ClipOp::Cut
            && paths.iter().any(|path| {
                path.file_name()
                    .map(|file_name| cwd.join(file_name) == *path)
                    .unwrap_or(false)
            })
        {
            return DropPreparation {
                accepted: false,
                request: None,
                status: "Already here".to_string(),
            };
        }

        let request = QueuedPaste {
            dest_dir: cwd.to_path_buf(),
            paths,
            op,
            origin: PasteOrigin::Drop,
        };
        if paste_would_copy_directory_into_itself(&request) {
            return DropPreparation {
                accepted: false,
                request: None,
                status: "Cannot drop a folder into itself".to_string(),
            };
        }
        if self.paste_progress.is_some() {
            self.queued_pastes.push_back(request);
            let pending = self.queued_pastes.len();
            let status = if pending == 1 {
                "Queued drop (1 pending)".to_string()
            } else {
                format!("Queued drop ({pending} pending)")
            };
            DropPreparation {
                accepted: true,
                request: None,
                status,
            }
        } else {
            DropPreparation {
                accepted: true,
                request: Some(self.start_paste_request(request)),
                status: "Dropping files…".to_string(),
            }
        }
    }

    pub(crate) fn start_next_queued_paste(&mut self) -> Option<PasteRequest> {
        let request = self.queued_pastes.pop_front()?;
        Some(self.start_paste_request(request))
    }

    fn take_clipboard_paste(&mut self, cwd: &Path) -> Option<QueuedPaste> {
        let clipboard = self.clipboard.take()?;
        if clipboard.paths.is_empty() {
            return None;
        };
        Some(QueuedPaste {
            dest_dir: cwd.to_path_buf(),
            paths: clipboard.paths,
            op: clipboard.op,
            origin: PasteOrigin::Clipboard,
        })
    }

    fn start_paste_request(&mut self, request: QueuedPaste) -> PasteRequest {
        let token = self.paste_token.wrapping_add(1);
        self.paste_token = token;
        self.paste_progress = Some(PasteProgress {
            completed: 0,
            total: request.paths.len(),
            op: request.op,
            origin: request.origin.clone(),
        });
        self.paste_dest_dir = Some(request.dest_dir.clone());
        PasteRequest {
            token,
            dest_dir: request.dest_dir,
            paths: request.paths,
            op: request.op,
        }
    }
}

fn paste_would_copy_directory_into_itself(request: &QueuedPaste) -> bool {
    request
        .paths
        .iter()
        .any(|path| path.is_dir() && request.dest_dir.starts_with(path))
}

impl FileOperationsState {
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

    pub fn queued_paste_count(&self) -> usize {
        self.queued_pastes.len()
    }

    /// Returns the clipboard operation for a specific path, if it is in the
    /// clipboard.
    pub fn clipboard_op_for(&self, path: &Path) -> Option<ClipOp> {
        self.clipboard
            .as_ref()
            .filter(|c| c.paths.iter().any(|p| p == path))
            .map(|c| c.op)
    }

    pub(crate) fn clear_queued_pastes(&mut self) -> usize {
        let queued = self.queued_pastes.len();
        self.queued_pastes.clear();
        queued
    }
}
