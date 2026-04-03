use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result};

use super::{
    jobs::JobScheduler,
    overlays::{comic, epub, images, inline_image, pdf},
    types::*,
};
use crate::core::{Entry, SortMode};
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
pub(super) struct Clipboard {
    pub(super) paths: Vec<PathBuf>,
    pub(super) op: ClipOp,
}

#[derive(Clone, Debug)]
pub(super) struct PasteProgress {
    pub(super) completed: usize,
    pub(super) total: usize,
    pub(super) op: ClipOp,
}

#[derive(Clone, Debug)]
pub(super) struct QueuedPaste {
    pub(super) dest_dir: PathBuf,
    pub(super) paths: Vec<PathBuf>,
    pub(super) op: ClipOp,
}

#[derive(Clone, Debug)]
pub(super) struct TrashProgress {
    pub(super) completed: usize,
    pub(super) total: usize,
    pub(super) permanent: bool,
    /// Path of the entry to select after deletion completes: the first
    /// surviving entry at or after the cursor, falling back to the last entry
    /// before the cursor.  Stored as a path (not name) so it takes priority
    /// over the stale remembered-view for this directory.
    pub(super) next_selection: Option<std::path::PathBuf>,
}

#[derive(Clone, Debug)]
pub(super) struct RestoreProgress {
    pub(super) completed: usize,
    pub(super) total: usize,
}

#[derive(Clone, Debug)]
pub(super) struct TrashTarget {
    pub(super) path: std::path::PathBuf,
    pub(super) name: String,
    pub(super) is_dir: bool,
}

#[derive(Clone, Debug)]
pub(super) struct TrashOverlay {
    pub(super) targets: Vec<TrashTarget>,
    pub(super) scroll: usize,
    pub(super) confirmed: bool,
    /// When true the items will be permanently deleted instead of trashed.
    pub(super) permanent: bool,
}

#[derive(Clone, Debug)]
pub(super) struct RestoreOverlay {
    pub(super) targets: Vec<TrashTarget>,
    pub(super) scroll: usize,
    pub(super) confirmed: bool,
}

#[derive(Clone, Debug)]
pub(super) struct RenameOverlay {
    pub(super) is_dir: bool,
    pub(super) original_name: String,
    pub(super) input: String,
    pub(super) cursor_col: usize,
    pub(super) error: Option<String>,
}

pub(super) struct BulkRenameItem {
    pub(super) path: PathBuf,
    pub(super) original_name: String,
    pub(super) is_dir: bool,
}

pub(super) struct BulkRenameOverlay {
    pub(super) items: Vec<BulkRenameItem>,
    /// Editable new name for each item, one-to-one with `items`.
    pub(super) new_names: Vec<String>,
    pub(super) cursor_line: usize,
    pub(super) cursor_col: usize,
    /// Remembered column target for vertical motion.
    pub(super) preferred_col: usize,
    /// Per-line validation error; same length as `items`.
    pub(super) line_errors: Vec<Option<String>>,
}

pub(super) struct CreateOverlay {
    /// One entry per line; always at least one element.
    pub(super) lines: Vec<String>,
    pub(super) cursor_line: usize,
    pub(super) cursor_col: usize,
    /// Remembered column target for vertical motion — updated on horizontal
    /// edits but NOT when vertical motion clamps to a shorter line.
    pub(super) preferred_col: usize,
    /// Per-line validation error; same length as `lines`.
    pub(super) line_errors: Vec<Option<String>>,
}

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
pub(super) struct CopyOverlayRow {
    pub(super) shortcut: char,
    pub(super) label: String,
    pub(super) status_label: String,
    pub(super) value: String,
}

#[derive(Clone, Debug)]
pub(super) struct CopyOverlay {
    pub(super) title: String,
    pub(super) rows: Vec<CopyOverlayRow>,
}

#[derive(Clone, Debug)]
pub(super) enum GoToDestination {
    Top,
    Path(PathBuf),
    Missing(String),
}

#[derive(Clone, Debug)]
pub(super) struct GoToOverlayRow {
    pub(super) shortcut: char,
    pub(super) label: String,
    pub(super) destination: GoToDestination,
}

#[derive(Clone, Debug)]
pub(super) struct GoToOverlay {
    pub(super) title: String,
    pub(super) rows: Vec<GoToOverlayRow>,
}

