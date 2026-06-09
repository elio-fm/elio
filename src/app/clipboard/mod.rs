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

        if paste_would_copy_directory_into_itself(&request) {
            self.jobs.clipboard = Some(Clipboard {
                paths: request.paths,
                op: request.op,
            });
            self.status = "Cannot paste a folder into itself".to_string();
            return Ok(());
        }

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

    pub(in crate::app) fn link_yanked(&mut self, relative: bool) -> Result<()> {
        let Some(clipboard) = &self.jobs.clipboard else {
            self.status = "Nothing to link".to_string();
            return Ok(());
        };
        if clipboard.op != ClipOp::Yank {
            self.status = "Yank items before linking".to_string();
            return Ok(());
        }

        let paths = clipboard.paths.clone();
        if paths.is_empty() {
            self.status = "Nothing to link".to_string();
            return Ok(());
        }

        #[cfg(unix)]
        {
            let mut created = Vec::new();
            let mut first_error = None;
            for source in paths {
                let link_path = unique_link_dest(&self.navigation.cwd, &source);
                let target = if relative {
                    relative_path(&self.navigation.cwd, &source)
                } else {
                    source.clone()
                };
                match std::os::unix::fs::symlink(&target, &link_path) {
                    Ok(()) => created.push(link_path),
                    Err(error) => {
                        let name = source
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("item");
                        first_error = Some(format!("Could not link \"{name}\": {error}"));
                        break;
                    }
                }
            }

            if !created.is_empty() {
                let _ = self.queue_directory_reload(false);
            }
            self.status = match (created.len(), first_error) {
                (0, Some(error)) => error,
                (1, None) => format!(
                    "Created symlink \"{}\"",
                    created[0]
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("item")
                ),
                (n, None) => format!("Created {n} symlinks"),
                (n, Some(error)) => format!("Created {n} symlinks; last error: {error}"),
            };
        }

        #[cfg(not(unix))]
        {
            let _ = relative;
            self.status = "Symlinks are not supported on this platform".to_string();
        }

        Ok(())
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

fn paste_would_copy_directory_into_itself(request: &QueuedPaste) -> bool {
    request
        .paths
        .iter()
        .any(|path| path.is_dir() && request.dest_dir.starts_with(path))
}

#[cfg(unix)]
fn unique_link_dest(dest_dir: &Path, source: &Path) -> PathBuf {
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "link".to_string());
    let candidate = dest_dir.join(&name);
    if std::fs::symlink_metadata(&candidate).is_err() {
        return candidate;
    }

    let source_path = Path::new(&name);
    let stem = source_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.clone());
    let ext = source_path
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned());
    for index in 1u32.. {
        let next_name = match &ext {
            Some(ext) => format!("{stem}_{index}.{ext}"),
            None => format!("{stem}_{index}"),
        };
        let next = dest_dir.join(next_name);
        if std::fs::symlink_metadata(&next).is_err() {
            return next;
        }
    }
    candidate
}

#[cfg(unix)]
fn relative_path(from_dir: &Path, to: &Path) -> PathBuf {
    let from = from_dir.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(to.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for component in &to[common..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    }
}

#[cfg(test)]
mod tests;
