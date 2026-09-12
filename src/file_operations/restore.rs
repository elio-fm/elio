use super::FileOperationsState;
use super::trash_delete::TrashTarget;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct RestoreProgress {
    pub(crate) completed: usize,
    pub(crate) total: usize,
    pub(crate) next_selection: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub(crate) struct RestoreOverlay {
    pub(crate) targets: Vec<TrashTarget>,
    pub(crate) scroll: usize,
    pub(crate) confirmed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct RestoreRequest {
    pub(crate) token: u64,
    pub(crate) targets: Vec<TrashTarget>,
}

pub(crate) enum RestoreConfirmation {
    None,
    InProgress,
    Targets(RestoreOverlay),
}

impl FileOperationsState {
    pub(crate) fn open_restore_prompt(&mut self, targets: Vec<TrashTarget>) {
        self.create = None;
        self.trash = None;
        self.restore = Some(RestoreOverlay {
            targets,
            scroll: 0,
            confirmed: true,
        });
    }

    pub(crate) fn take_restore_confirmation(&mut self) -> RestoreConfirmation {
        if self.restore_progress.is_some() {
            self.restore = None;
            return RestoreConfirmation::InProgress;
        }
        match self.restore.take() {
            Some(overlay) if !overlay.targets.is_empty() => RestoreConfirmation::Targets(overlay),
            _ => RestoreConfirmation::None,
        }
    }

    pub(crate) fn start_restore(
        &mut self,
        overlay: RestoreOverlay,
        source_cwd: PathBuf,
        next_selection: Option<PathBuf>,
    ) -> RestoreRequest {
        let token = self.restore_token.wrapping_add(1);
        self.restore_token = token;
        self.restore_progress = Some(RestoreProgress {
            completed: 0,
            total: overlay.targets.len(),
            next_selection,
        });
        self.restore_source_cwd = Some(source_cwd);
        RestoreRequest {
            token,
            targets: overlay.targets,
        }
    }

    pub fn restore_is_open(&self) -> bool {
        self.restore.is_some()
    }

    pub fn restore_progress(&self) -> Option<(usize, usize)> {
        self.restore_progress
            .as_ref()
            .map(|p| (p.completed, p.total))
    }

    pub fn restore_title(&self) -> String {
        let Some(r) = &self.restore else {
            return String::new();
        };
        match r.targets.len() {
            0 => String::new(),
            1 => {
                let kind = if r.targets[0].is_dir {
                    "folder"
                } else {
                    "file"
                };
                format!("Restore 1 selected {kind}?")
            }
            _ => {
                let files = r.targets.iter().filter(|target| !target.is_dir).count();
                let dirs = r.targets.iter().filter(|target| target.is_dir).count();
                let desc = match (files, dirs) {
                    (f, 0) => format!("{f} file{}", if f == 1 { "" } else { "s" }),
                    (0, d) => format!("{d} folder{}", if d == 1 { "" } else { "s" }),
                    (f, d) => format!(
                        "{f} file{} and {d} folder{}",
                        if f == 1 { "" } else { "s" },
                        if d == 1 { "" } else { "s" }
                    ),
                };
                format!("Restore {desc}?")
            }
        }
    }

    pub fn restore_scroll(&self) -> usize {
        self.restore.as_ref().map_or(0, |r| r.scroll)
    }

    pub fn restore_target_count(&self) -> usize {
        self.restore.as_ref().map_or(0, |r| r.targets.len())
    }

    pub fn restore_visible_rows(&self) -> usize {
        self.restore_target_count().min(8)
    }

    pub fn restore_target_name_at(&self, index: usize) -> Option<&str> {
        self.restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .map(|target| target.name.as_str())
    }

    pub fn restore_target_path_at(&self, index: usize) -> Option<&std::path::Path> {
        self.restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .map(|target| target.path.as_path())
    }

    pub fn restore_target_is_dir_at(&self, index: usize) -> bool {
        self.restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .is_some_and(|target| target.is_dir)
    }

    pub fn restore_confirmed(&self) -> bool {
        self.restore.as_ref().is_some_and(|r| r.confirmed)
    }
}