#[derive(Clone, Debug)]
pub(super) struct SearchCache {
    pub(super) cwd: PathBuf,
    pub(super) scope: SearchScope,
    pub(super) show_hidden: bool,
    pub(super) candidates: Arc<Vec<SearchCandidate>>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedPreview {
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) preview: preview::PreviewContent,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct PreviewCacheKey {
    pub(super) path: PathBuf,
    pub(super) variant: preview::PreviewRequestOptions,
    pub(super) code_line_limit: usize,
    /// The render limit used for this cache entry. Partial (incremental)
    /// renders have `code_render_limit < code_line_limit`; complete renders
    /// have `code_render_limit == code_line_limit`.
    pub(super) code_render_limit: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct PreviewLineCountKey {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
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

#[derive(Clone, Debug, Default)]
pub(super) struct MediaPreviewState {
    pub(super) ffprobe_available: Option<bool>,
    pub(super) ffmpeg_available: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DirectoryCountViewport {
    pub(super) fingerprint: crate::fs::DirectoryFingerprint,
    pub(super) scroll_row: usize,
    pub(super) cols: usize,
    pub(super) rows_visible: usize,
    pub(super) show_hidden: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PreviewLoadState {
    Placeholder(PathBuf),
    Refreshing(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PreviewDirectoryStatsState {
    Loading {
        token: u64,
        path: PathBuf,
    },
    Complete {
        token: u64,
        path: PathBuf,
        stats: crate::fs::DirectoryStats,
    },
    Incomplete {
        token: u64,
        path: PathBuf,
        partial: crate::fs::DirectoryStats,
        error: String,
    },
}

impl PreviewDirectoryStatsState {
    pub(super) fn token(&self) -> u64 {
        match self {
            Self::Loading { token, .. }
            | Self::Complete { token, .. }
            | Self::Incomplete { token, .. } => *token,
        }
    }

    pub(super) fn path(&self) -> &PathBuf {
        match self {
            Self::Loading { path, .. }
            | Self::Complete { path, .. }
            | Self::Incomplete { path, .. } => path,
        }
    }
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
    pub(super) directory_stats: Option<PreviewDirectoryStatsState>,
    pub(super) deferred_refresh_at: Option<Instant>,
    pub(super) prefetch_ready_at: Option<Instant>,
    pub(super) result_cache: HashMap<PreviewCacheKey, CachedPreview>,
    pub(super) result_order: VecDeque<PreviewCacheKey>,
    pub(super) line_count_cache: HashMap<PreviewLineCountKey, usize>,
    pub(super) line_count_order: VecDeque<PreviewLineCountKey>,
    pub(super) pending_line_counts: HashSet<PreviewLineCountKey>,
    /// True while an incremental extension job is outstanding for the current
    /// selection. Prevents duplicate extension submissions.
    pub(super) incremental_render_in_flight: bool,
    /// The path of the entry that triggered the in-flight extension job.
    /// Used to clear `incremental_render_in_flight` when a stale result drops.
    pub(super) incremental_render_path: Option<std::path::PathBuf>,
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

#[derive(Clone, Debug)]
pub(super) struct PendingDirectoryFingerprintScan {
    pub(super) token: u64,
    pub(super) cwd: PathBuf,
    pub(super) show_hidden: bool,
}

pub(super) struct DirectoryRuntime {
    pub(super) fingerprint: crate::fs::DirectoryFingerprint,
    pub(super) watch_tx: std::sync::mpsc::Sender<crate::fs::DirectoryWatchEvent>,
    pub(super) watch_rx: std::sync::mpsc::Receiver<crate::fs::DirectoryWatchEvent>,
    pub(super) watch: Option<crate::fs::DirectoryWatcher>,
    pub(super) pending_reload_at: Option<Instant>,
    pub(super) pending_fingerprint_scan: Option<PendingDirectoryFingerprintScan>,
    pub(super) pending_load: Option<PendingDirectoryLoad>,
    pub(super) use_polling_reload: bool,
    pub(super) last_auto_reload_at: Instant,
}

pub(crate) struct NavigationState {
    pub(crate) cwd: PathBuf,
    pub(crate) entries: Vec<Entry>,
    pub(crate) sidebar: Vec<SidebarRow>,
    pub(crate) selected: usize,
    pub(crate) scroll_row: usize,
    pub(crate) view_mode: ViewMode,
    pub(crate) zoom_level: u8,
    pub(crate) sort_mode: SortMode,
    pub(crate) show_hidden: bool,
    /// True when the loaded directory is the trash folder.
    /// Set in apply_directory_snapshot so it's only true once the load completes.
    pub(in crate::app) in_trash: bool,
    pub(in crate::app) navigation_history: NavigationHistory,
    pub(in crate::app) selected_paths: HashSet<PathBuf>,
    pub(in crate::app) directory_item_count_cache: HashMap<DirectoryItemCountKey, Option<usize>>,
    pub(in crate::app) directory_item_count_order: VecDeque<DirectoryItemCountKey>,
    pub(in crate::app) directory_count_viewport: Option<DirectoryCountViewport>,
    pub(in crate::app) directory_view_memory: HashMap<PathBuf, DirectoryViewMemory>,
    pub(in crate::app) directory_runtime: DirectoryRuntime,
    pub(in crate::app) last_sidebar_refresh_at: Instant,
}

pub(in crate::app) struct PreviewRuntime {
    pub(in crate::app) state: PreviewState,
    pub(in crate::app) comic: comic::ComicPreviewState,
    pub(in crate::app) epub: epub::EpubPreviewState,
    pub(in crate::app) image: images::ImagePreviewState,
    pub(in crate::app) media: MediaPreviewState,
    pub(in crate::app) pdf: pdf::PdfPreviewState,
    pub(in crate::app) terminal_images: inline_image::TerminalImageState,
}

#[derive(Default)]
pub(crate) struct OverlayState {
    pub(in crate::app) trash: Option<TrashOverlay>,
    pub(in crate::app) restore: Option<RestoreOverlay>,
    pub(in crate::app) create: Option<CreateOverlay>,
    pub(in crate::app) rename: Option<RenameOverlay>,
    pub(in crate::app) bulk_rename: Option<BulkRenameOverlay>,
    pub(in crate::app) goto: Option<GoToOverlay>,
    pub(in crate::app) copy: Option<CopyOverlay>,
    pub(in crate::app) search: Option<SearchOverlay>,
    pub(crate) help: bool,
}

pub(in crate::app) struct JobRuntime {
    pub(in crate::app) directory_token: u64,
    pub(in crate::app) directory_fingerprint_token: u64,
    pub(in crate::app) search_token: u64,
    pub(in crate::app) search_loading: bool,
    pub(in crate::app) search_cache: Option<SearchCache>,
    pub(in crate::app) scheduler: JobScheduler,
    pub(in crate::app) clipboard: Option<Clipboard>,
    pub(in crate::app) paste_token: u64,
    pub(in crate::app) paste_progress: Option<PasteProgress>,
    pub(in crate::app) queued_pastes: VecDeque<QueuedPaste>,
    /// Destination directory of the in-flight paste. Kept separately from
    /// `paste_progress` so that cancelling the chip does not lose the context
    /// needed by the completion handler to reload the right directory.
    pub(in crate::app) paste_dest_dir: Option<PathBuf>,
    pub(in crate::app) trash_token: u64,
    pub(in crate::app) trash_progress: Option<TrashProgress>,
    /// Source directory of the in-flight trash. Kept separately from
    /// `trash_progress` for the same reason as `paste_dest_dir`.
    pub(in crate::app) trash_source_cwd: Option<PathBuf>,
    pub(in crate::app) restore_token: u64,
    pub(in crate::app) restore_progress: Option<RestoreProgress>,
    /// Source directory of the in-flight restore. Kept separately from
    /// `restore_progress` so that cancelling the chip does not lose the
    /// context needed by the completion handler.
    pub(in crate::app) restore_source_cwd: Option<PathBuf>,
}

pub(in crate::app) struct InputRuntime {
    pub(in crate::app) frame_state: FrameState,
    pub(in crate::app) last_click: Option<ClickState>,
    pub(in crate::app) wheel_scroll: ScrollState,
    pub(in crate::app) wheel_profile: WheelProfile,
    pub(in crate::app) last_wheel_target: Option<WheelTarget>,
    // Cursor panel tracked exclusively from MouseEventKind::Moved events.
    // These events come from ?1003h (any-event tracking) and always carry the true
    // cursor position, so this is a reliable fallback when scroll event coordinates
    // are wrong or absent (observed in some Alacritty/Ghostty configurations).
    pub(in crate::app) hover_panel: Option<WheelTarget>,
    pub(in crate::app) browser_wheel_post_burst_pending: bool,
    pub(in crate::app) last_navigation_key: Option<(NavigationRepeatKey, Instant)>,
    pub(in crate::app) last_selection_change_at: Instant,
    /// Tracks when keyboard navigation last moved the selection.
    /// Only updated by `move_vertical_keyboard`, `move_by_keyboard`, and `page`
    /// (all keyboard-only paths), not by direct selection or wheel input, so it
    /// does not interfere with wheel auto-focus routing.
    pub(in crate::app) last_key_nav_at: Instant,
}

pub struct App {
    pub(crate) navigation: NavigationState,
    pub(in crate::app) preview: PreviewRuntime,
    pub(crate) overlays: OverlayState,
    pub(in crate::app) jobs: JobRuntime,
    pub(in crate::app) input: InputRuntime,
    pub(in crate::app) status: String,
    pub(crate) should_quit: bool,
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
            navigation: NavigationState {
                cwd,
                entries: Vec::new(),
                sidebar: Vec::new(),
                selected: 0,
                scroll_row: 0,
                view_mode: startup_view_mode(crate::config::ui().start_in_grid),
                zoom_level: crate::config::ui().grid_zoom,
                sort_mode: SortMode::Name,
                show_hidden: crate::config::ui().show_hidden,
                in_trash: false,
                navigation_history: NavigationHistory::default(),
                selected_paths: HashSet::new(),
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
                    pending_fingerprint_scan: None,
                    pending_load: None,
                    use_polling_reload: true,
                    last_auto_reload_at: Instant::now(),
                },
                last_sidebar_refresh_at: Instant::now(),
            },
            preview: PreviewRuntime {
                state: PreviewState {
                    scroll: 0,
                    horizontal_scroll: 0,
                    content: preview::PreviewContent::placeholder("No selection"),
                    token: 0,
                    metrics: PreviewMetrics::default(),
                    load_state: None,
                    directory_stats: None,
                    deferred_refresh_at: None,
                    prefetch_ready_at: None,
                    result_cache: HashMap::new(),
                    result_order: VecDeque::new(),
                    line_count_cache: HashMap::new(),
                    line_count_order: VecDeque::new(),
                    pending_line_counts: HashSet::new(),
                    incremental_render_in_flight: false,
                    incremental_render_path: None,
                },
                comic: comic::ComicPreviewState::default(),
                epub: epub::EpubPreviewState::default(),
                image: images::ImagePreviewState::default(),
                media: MediaPreviewState::default(),
                pdf: pdf::PdfPreviewState::default(),
                terminal_images: inline_image::TerminalImageState::default(),
            },
            overlays: OverlayState::default(),
            jobs: JobRuntime {
                directory_token: 0,
                directory_fingerprint_token: 0,
                search_token: 0,
                search_loading: false,
                search_cache: None,
                scheduler,
                clipboard: None,
                paste_token: 0,
                paste_progress: None,
                queued_pastes: VecDeque::new(),
                paste_dest_dir: None,
                trash_token: 0,
                trash_progress: None,
                trash_source_cwd: None,
                restore_token: 0,
                restore_progress: None,
                restore_source_cwd: None,
            },
            input: InputRuntime {
                frame_state: FrameState::default(),
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
                hover_panel: None,
                browser_wheel_post_burst_pending: false,
                last_navigation_key: None,
                last_selection_change_at: Instant::now(),
                // Initialize to far past so the first keypress is always Immediate.
                last_key_nav_at: Instant::now() - Duration::from_secs(1),
            },
            status: String::new(),
            should_quit: false,
        };
        app.navigation.in_trash = App::path_is_trash(&app.navigation.cwd);
        let snapshot = crate::fs::load_directory_snapshot(
            &app.navigation.cwd,
            app.effective_show_hidden(),
            app.navigation.sort_mode,
        )?;
        app.navigation.sidebar = crate::fs::build_sidebar_rows();
        app.navigation.last_sidebar_refresh_at = Instant::now();
        app.navigation.entries = snapshot.entries;
        app.navigation.directory_runtime.fingerprint = snapshot.fingerprint;
        app.clamp_selection();
        app.remember_current_directory_view();
        app.refresh_preview();
        app.reset_directory_watch();
        Ok(app)
    }

    pub(in crate::app) fn ffprobe_available(&mut self) -> bool {
        *self
            .preview
            .media
            .ffprobe_available
            .get_or_insert_with(|| inline_image::command_exists("ffprobe"))
    }

    pub(in crate::app) fn media_ffmpeg_available(&mut self) -> bool {
        *self
            .preview
            .media
            .ffmpeg_available
            .get_or_insert_with(|| inline_image::command_exists("ffmpeg"))
    }

    #[cfg(test)]
    pub(in crate::app) fn set_media_ffprobe_available_for_tests(&mut self, available: bool) {
        self.preview.media.ffprobe_available = Some(available);
    }

    #[cfg(test)]
    pub(in crate::app) fn set_media_ffmpeg_available_for_tests(&mut self, available: bool) {
        self.preview.media.ffmpeg_available = Some(available);
    }
}

fn startup_view_mode(start_in_grid: bool) -> ViewMode {
    if start_in_grid {
        ViewMode::Grid
    } else {
        ViewMode::List
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_view_mode_defaults_to_list() {
        assert_eq!(startup_view_mode(false), ViewMode::List);
    }

    #[test]
    fn startup_view_mode_can_start_in_grid() {
        assert_eq!(startup_view_mode(true), ViewMode::Grid);
    }
}
