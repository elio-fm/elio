mod actions;
mod events;
mod preview;
mod previewing;
mod searching;
mod support;
mod watching;

use self::previewing::spawn_preview_worker;
use self::searching::spawn_search_worker;
use crate::search::SearchCandidate;
use anyhow::{Context, Result};
use ratatui::{layout::Rect, text::Line};
use std::{
    collections::{HashMap, VecDeque},
    env,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant, SystemTime},
};

pub(crate) use self::support::{format_size, format_time_ago, rect_contains};

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(450);
const WHEEL_SCROLL_INTERVAL_HORIZONTAL: Duration = Duration::from_millis(64);
const WHEEL_SCROLL_INTERVAL_VERTICAL: Duration = Duration::from_millis(42);
const WHEEL_SCROLL_INTERVAL_PREVIEW: Duration = Duration::from_millis(22);
const WHEEL_SCROLL_INTERVAL_PREVIEW_HORIZONTAL: Duration = Duration::from_millis(18);
const WHEEL_SCROLL_INTERVAL_SEARCH: Duration = Duration::from_millis(72);
const WHEEL_SCROLL_QUEUE_LIMIT: isize = 8;
const WHEEL_SCROLL_QUEUE_LIMIT_HORIZONTAL: isize = 3;
const WHEEL_SCROLL_QUEUE_LIMIT_PREVIEW_HORIZONTAL: isize = 10;
const WHEEL_SCROLL_QUEUE_LIMIT_SEARCH: isize = 2;
const SEARCH_MATCH_LIMIT: usize = 250;
const SEARCH_CACHE_LIMIT: usize = 32;
const PREVIEW_CACHE_LIMIT: usize = 24;
const AUTO_RELOAD_INTERVAL: Duration = Duration::from_millis(250);

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum FileClass {
    Directory,
    Code,
    Config,
    Document,
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
    pub preview_panel: Option<Rect>,
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
    remainder: isize,
    last_step_at: Option<Instant>,
}

#[derive(Clone, Debug)]
struct ScrollState {
    horizontal: ScrollLane,
    vertical: ScrollLane,
    preview: ScrollLane,
    preview_horizontal: ScrollLane,
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

#[derive(Debug)]
struct PreviewBuild {
    token: u64,
    entry: Entry,
    result: preview::PreviewContent,
}

#[derive(Debug)]
struct PreviewRequest {
    token: u64,
    entry: Entry,
}

#[derive(Clone, Debug)]
struct CachedPreview {
    size: u64,
    modified: Option<SystemTime>,
    preview: preview::PreviewContent,
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
    pub preview_scroll: usize,
    pub preview_horizontal_scroll: usize,
    pub status: String,
    pub help_open: bool,
    pub should_quit: bool,
    back_history: Vec<PathBuf>,
    forward_history: Vec<PathBuf>,
    preview_cache: preview::PreviewContent,
    frame_state: FrameState,
    search: Option<SearchOverlay>,
    search_cache: Option<SearchCache>,
    search_loading: bool,
    search_token: u64,
    search_request_tx: mpsc::Sender<SearchRequest>,
    search_rx: mpsc::Receiver<SearchBuild>,
    preview_token: u64,
    preview_request_tx: mpsc::Sender<PreviewRequest>,
    preview_rx: mpsc::Receiver<PreviewBuild>,
    preview_result_cache: HashMap<PathBuf, CachedPreview>,
    preview_result_order: VecDeque<PathBuf>,
    directory_watch_tx: mpsc::Sender<watching::DirectoryWatchEvent>,
    directory_watch_rx: mpsc::Receiver<watching::DirectoryWatchEvent>,
    directory_watch: Option<watching::DirectoryWatcher>,
    pending_directory_reload_at: Option<Instant>,
    use_polling_reload: bool,
    last_click: Option<ClickState>,
    wheel_scroll: ScrollState,
    wheel_step_divisor: isize,
    directory_fingerprint: support::DirectoryFingerprint,
    last_auto_reload_at: Instant,
}

impl App {
    pub fn new() -> Result<Self> {
        let cwd = env::current_dir().context("failed to read current directory")?;
        Self::new_at(cwd)
    }

    pub fn new_at(cwd: PathBuf) -> Result<Self> {
        let (search_request_tx, search_request_rx) = mpsc::channel();
        let (search_tx, search_rx) = mpsc::channel();
        let (preview_request_tx, preview_request_rx) = mpsc::channel();
        let (preview_tx, preview_rx) = mpsc::channel();
        let (directory_watch_tx, directory_watch_rx) = mpsc::channel();
        spawn_search_worker(search_request_rx, search_tx);
        spawn_preview_worker(preview_request_rx, preview_tx);
        let mut app = Self {
            cwd,
            entries: Vec::new(),
            sidebar: Vec::new(),
            selected: 0,
            scroll_row: 0,
            view_mode: ViewMode::List,
            zoom_level: 1,
            sort_mode: SortMode::Name,
            show_hidden: false,
            preview_scroll: 0,
            preview_horizontal_scroll: 0,
            status: String::new(),
            help_open: false,
            should_quit: false,
            back_history: Vec::new(),
            forward_history: Vec::new(),
            preview_cache: preview::PreviewContent::placeholder("No selection"),
            frame_state: FrameState::default(),
            search: None,
            search_cache: None,
            search_loading: false,
            search_token: 0,
            search_request_tx,
            search_rx,
            preview_token: 0,
            preview_request_tx,
            preview_rx,
            preview_result_cache: HashMap::new(),
            preview_result_order: VecDeque::new(),
            directory_watch_tx,
            directory_watch_rx,
            directory_watch: None,
            pending_directory_reload_at: None,
            use_polling_reload: true,
            last_click: None,
            wheel_scroll: ScrollState {
                horizontal: ScrollLane {
                    pending: 0,
                    remainder: 0,
                    last_step_at: None,
                },
                vertical: ScrollLane {
                    pending: 0,
                    remainder: 0,
                    last_step_at: None,
                },
                preview: ScrollLane {
                    pending: 0,
                    remainder: 0,
                    last_step_at: None,
                },
                preview_horizontal: ScrollLane {
                    pending: 0,
                    remainder: 0,
                    last_step_at: None,
                },
                search: ScrollLane {
                    pending: 0,
                    remainder: 0,
                    last_step_at: None,
                },
            },
            wheel_step_divisor: wheel_step_divisor(),
            directory_fingerprint: support::DirectoryFingerprint::default(),
            last_auto_reload_at: Instant::now(),
        };
        app.reload()?;
        app.reset_directory_watch();
        Ok(app)
    }

    pub fn set_frame_state(&mut self, frame_state: FrameState) -> bool {
        self.frame_state = frame_state;
        self.sync_scroll() | self.sync_search_scroll() | self.sync_preview_scroll()
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    pub fn has_pending_auto_reload(&self) -> bool {
        self.pending_directory_reload_at.is_some()
    }

    pub fn report_runtime_error(&mut self, context: &str, error: &anyhow::Error) {
        self.status = format!("{context}: {error}");
    }
}

fn wheel_step_divisor() -> isize {
    let term = env::var("TERM").unwrap_or_default();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();
    if term.contains("ghostty") || term_program.eq_ignore_ascii_case("ghostty") {
        3
    } else {
        1
    }
}
