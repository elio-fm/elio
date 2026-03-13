mod actions;
mod directory_counts;
mod events;
mod image_preview;
mod jobs;
mod pdf_preview;
mod preview;
mod previewing;
mod searching;
mod support;
mod terminal_images;
mod watching;

#[cfg(test)]
use self::jobs::SchedulerMetricsSnapshot;
use self::jobs::{JobScheduler, PreviewPriority, PreviewRequest, SearchRequest};
use crate::search::SearchCandidate;
use anyhow::{Context, Result};
use ratatui::{layout::Rect, text::Line};
use std::{
    collections::{HashMap, VecDeque},
    env,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

pub(crate) use self::support::{
    format_item_count, format_size, format_time_ago, rect_contains, sanitize_terminal_text,
};

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(450);
const KEY_REPEAT_NAV_INTERVAL: Duration = Duration::from_millis(28);
const WHEEL_SCROLL_INTERVAL_HORIZONTAL: Duration = Duration::from_millis(64);
const WHEEL_SCROLL_INTERVAL_VERTICAL: Duration = Duration::from_millis(16);
const WHEEL_SCROLL_INTERVAL_VERTICAL_HIGH_FREQUENCY: Duration = Duration::from_millis(12);
const WHEEL_SCROLL_INTERVAL_PREVIEW: Duration = Duration::from_millis(12);
const WHEEL_SCROLL_INTERVAL_PREVIEW_HORIZONTAL: Duration = Duration::from_millis(12);
const WHEEL_SCROLL_INTERVAL_SEARCH: Duration = Duration::from_millis(72);
const PREVIEW_AUTO_FOCUS_DELAY: Duration = Duration::from_millis(220);
const IMAGE_SELECTION_ACTIVATION_DELAY: Duration = Duration::from_millis(120);
const HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY: Duration = Duration::from_millis(85);
const WHEEL_SCROLL_QUEUE_LIMIT: isize = 8;
const WHEEL_SCROLL_QUEUE_LIMIT_HORIZONTAL: isize = 3;
const WHEEL_SCROLL_QUEUE_LIMIT_PREVIEW_HORIZONTAL: isize = 10;
const WHEEL_SCROLL_QUEUE_LIMIT_SEARCH: isize = 2;
const WHEEL_SCROLL_BURST_WINDOW: Duration = Duration::from_millis(90);
const SEARCH_MATCH_LIMIT: usize = 250;
const SEARCH_CACHE_LIMIT: usize = 32;
const PREVIEW_CACHE_LIMIT: usize = 24;
const PREVIEW_PREFETCH_LIMIT: usize = 2;
const DIRECTORY_ITEM_COUNT_CACHE_LIMIT: usize = 128;
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
    pub entries_panel: Option<Rect>,
    pub preview_panel: Option<Rect>,
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
    last_input_at: Option<Instant>,
    last_input_direction: isize,
    burst_count: u8,
}

