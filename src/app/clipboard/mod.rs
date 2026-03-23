use super::{
    App,
    state::{Clipboard, DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad},
    types::ClipOp,
};
use anyhow::Result;
use std::{
    fs,
    path::{Path, PathBuf},
};

impl App {
    /// Returns `(count, op)` for the current clipboard, or `None` if empty.
    pub fn clipboard_info(&self) -> Option<(usize, ClipOp)> {
        self.clipboard.as_ref().map(|c| (c.paths.len(), c.op))
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

    /// Paste the clipboard contents into the current directory.
    pub(in crate::app) fn paste(&mut self) -> Result<()> {
        let Some(clipboard) = self.clipboard.take() else {
            self.status = "Nothing to paste".to_string();
            return Ok(());
        };

        let op = clipboard.op;
        let dest_dir = self.cwd.clone();
        let mut pasted: usize = 0;
        let mut errors: Vec<String> = Vec::new();

        for src in &clipboard.paths {
            let Some(file_name) = src.file_name().and_then(|n| n.to_str()) else {
                errors.push(format!("Cannot determine name for {}", src.display()));
                continue;
            };

            if !src.exists() {
                errors.push(format!("\"{}\" no longer exists", file_name));
                continue;
            }

            // For cut: if source is already in this directory under the same
            // name it would get, the operation is a no-op.
            if op == ClipOp::Cut {
                let natural = dest_dir.join(file_name);
                if natural == *src {
                    pasted += 1;
                    continue;
                }
            }

            let dest = unique_dest(&dest_dir, file_name);

            match op {
                ClipOp::Yank => {
                    if let Err(e) = copy_recursive(src, &dest) {
                        errors.push(format!("\"{}\" could not be copied: {e}", file_name));
                    } else {
                        pasted += 1;
                    }
                }
                ClipOp::Cut => {
                    match fs::rename(src, &dest) {
                        Ok(()) => pasted += 1,
                        // Cross-device move: copy then delete source.
                        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                            match copy_recursive(src, &dest) {
                                Ok(()) => {
                                    let del = if src.is_dir() {
                                        fs::remove_dir_all(src)
                                    } else {
                                        fs::remove_file(src)
                                    };
                                    if let Err(de) = del {
                                        errors.push(format!(
                                            "\"{}\" was copied but source could not be removed: {de}",
                                            file_name
                                        ));
                                    }
                                    pasted += 1;
                                }
                                Err(ce) => {
                                    // Clean up the partial destination copy.
                                    let _ = if dest.is_dir() {
                                        fs::remove_dir_all(&dest)
                                    } else {
                                        fs::remove_file(&dest)
                                    };
                                    errors.push(format!(
                                        "\"{}\" could not be moved: {ce}",
                                        file_name
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            errors.push(format!("\"{}\" could not be moved: {e}", file_name));
                        }
                    }
                }
            }
        }

        let verb = match op {
            ClipOp::Yank => "Copied",
            ClipOp::Cut => "Moved",
        };

        let status = if errors.is_empty() {
            match pasted {
                0 => "Nothing was pasted".to_string(),
                1 => format!("{verb} 1 item"),
                n => format!("{verb} {n} items"),
            }
        } else if pasted == 0 {
            if errors.len() == 1 {
                errors.remove(0)
            } else {
                format!("{} errors — first: {}", errors.len(), errors[0])
            }
        } else {
            format!(
                "{verb} {pasted} item(s); {} error(s) — first: {}",
                errors.len(),
                errors[0]
            )
        };

        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: self.cwd.clone(),
            previous_cwd: self.cwd.clone(),
            previous_selected_path: None,
            previous_selection_name: None,
            reselect_path: None,
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(status),
        })?;

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

/// Return a destination path inside `dir` for an item named `name` that does
/// not collide with any existing file.  If `dir/name` is free it is returned
/// as-is; otherwise `dir/stem (N).ext` is tried for N = 1, 2, …
fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let base = Path::new(name);
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = base.extension().and_then(|s| s.to_str());
    for i in 1u32.. {
        let candidate = match ext {
            Some(e) => format!("{stem} ({i}).{e}"),
            None => format!("{stem} ({i})"),
        };
        let path = dir.join(&candidate);
        if !path.exists() {
            return path;
        }
    }
    first // unreachable in practice
}

/// Recursively copy `src` to `dest`.  If `src` is a directory the entire
/// subtree is duplicated; if it is a file a single `fs::copy` is performed.
fn copy_recursive(src: &Path, dest: &Path) -> anyhow::Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dest)
            .map_err(|e| anyhow::anyhow!("Cannot create directory \"{}\": {e}", dest.display()))?;
        for entry_result in fs::read_dir(src)
            .map_err(|e| anyhow::anyhow!("Cannot read \"{}\": {e}", src.display()))?
        {
            let child = entry_result
                .map_err(|e| anyhow::anyhow!("Cannot read entry in \"{}\": {e}", src.display()))?;
            copy_recursive(&child.path(), &dest.join(child.file_name()))?;
        }
    } else {
        fs::copy(src, dest).map_err(|e| {
            anyhow::anyhow!(
                "Cannot copy \"{}\" to \"{}\": {e}",
                src.display(),
                dest.display()
            )
        })?;
    }
    Ok(())
}
