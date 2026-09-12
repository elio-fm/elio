use super::{
    DirectoryCountViewport, DirectoryHistory, DirectoryItemCountKey, DirectoryRuntime,
    DirectoryViewMemory, LocalFilter, SelectedPaths, ViewMode,
};
use crate::fs::{Entry, SortMode};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    time::Instant,
};

pub(crate) struct FileBrowserState {
    pub(crate) cwd: PathBuf,
    pub(crate) entries: Vec<Entry>,
    pub(crate) unfiltered_entries: Vec<Entry>,
    pub(crate) local_filter: LocalFilter,
    pub(crate) selected: usize,
    pub(crate) scroll_row: usize,
    pub(crate) view_mode: ViewMode,
    pub(crate) zoom_level: u8,
    pub(crate) sort_mode: SortMode,
    pub(crate) show_hidden: bool,
    /// True when the loaded directory is the trash folder.
    /// Set when the directory snapshot completes.
    pub(crate) in_trash: bool,
    pub(crate) directory_history: DirectoryHistory,
    pub(crate) selected_paths: SelectedPaths,
    pub(crate) directory_item_count_cache: HashMap<DirectoryItemCountKey, Option<usize>>,
    pub(crate) directory_item_count_order: VecDeque<DirectoryItemCountKey>,
    pub(crate) directory_count_viewport: Option<DirectoryCountViewport>,
    pub(crate) directory_item_count_ready_at: Option<Instant>,
    pub(crate) directory_view_memory: HashMap<PathBuf, DirectoryViewMemory>,
    pub(crate) directory_runtime: DirectoryRuntime,
}

impl FileBrowserState {
    pub(crate) fn new(
        cwd: PathBuf,
        start_in_grid: bool,
        zoom_level: u8,
        show_hidden: bool,
    ) -> Self {
        Self {
            cwd,
            entries: Vec::new(),
            unfiltered_entries: Vec::new(),
            local_filter: LocalFilter::default(),
            selected: 0,
            scroll_row: 0,
            view_mode: ViewMode::from_start_in_grid(start_in_grid),
            zoom_level,
            sort_mode: SortMode::Name,
            show_hidden,
            in_trash: false,
            directory_history: DirectoryHistory::default(),
            selected_paths: SelectedPaths::default(),
            directory_item_count_cache: HashMap::new(),
            directory_item_count_order: VecDeque::new(),
            directory_count_viewport: None,
            directory_item_count_ready_at: None,
            directory_view_memory: HashMap::new(),
            directory_runtime: DirectoryRuntime::new(),
        }
    }

    pub(crate) fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }
}