#[derive(Clone, Debug)]
struct ScrollState {
    horizontal: ScrollLane,
    vertical: ScrollLane,
    preview: ScrollLane,
    preview_horizontal: ScrollLane,
    search: ScrollLane,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WheelTarget {
    Entries,
    Preview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WheelProfile {
    Default,
    HighFrequency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NavigationRepeatKey {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
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

#[derive(Clone, Debug)]
struct CachedPreview {
    size: u64,
    modified: Option<SystemTime>,
    preview: preview::PreviewContent,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DirectoryItemCountKey {
    path: PathBuf,
    modified: Option<SystemTime>,
    show_hidden: bool,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PreviewMetricsSnapshot {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub applied_results: u64,
    pub stale_results_dropped: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PreviewMetrics {
    cache_hits: u64,
    cache_misses: u64,
    applied_results: u64,
    stale_results_dropped: u64,
}

impl PreviewMetrics {
    #[cfg(test)]
    fn snapshot(self) -> PreviewMetricsSnapshot {
        PreviewMetricsSnapshot {
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            applied_results: self.applied_results,
            stale_results_dropped: self.stale_results_dropped,
        }
    }
}

#[derive(Clone, Debug)]
enum DirectoryHistoryMode {
    None,
    PushCurrent,
    GoBack,
    GoForward,
}

#[derive(Clone, Debug)]
enum DirectoryLoadCompletion {
    Keep,
    Clear,
    Status(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoryEntry {
    cwd: PathBuf,
    selected_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
struct NavigationHistory {
    back: Vec<HistoryEntry>,
    forward: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Default)]
struct DirectoryViewMemory {
    selected_path: Option<PathBuf>,
    scroll_row: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryCountViewport {
    fingerprint: support::DirectoryFingerprint,
    scroll_row: usize,
    cols: usize,
    rows_visible: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PreviewLoadState {
    Placeholder(PathBuf),
    Refreshing(PathBuf),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreviewRefreshMode {
    Immediate,
    Deferred,
}

struct PreviewState {
    scroll: usize,
    horizontal_scroll: usize,
    content: preview::PreviewContent,
    token: u64,
    metrics: PreviewMetrics,
    load_state: Option<PreviewLoadState>,
    deferred_refresh_at: Option<Instant>,
    result_cache: HashMap<PathBuf, CachedPreview>,
    result_order: VecDeque<PathBuf>,
}

#[derive(Clone, Debug)]
struct PendingDirectoryLoad {
    token: u64,
    target_cwd: PathBuf,
    previous_cwd: PathBuf,
    previous_selected_path: Option<PathBuf>,
    previous_selection_name: Option<String>,
    reselect_path: Option<PathBuf>,
    history_mode: DirectoryHistoryMode,
    refresh_search: bool,
    completion: DirectoryLoadCompletion,
}

struct DirectoryRuntime {
    fingerprint: support::DirectoryFingerprint,
    watch_tx: std::sync::mpsc::Sender<watching::DirectoryWatchEvent>,
    watch_rx: std::sync::mpsc::Receiver<watching::DirectoryWatchEvent>,
    watch: Option<watching::DirectoryWatcher>,
    pending_reload_at: Option<Instant>,
    pending_load: Option<PendingDirectoryLoad>,
    use_polling_reload: bool,
    last_auto_reload_at: Instant,
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
    navigation_history: NavigationHistory,
    preview_state: PreviewState,
    image_preview: image_preview::ImagePreviewState,
    pdf_preview: pdf_preview::PdfPreviewState,
    terminal_images: terminal_images::TerminalImageState,
    frame_state: FrameState,
    search: Option<SearchOverlay>,
    search_cache: Option<SearchCache>,
    search_loading: bool,
    search_token: u64,
    directory_token: u64,
    scheduler: JobScheduler,
    directory_item_count_cache: HashMap<DirectoryItemCountKey, Option<usize>>,
    directory_item_count_order: VecDeque<DirectoryItemCountKey>,
    directory_count_viewport: Option<DirectoryCountViewport>,
    directory_view_memory: HashMap<PathBuf, DirectoryViewMemory>,
    directory_runtime: DirectoryRuntime,
    last_click: Option<ClickState>,
    wheel_scroll: ScrollState,
    wheel_profile: WheelProfile,
    last_wheel_target: Option<WheelTarget>,
    browser_wheel_post_burst_pending: bool,
    last_navigation_key: Option<(NavigationRepeatKey, Instant)>,
    last_selection_change_at: Instant,
}

impl App {
    pub fn new() -> Result<Self> {
        let cwd = env::current_dir().context("failed to read current directory")?;
        Self::new_at(cwd)
    }

    pub fn new_at(cwd: PathBuf) -> Result<Self> {
        let scheduler = JobScheduler::new();
        let (directory_watch_tx, directory_watch_rx) = std::sync::mpsc::channel();
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
            status: String::new(),
            help_open: false,
            should_quit: false,
            navigation_history: NavigationHistory::default(),
            preview_state: PreviewState {
                scroll: 0,
                horizontal_scroll: 0,
                content: preview::PreviewContent::placeholder("No selection"),
                token: 0,
                metrics: PreviewMetrics::default(),
                load_state: None,
                deferred_refresh_at: None,
                result_cache: HashMap::new(),
                result_order: VecDeque::new(),
            },
            image_preview: image_preview::ImagePreviewState::default(),
            pdf_preview: pdf_preview::PdfPreviewState::default(),
            terminal_images: terminal_images::TerminalImageState::default(),
            frame_state: FrameState::default(),
            search: None,
            search_cache: None,
            search_loading: false,
            search_token: 0,
            directory_token: 0,
            scheduler,
            directory_item_count_cache: HashMap::new(),
            directory_item_count_order: VecDeque::new(),
            directory_count_viewport: None,
            directory_view_memory: HashMap::new(),
            directory_runtime: DirectoryRuntime {
                fingerprint: support::DirectoryFingerprint::default(),
                watch_tx: directory_watch_tx,
                watch_rx: directory_watch_rx,
                watch: None,
                pending_reload_at: None,
                pending_load: None,
                use_polling_reload: true,
                last_auto_reload_at: Instant::now(),
            },
            last_click: None,
            wheel_scroll: ScrollState {
                horizontal: ScrollLane::new(),
                vertical: ScrollLane::new(),
                preview: ScrollLane::new(),
                preview_horizontal: ScrollLane::new(),
                search: ScrollLane::new(),
            },
            wheel_profile: detect_wheel_profile(),
            last_wheel_target: Some(WheelTarget::Entries),
            browser_wheel_post_burst_pending: false,
            last_navigation_key: None,
            last_selection_change_at: Instant::now(),
        };
        let snapshot = support::load_directory_snapshot(&app.cwd, app.show_hidden, app.sort_mode)?;
        app.sidebar = support::build_sidebar_items();
        app.entries = snapshot.entries;
        app.directory_runtime.fingerprint = snapshot.fingerprint;
        app.clamp_selection();
        app.remember_current_directory_view();
        app.refresh_preview();
        app.reset_directory_watch();
        Ok(app)
    }

    pub fn set_frame_state(&mut self, frame_state: FrameState) -> bool {
        self.frame_state = frame_state;
        let dirty = self.sync_scroll() | self.sync_search_scroll() | self.sync_preview_scroll();
        if !self.browser_wheel_burst_active() {
            self.queue_visible_directory_item_counts();
        }
        self.refresh_static_image_preloads();
        self.remember_current_directory_view();
        dirty
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    pub fn has_pending_auto_reload(&self) -> bool {
        self.directory_runtime.pending_reload_at.is_some()
    }

    pub fn has_pending_background_work(&self) -> bool {
        self.scheduler.has_pending_work()
    }

    pub(crate) fn browser_wheel_burst_active(&self) -> bool {
        self.wheel_profile == WheelProfile::HighFrequency
            && self.search.is_none()
            && self.last_wheel_target == Some(WheelTarget::Entries)
            && self
                .wheel_scroll
                .vertical
                .last_input_at
                .is_some_and(|at| at.elapsed() <= WHEEL_SCROLL_BURST_WINDOW)
    }

    pub(crate) fn pending_browser_wheel_timer(&self) -> Option<Duration> {
        if !self.browser_wheel_post_burst_pending {
            return None;
        }
        self.wheel_scroll
            .vertical
            .last_input_at
            .map(|at| WHEEL_SCROLL_BURST_WINDOW.saturating_sub(at.elapsed()))
    }

    pub(crate) fn process_browser_wheel_timers(&mut self) -> bool {
        if self.browser_wheel_post_burst_pending && !self.browser_wheel_burst_active() {
            self.browser_wheel_post_burst_pending = false;
            return true;
        }
        false
    }

    #[cfg(test)]
    pub fn scheduler_metrics(&self) -> SchedulerMetricsSnapshot {
        self.scheduler.metrics_snapshot()
    }

    #[cfg(test)]
    pub fn preview_metrics(&self) -> PreviewMetricsSnapshot {
        self.preview_state.metrics.snapshot()
    }

    pub fn report_runtime_error(&mut self, context: &str, error: &anyhow::Error) {
        self.status = format!("{context}: {error}");
    }
}

impl ScrollLane {
    fn new() -> Self {
        Self {
            pending: 0,
            remainder: 0,
            last_step_at: None,
            last_input_at: None,
            last_input_direction: 0,
            burst_count: 0,
        }
    }
}

fn detect_wheel_profile() -> WheelProfile {
    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    let term_program = env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();

    let is_ghostty = term.contains("ghostty") || term_program.contains("ghostty");
    let is_alacritty = term.contains("alacritty")
        || term_program.contains("alacritty")
        || env::var_os("ALACRITTY_SOCKET").is_some();
    let is_vte = env::var_os("VTE_VERSION").is_some();
    let is_warp = term_program.contains("warp") || env::var_os("WARP_SESSION_ID").is_some();

    if is_ghostty || is_alacritty || is_vte || is_warp {
        WheelProfile::HighFrequency
    } else {
        WheelProfile::Default
    }
}
