mod actions;
mod events;
mod searching;
mod support;

use self::searching::spawn_search_worker;
use crate::search::SearchCandidate;
use anyhow::{Context, Result};
use ratatui::{layout::Rect, text::Line};
use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant, SystemTime},
};

pub(crate) use self::support::{folder_color, format_size, format_time_ago, rect_contains};

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(450);
const WHEEL_SCROLL_INTERVAL_HORIZONTAL: Duration = Duration::from_millis(36);
const WHEEL_SCROLL_INTERVAL_VERTICAL: Duration = Duration::from_millis(42);
const WHEEL_SCROLL_INTERVAL_SEARCH: Duration = Duration::from_millis(38);
const WHEEL_SCROLL_QUEUE_LIMIT: isize = 8;
const SEARCH_MATCH_LIMIT: usize = 250;
const SEARCH_CACHE_LIMIT: usize = 32;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub name_key: String,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub readonly: bool,
    pub hidden: bool,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Directory
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            EntryKind::Directory => "Folder",
            EntryKind::File => "File",
        }
    }

    pub fn detail_label(&self) -> &'static str {
        match self.kind {
            EntryKind::Directory => "Folder",
            EntryKind::File => match support::extension_class(&self.path) {
                "code" => "Code file",
                "image" => "Image file",
                "audio" => "Audio file",
                "video" => "Video file",
                "archive" => "Archive file",
                "text" => "Text file",
                _ => "File",
            },
        }
    }

    pub fn badge(&self) -> &'static str {
        match self.kind {
            EntryKind::Directory => "󰉋 DIR",
            EntryKind::File => match support::extension_class(&self.path) {
                "code" => "󰆍 CODE",
                "image" => "󰋩 IMG",
                "audio" => "󰎆 AUDIO",
                "video" => "󰈫 VIDEO",
                "archive" => "󰗄 ARC",
                "text" => "󰈙 TXT",
                _ => "󰈔 FILE",
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct SidebarItem {
    pub title: String,
    pub icon: &'static str,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct FrameState {
    pub sidebar_hits: Vec<PathHit>,
    pub entry_hits: Vec<EntryHit>,
    pub search_hits: Vec<SearchHit>,
    pub search_panel: Option<Rect>,
    pub back_button: Option<Rect>,
    pub forward_button: Option<Rect>,
    pub parent_button: Option<Rect>,
    pub refresh_button: Option<Rect>,
    pub hidden_button: Option<Rect>,
    pub view_button: Option<Rect>,
    pub metrics: ViewMetrics,
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

#[derive(Clone, Debug)]
struct ClickState {
    path: PathBuf,
    at: Instant,
}

#[derive(Clone, Debug)]
struct ScrollLane {
    pending: isize,
    last_step_at: Option<Instant>,
}

#[derive(Clone, Debug)]
struct ScrollState {
    horizontal: ScrollLane,
    vertical: ScrollLane,
    search: ScrollLane,
}

#[derive(Clone, Debug)]
struct SearchOverlay {
    scope: SearchScope,
    query: String,
    query_cursor: usize,
    candidates: Arc<Vec<SearchCandidate>>,
    matches: Vec<usize>,
    cached_matches: HashMap<String, Vec<usize>>,
    selected: usize,
    scroll: usize,
    loading: bool,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

    fn candidate_scope(self) -> crate::search::SearchCandidateScope {
        match self {
            Self::Folders => crate::search::SearchCandidateScope::Folders,
            Self::Files => crate::search::SearchCandidateScope::Files,
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

#[derive(Clone, Debug)]
struct SearchCache {
    cwd: PathBuf,
    scope: SearchScope,
    candidates: Arc<Vec<SearchCandidate>>,
}

#[derive(Debug)]
struct SearchBuild {
    token: u64,
    cwd: PathBuf,
    scope: SearchScope,
    result: Result<Arc<Vec<SearchCandidate>>, String>,
}

#[derive(Debug)]
struct SearchRequest {
    token: u64,
    cwd: PathBuf,
    scope: SearchScope,
}

pub struct App {
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    pub sidebar: Vec<SidebarItem>,
    pub selected: usize,
    pub scroll_row: usize,
    pub view_mode: ViewMode,
    pub zoom_level: u8,
    pub sort_mode: SortMode,
    pub show_hidden: bool,
    pub status: String,
    pub help_open: bool,
    pub should_quit: bool,
    back_history: Vec<PathBuf>,
    forward_history: Vec<PathBuf>,
    preview_cache: Vec<Line<'static>>,
    frame_state: FrameState,
    search: Option<SearchOverlay>,
    search_cache: Option<SearchCache>,
    search_loading: bool,
    search_token: u64,
    search_request_tx: mpsc::Sender<SearchRequest>,
    search_rx: mpsc::Receiver<SearchBuild>,
    last_click: Option<ClickState>,
    wheel_scroll: ScrollState,
}

impl App {
    pub fn new() -> Result<Self> {
        let cwd = env::current_dir().context("failed to read current directory")?;
        Self::new_at(cwd)
    }

    pub fn new_at(cwd: PathBuf) -> Result<Self> {
        let (search_request_tx, search_request_rx) = mpsc::channel();
        let (search_tx, search_rx) = mpsc::channel();
        spawn_search_worker(search_request_rx, search_tx);
        let mut app = Self {
            cwd,
            entries: Vec::new(),
            sidebar: Vec::new(),
            selected: 0,
            scroll_row: 0,
            view_mode: ViewMode::Grid,
            zoom_level: 1,
            sort_mode: SortMode::Name,
            show_hidden: false,
            status: String::new(),
            help_open: false,
            should_quit: false,
            back_history: Vec::new(),
            forward_history: Vec::new(),
            preview_cache: Vec::new(),
            frame_state: FrameState::default(),
            search: None,
            search_cache: None,
            search_loading: false,
            search_token: 0,
            search_request_tx,
            search_rx,
            last_click: None,
            wheel_scroll: ScrollState {
                horizontal: ScrollLane {
                    pending: 0,
                    last_step_at: None,
                },
                vertical: ScrollLane {
                    pending: 0,
                    last_step_at: None,
                },
                search: ScrollLane {
                    pending: 0,
                    last_step_at: None,
                },
            },
        };
        app.reload()?;
        Ok(app)
    }

    pub fn set_frame_state(&mut self, frame_state: FrameState) -> bool {
        self.frame_state = frame_state;
        self.sync_scroll() | self.sync_search_scroll()
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }
}
