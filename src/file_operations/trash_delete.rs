use super::FileOperationsState;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct TrashProgress {
    pub(crate) completed: usize,
    pub(crate) total: usize,
    pub(crate) permanent: bool,
    pub(crate) duplicate_targets: Option<Vec<PathBuf>>,
    pub(crate) next_selection: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub(crate) struct TrashTarget {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct TrashRequest {
    pub(crate) token: u64,
    pub(crate) targets: Vec<TrashTarget>,
    pub(crate) permanent: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct TrashOverlay {
    pub(crate) targets: Vec<TrashTarget>,
    pub(crate) scroll: usize,
    pub(crate) confirmed: bool,
    pub(crate) permanent: bool,
}

pub(crate) enum TrashConfirmation {
    None,
    InProgress { permanent: bool },
    Targets(TrashOverlay),
}

pub(crate) struct TrashJobCompletion {
    pub(crate) duplicate_targets: Option<Vec<PathBuf>>,
    pub(crate) next_selection: Option<PathBuf>,
    pub(crate) source_cwd: Option<PathBuf>,
}

pub(crate) fn trash_target_from_path(path: PathBuf) -> TrashTarget {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string());
    let is_dir = path.is_dir();
    TrashTarget { path, name, is_dir }
}

pub(crate) enum TrashTargetScope {
    Normal,
    Trash,
    Mixed,
}

pub(crate) fn trash_target_scope(
    targets: &[TrashTarget],
    cwd: &Path,
    cwd_is_trash: bool,
) -> TrashTargetScope {
    let has_trash = targets
        .iter()
        .any(|target| trash_target_is_inside_trash(&target.path, cwd, cwd_is_trash));
    let has_normal = targets
        .iter()
        .any(|target| !trash_target_is_inside_trash(&target.path, cwd, cwd_is_trash));
    match (has_trash, has_normal) {
        (true, true) => TrashTargetScope::Mixed,
        (true, false) => TrashTargetScope::Trash,
        _ => TrashTargetScope::Normal,
    }
}

pub(crate) fn trash_target_is_inside_trash(path: &Path, cwd: &Path, cwd_is_trash: bool) -> bool {
    crate::places::path_is_inside_trash(path) || (cwd_is_trash && path.starts_with(cwd))
}

/// Returns `true` when the first trash target appears to be on a different
/// device than `dirs::data_dir()` (i.e. `~/.local/share` on Linux), which is
/// where the home trash typically lives.
///
/// This is a best-effort heuristic.  The freedesktop trash spec may use a
/// per-mount `.Trash-UID` directory instead of the home trash, so false
/// positives are possible — in that case the user sees "Copying to trash…"
/// briefly before the fast rename completes.  The heuristic is UI-only and
/// never affects behaviour.
#[cfg(unix)]
pub(crate) fn likely_cross_device_trash(targets: &[TrashTarget]) -> bool {
    use std::os::unix::fs::MetadataExt;
    let source_dev = targets
        .first()
        .and_then(|t| std::fs::metadata(&t.path).ok())
        .map(|m| m.dev());
    let data_dev = dirs::data_dir()
        .and_then(|d| std::fs::metadata(&d).ok())
        .map(|m| m.dev());
    match (source_dev, data_dev) {
        (Some(s), Some(d)) => s != d,
        _ => false,
    }
}

impl FileOperationsState {
    pub(crate) fn trash_overlay_mut(&mut self) -> Option<&mut TrashOverlay> {
        self.trash.as_mut()
    }

    pub(crate) fn dismiss_trash(&mut self) {
        self.trash = None;
    }

    pub(crate) fn open_trash_prompt(&mut self, targets: Vec<TrashTarget>, permanent: bool) {
        self.create = None;
        self.trash = Some(TrashOverlay {
            targets,
            scroll: 0,
            confirmed: true,
            permanent,
        });
    }

    pub(crate) fn trash_target_label_at(&self, cwd: &Path, index: usize) -> Option<String> {
        let target = self
            .trash
            .as_ref()
            .and_then(|trash| trash.targets.get(index))?;
        if self.trash.as_ref().is_some_and(|trash| {
            trash
                .targets
                .iter()
                .any(|target| target.path.parent() != Some(cwd))
        }) {
            Some(target.path.display().to_string())
        } else {
            Some(target.name.clone())
        }
    }

    pub(crate) fn take_trash_confirmation(&mut self) -> TrashConfirmation {
        if let Some(progress) = &self.trash_progress {
            let permanent = progress.permanent;
            self.trash = None;
            return TrashConfirmation::InProgress { permanent };
        }
        match self.trash.take() {
            Some(overlay) if !overlay.targets.is_empty() => TrashConfirmation::Targets(overlay),
            _ => TrashConfirmation::None,
        }
    }

    pub(crate) fn start_trash(
        &mut self,
        overlay: TrashOverlay,
        source_cwd: PathBuf,
        next_selection: Option<PathBuf>,
        duplicate_session_open: bool,
    ) -> TrashRequest {
        let duplicate_targets = duplicate_session_open.then(|| {
            overlay
                .targets
                .iter()
                .map(|target| target.path.clone())
                .collect()
        });
        let token = self.trash_token.wrapping_add(1);
        self.trash_token = token;
        self.trash_progress = Some(TrashProgress {
            completed: 0,
            total: overlay.targets.len(),
            permanent: overlay.permanent,
            duplicate_targets,
            next_selection,
        });
        self.trash_source_cwd = Some(source_cwd);
        TrashRequest {
            token,
            targets: overlay.targets,
            permanent: overlay.permanent,
        }
    }

    pub(crate) fn trash_job_is_current(&self, token: u64) -> bool {
        token == self.trash_token
    }

    pub(crate) fn update_trash_progress(&mut self, completed: usize) {
        if let Some(progress) = &mut self.trash_progress {
            progress.completed = completed;
        }
    }

    pub(crate) fn finish_trash_job(&mut self, completed: usize) -> TrashJobCompletion {
        let progress = self.trash_progress.take();
        let duplicate_targets = progress
            .as_ref()
            .and_then(|progress| {
                (completed == progress.total).then(|| progress.duplicate_targets.clone())
            })
            .flatten();
        let next_selection = progress.and_then(|progress| {
            (completed == progress.total)
                .then_some(progress.next_selection)
                .flatten()
        });
        TrashJobCompletion {
            duplicate_targets,
            next_selection,
            source_cwd: self.trash_source_cwd.take(),
        }
    }

    pub(crate) fn cancel_trash_job(&mut self) -> Option<u64> {
        let progress = self.trash_progress.as_ref()?;
        let token = self.trash_token;
        if progress.permanent {
            self.trash_progress = None;
        }
        Some(token)
    }

    pub fn trash_is_open(&self) -> bool {
        self.trash.is_some()
    }

    #[cfg(test)]
    pub(crate) fn trash_is_permanent(&self) -> bool {
        self.trash.as_ref().is_some_and(|trash| trash.permanent)
    }

    pub fn trash_progress(&self) -> Option<(usize, usize, bool)> {
        self.trash_progress
            .as_ref()
            .map(|p| (p.completed, p.total, p.permanent))
    }

    pub fn trash_title(&self) -> String {
        let Some(t) = &self.trash else {
            return String::new();
        };
        let verb = if t.permanent {
            "Delete permanently"
        } else {
            "Trash"
        };
        match t.targets.len() {
            0 => String::new(),
            1 => {
                let kind = if t.targets[0].is_dir {
                    "folder"
                } else {
                    "file"
                };
                format!("{verb} 1 selected {kind}?")
            }
            _ => {
                let files = t.targets.iter().filter(|target| !target.is_dir).count();
                let dirs = t.targets.iter().filter(|target| target.is_dir).count();
                let desc = match (files, dirs) {
                    (f, 0) => format!("{f} file{}", if f == 1 { "" } else { "s" }),
                    (0, d) => format!("{d} folder{}", if d == 1 { "" } else { "s" }),
                    (f, d) => format!(
                        "{f} file{} and {d} folder{}",
                        if f == 1 { "" } else { "s" },
                        if d == 1 { "" } else { "s" }
                    ),
                };
                format!("{verb} {desc}?")
            }
        }
    }

    pub fn trash_scroll(&self) -> usize {
        self.trash.as_ref().map_or(0, |t| t.scroll)
    }

    pub fn trash_target_count(&self) -> usize {
        self.trash.as_ref().map_or(0, |t| t.targets.len())
    }

    pub fn trash_visible_rows(&self) -> usize {
        self.trash_target_count().min(8)
    }

    pub fn trash_target_path_at(&self, index: usize) -> Option<&std::path::Path> {
        self.trash
            .as_ref()
            .and_then(|t| t.targets.get(index))
            .map(|target| target.path.as_path())
    }

    pub fn trash_target_is_dir_at(&self, index: usize) -> bool {
        self.trash
            .as_ref()
            .and_then(|t| t.targets.get(index))
            .is_some_and(|target| target.is_dir)
    }

    pub fn trash_confirmed(&self) -> bool {
        self.trash.as_ref().is_some_and(|t| t.confirmed)
    }
}
