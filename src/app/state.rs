use std::{
    collections::{HashMap, VecDeque},
    env,
    path::PathBuf,
    sync::Arc,
    time::{Instant, SystemTime},
};

use anyhow::{Context, Result};

use super::{
    jobs::JobScheduler,
    overlays::{images, inline_image, pdf},
    types::*,
};
use crate::fs::search::SearchCandidate;
use crate::preview;

#[derive(Clone, Debug)]
pub(super) struct ClickState {
    pub(super) path: PathBuf,
    pub(super) at: Instant,
}

#[derive(Clone, Debug)]
pub(super) struct ScrollLane {
    pub(super) pending: isize,
    pub(super) remainder: isize,
    pub(super) last_step_at: Option<Instant>,
    pub(super) last_input_at: Option<Instant>,
    pub(super) last_input_direction: isize,
    pub(super) burst_count: u8,
}

impl ScrollLane {
    pub(super) fn new() -> Self {
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

#[derive(Clone, Debug)]
pub(super) struct ScrollState {
    pub(super) horizontal: ScrollLane,
    pub(super) vertical: ScrollLane,
    pub(super) preview: ScrollLane,
    pub(super) preview_horizontal: ScrollLane,
    pub(super) search: ScrollLane,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WheelTarget {
    Entries,
    Preview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WheelProfile {
    Default,
    HighFrequency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NavigationRepeatKey {
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
pub(super) struct SearchOverlay {
    pub(super) scope: SearchScope,
    pub(super) query: String,
    pub(super) query_cursor: usize,
    pub(super) candidates: Arc<Vec<SearchCandidate>>,
    pub(super) matches: Vec<usize>,
    pub(super) cached_matches: HashMap<String, Vec<usize>>,
    pub(super) selected: usize,
    pub(super) scroll: usize,
    pub(super) loading: bool,
    pub(super) error: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct SearchCache {
    pub(super) cwd: PathBuf,
    pub(super) scope: SearchScope,
    pub(super) candidates: Arc<Vec<SearchCandidate>>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedPreview {
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) preview: preview::PreviewContent,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct DirectoryItemCountKey {
    pub(super) path: PathBuf,
    pub(super) modified: Option<SystemTime>,
    pub(super) show_hidden: bool,
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
pub(super) struct PreviewMetrics {
    pub(super) cache_hits: u64,
    pub(super) cache_misses: u64,
    pub(super) applied_results: u64,
    pub(super) stale_results_dropped: u64,
}

impl PreviewMetrics {
    #[cfg(test)]
    pub(super) fn snapshot(self) -> PreviewMetricsSnapshot {
        PreviewMetricsSnapshot {
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            applied_results: self.applied_results,
            stale_results_dropped: self.stale_results_dropped,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum DirectoryHistoryMode {
    None,
    PushCurrent,
    GoBack,
    GoForward,
}

#[derive(Clone, Debug)]
pub(super) enum DirectoryLoadCompletion {
    Keep,
    Clear,
    Status(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HistoryEntry {
    pub(super) cwd: PathBuf,
    pub(super) selected_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct NavigationHistory {
    pub(super) back: Vec<HistoryEntry>,
    pub(super) forward: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct DirectoryViewMemory {
    pub(super) selected_path: Option<PathBuf>,
    pub(super) scroll_row: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DirectoryCountViewport {
    pub(super) fingerprint: crate::fs::DirectoryFingerprint,
    pub(super) scroll_row: usize,
    pub(super) cols: usize,
    pub(super) rows_visible: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PreviewLoadState {
    Placeholder(PathBuf),
    Refreshing(PathBuf),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PreviewRefreshMode {
    Immediate,
    Deferred,
}

pub(super) struct PreviewState {
    pub(super) scroll: usize,
    pub(super) horizontal_scroll: usize,
    pub(super) content: preview::PreviewContent,
    pub(super) token: u64,
    pub(super) metrics: PreviewMetrics,
    pub(super) load_state: Option<PreviewLoadState>,
    pub(super) deferred_refresh_at: Option<Instant>,
    pub(super) result_cache: HashMap<PathBuf, CachedPreview>,
    pub(super) result_order: VecDeque<PathBuf>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingDirectoryLoad {
    pub(super) token: u64,
    pub(super) target_cwd: PathBuf,
    pub(super) previous_cwd: PathBuf,
    pub(super) previous_selected_path: Option<PathBuf>,
    pub(super) previous_selection_name: Option<String>,
    pub(super) reselect_path: Option<PathBuf>,
    pub(super) history_mode: DirectoryHistoryMode,
    pub(super) refresh_search: bool,
    pub(super) completion: DirectoryLoadCompletion,
}

pub(super) struct DirectoryRuntime {
    pub(super) fingerprint: crate::fs::DirectoryFingerprint,
    pub(super) watch_tx: std::sync::mpsc::Sender<crate::fs::DirectoryWatchEvent>,
    pub(super) watch_rx: std::sync::mpsc::Receiver<crate::fs::DirectoryWatchEvent>,
    pub(super) watch: Option<crate::fs::DirectoryWatcher>,
    pub(super) pending_reload_at: Option<Instant>,
    pub(super) pending_load: Option<PendingDirectoryLoad>,
    pub(super) use_polling_reload: bool,
    pub(super) last_auto_reload_at: Instant,
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
    pub(super) navigation_history: NavigationHistory,
    pub(super) preview_state: PreviewState,
    pub(super) image_preview: images::ImagePreviewState,
    pub(super) pdf_preview: pdf::PdfPreviewState,
    pub(super) terminal_images: inline_image::TerminalImageState,
    pub(super) frame_state: FrameState,
    pub(super) search: Option<SearchOverlay>,
    pub(super) search_cache: Option<SearchCache>,
    pub(super) search_loading: bool,
    pub(super) search_token: u64,
    pub(super) directory_token: u64,
    pub(super) scheduler: JobScheduler,
    pub(super) directory_item_count_cache: HashMap<DirectoryItemCountKey, Option<usize>>,
    pub(super) directory_item_count_order: VecDeque<DirectoryItemCountKey>,
    pub(super) directory_count_viewport: Option<DirectoryCountViewport>,
    pub(super) directory_view_memory: HashMap<PathBuf, DirectoryViewMemory>,
    pub(super) directory_runtime: DirectoryRuntime,
    pub(super) last_click: Option<ClickState>,
    pub(super) wheel_scroll: ScrollState,
    pub(super) wheel_profile: WheelProfile,
    pub(super) last_wheel_target: Option<WheelTarget>,
    pub(super) browser_wheel_post_burst_pending: bool,
    pub(super) last_navigation_key: Option<(NavigationRepeatKey, Instant)>,
    pub(super) last_selection_change_at: Instant,
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
            image_preview: images::ImagePreviewState::default(),
            pdf_preview: pdf::PdfPreviewState::default(),
            terminal_images: inline_image::TerminalImageState::default(),
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
                fingerprint: crate::fs::DirectoryFingerprint::default(),
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
        let snapshot =
            crate::fs::load_directory_snapshot(&app.cwd, app.show_hidden, app.sort_mode)?;
        app.sidebar = crate::fs::build_sidebar_items();
        app.entries = snapshot.entries;
        app.directory_runtime.fingerprint = snapshot.fingerprint;
        app.clamp_selection();
        app.remember_current_directory_view();
        app.refresh_preview();
        app.reset_directory_watch();
        Ok(app)
    }
}

pub(super) fn detect_wheel_profile() -> WheelProfile {
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
