use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    time::Instant,
};

#[derive(Clone, Debug)]
pub(crate) enum DirectoryHistoryMode {
    None,
    PushCurrent,
    GoBack,
    GoForward,
}

#[derive(Clone, Debug)]
pub(crate) enum DirectoryLoadCompletion {
    Keep,
    Clear,
    Status(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoryEntry {
    pub(crate) cwd: PathBuf,
    pub(crate) selected_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DirectoryHistory {
    pub(crate) back: Vec<HistoryEntry>,
    pub(crate) forward: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DirectoryViewMemory {
    pub(crate) selected_path: Option<PathBuf>,
    pub(crate) scroll_row: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingDirectoryLoad {
    pub(crate) token: u64,
    pub(crate) target_cwd: PathBuf,
    pub(crate) previous_cwd: PathBuf,
    pub(crate) previous_selected_path: Option<PathBuf>,
    pub(crate) previous_selection_name: Option<String>,
    pub(crate) reselect_path: Option<PathBuf>,
    pub(crate) history_mode: DirectoryHistoryMode,
    pub(crate) refresh_search: bool,
    pub(crate) completion: DirectoryLoadCompletion,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingDirectoryFingerprintScan {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) show_hidden: bool,
}

pub(crate) struct DirectoryRuntime {
    pub(crate) load_token: u64,
    pub(crate) fingerprint_token: u64,
    pub(crate) fingerprint: crate::filesystem::DirectoryFingerprint,
    pub(crate) watch_tx: Sender<crate::filesystem::DirectoryWatchEvent>,
    pub(crate) watch_rx: Receiver<crate::filesystem::DirectoryWatchEvent>,
    pub(crate) watch: Option<crate::filesystem::DirectoryWatcher>,
    pub(crate) pending_reload_at: Option<Instant>,
    pub(crate) pending_fingerprint_scan: Option<PendingDirectoryFingerprintScan>,
    pub(crate) pending_load: Option<PendingDirectoryLoad>,
    pub(crate) use_polling_reload: bool,
    pub(crate) last_auto_reload_at: Instant,
}

impl DirectoryRuntime {
    pub(super) fn new() -> Self {
        let (watch_tx, watch_rx) = std::sync::mpsc::channel();
        Self {
            load_token: 0,
            fingerprint_token: 0,
            fingerprint: crate::filesystem::DirectoryFingerprint::default(),
            watch_tx,
            watch_rx,
            watch: None,
            pending_reload_at: None,
            pending_fingerprint_scan: None,
            pending_load: None,
            use_polling_reload: true,
            last_auto_reload_at: Instant::now(),
        }
    }
}

impl super::FileBrowserState {
    pub(crate) fn current_directory_escape_for_paths(&self, paths: &[PathBuf]) -> Option<PathBuf> {
        paths
            .iter()
            .filter(|path| self.cwd == **path || self.cwd.starts_with(path))
            .filter_map(|path| path.parent().map(std::path::Path::to_path_buf))
            .min_by_key(|path| path.components().count())
    }

    pub(crate) fn remembered_view(&self, cwd: &std::path::Path) -> Option<DirectoryViewMemory> {
        self.directory_view_memory.get(cwd).cloned()
    }

    pub(crate) fn remember_current_directory_view(&mut self) {
        self.directory_view_memory.insert(
            self.cwd.clone(),
            DirectoryViewMemory {
                selected_path: self.selected_entry().map(|entry| entry.path.clone()),
                scroll_row: self.scroll_row,
            },
        );
    }

    pub(crate) fn apply_directory_history(
        &mut self,
        mode: DirectoryHistoryMode,
        previous_cwd: PathBuf,
        previous_selected_path: Option<PathBuf>,
    ) {
        match mode {
            DirectoryHistoryMode::None => {}
            DirectoryHistoryMode::PushCurrent => {
                self.directory_history.back.push(HistoryEntry {
                    cwd: previous_cwd,
                    selected_path: previous_selected_path,
                });
                self.directory_history.forward.clear();
            }
            DirectoryHistoryMode::GoBack => {
                if !self.directory_history.back.is_empty() {
                    self.directory_history.back.pop();
                }
                self.directory_history.forward.push(HistoryEntry {
                    cwd: previous_cwd,
                    selected_path: previous_selected_path,
                });
            }
            DirectoryHistoryMode::GoForward => {
                if !self.directory_history.forward.is_empty() {
                    self.directory_history.forward.pop();
                }
                self.directory_history.back.push(HistoryEntry {
                    cwd: previous_cwd,
                    selected_path: previous_selected_path,
                });
            }
        }
    }
}
