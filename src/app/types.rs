use std::{path::PathBuf, time::SystemTime};

use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    Grid,
    List,
}

impl ViewMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Grid => Self::List,
            Self::List => Self::Grid,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Grid => "Grid",
            Self::List => "List",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SortMode {
    Name,
    Modified,
    Size,
}

impl SortMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Name => Self::Modified,
            Self::Modified => Self::Size,
            Self::Size => Self::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Modified => "Modified",
            Self::Size => "Size",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    Directory,
    File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClipOp {
    Yank,
    Cut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum FileClass {
    Directory,
    Code,
    Config,
    Document,
    License,
    Image,
    Audio,
    Video,
    Archive,
    Font,
    Data,
    File,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub name_key: String,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub readonly: bool,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Directory
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidebarItem {
    pub kind: SidebarItemKind,
    pub title: String,
    pub icon: &'static str,
    pub path: PathBuf,
}

impl SidebarItem {
    pub fn new(
        kind: SidebarItemKind,
        title: impl Into<String>,
        icon: &'static str,
        path: PathBuf,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            icon,
            path,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SidebarItemKind {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Root,
    Trash,
    Device { removable: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SidebarRow {
    Section { title: &'static str },
    Item(SidebarItem),
}

impl SidebarRow {
    pub fn item(&self) -> Option<&SidebarItem> {
        match self {
            Self::Item(item) => Some(item),
            Self::Section { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrameState {
    pub sidebar_hits: Vec<PathHit>,
    pub entry_hits: Vec<EntryHit>,
    pub search_hits: Vec<SearchHit>,
    pub goto_hits: Vec<GoToHit>,
    pub copy_hits: Vec<CopyHit>,
    pub trash_panel: Option<Rect>,
    pub trash_confirm_btn: Option<Rect>,
    pub trash_cancel_btn: Option<Rect>,
    pub restore_panel: Option<Rect>,
    pub restore_confirm_btn: Option<Rect>,
    pub restore_cancel_btn: Option<Rect>,
    pub create_panel: Option<Rect>,
    pub rename_panel: Option<Rect>,
    pub create_list_area: Option<Rect>,
    pub create_scroll_top: usize,
    pub bulk_rename_list_area: Option<Rect>,
    pub bulk_rename_scroll_top: usize,
    pub goto_panel: Option<Rect>,
    pub copy_panel: Option<Rect>,
    pub search_panel: Option<Rect>,
    pub help_panel: Option<Rect>,
    pub entries_panel: Option<Rect>,
    pub preview_panel: Option<Rect>,
    pub preview_body_area: Option<Rect>,
    pub preview_media_area: Option<Rect>,
    pub preview_content_area: Option<Rect>,
    pub back_button: Option<Rect>,
    pub forward_button: Option<Rect>,
    pub parent_button: Option<Rect>,
    pub hidden_button: Option<Rect>,
    pub view_button: Option<Rect>,
    pub metrics: ViewMetrics,
    pub preview_rows_visible: usize,
    pub preview_cols_visible: usize,
    pub search_rows_visible: usize,
}

#[derive(Clone, Debug)]
pub struct PathHit {
    pub rect: Rect,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct EntryHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct GoToHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct CopyHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewMetrics {
    pub cols: usize,
    pub rows_visible: usize,
}

impl Default for ViewMetrics {
    fn default() -> Self {
        Self {
            cols: 1,
            rows_visible: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SearchScope {
    Folders,
    Files,
}

impl SearchScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Folders => "Folders",
            Self::Files => "Files",
        }
    }

    pub(super) fn candidate_scope(self) -> crate::fs::search::SearchCandidateScope {
        match self {
            Self::Folders => crate::fs::search::SearchCandidateScope::Folders,
            Self::Files => crate::fs::search::SearchCandidateScope::Files,
        }
    }

    pub fn empty_label(self) -> &'static str {
        match self {
            Self::Folders => "No matching folders in this tree",
            Self::Files => "No matching files in this tree",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchRow {
    pub index: usize,
    pub name: String,
    pub relative: String,
    pub is_dir: bool,
    pub selected: bool,
}
