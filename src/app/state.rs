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
pub(super) struct TrashProgress {
    pub(super) completed: usize,
    pub(super) total: usize,
    pub(super) permanent: bool,
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
    /// True when the loaded directory is the trash folder.
    /// Set in apply_directory_snapshot so it's only true once the load completes.
    pub(super) in_trash: bool,
    pub status: String,
    pub help_open: bool,
    pub should_quit: bool,
    pub(super) navigation_history: NavigationHistory,
    pub(super) preview_state: PreviewState,
    pub(super) comic_preview: comic::ComicPreviewState,
    pub(super) epub_preview: epub::EpubPreviewState,
    pub(super) image_preview: images::ImagePreviewState,
    pub(super) media_preview: MediaPreviewState,
    pub(super) pdf_preview: pdf::PdfPreviewState,
    pub(super) terminal_images: inline_image::TerminalImageState,
    pub(super) frame_state: FrameState,
    pub(super) selected_paths: HashSet<PathBuf>,
    pub(super) clipboard: Option<Clipboard>,
    pub(super) paste_token: u64,
    pub(super) paste_progress: Option<PasteProgress>,
    /// Destination directory of the in-flight paste.  Kept separately from
    /// `paste_progress` so that cancelling the chip does not lose the context
    /// needed by the completion handler to reload the right directory.
    pub(super) paste_dest_dir: Option<PathBuf>,
    pub(super) trash_token: u64,
    pub(super) trash_progress: Option<TrashProgress>,
    /// Source directory of the in-flight trash.  Kept separately from
    /// `trash_progress` for the same reason as `paste_dest_dir`.
    pub(super) trash_source_cwd: Option<PathBuf>,
    pub(super) trash: Option<TrashOverlay>,
    pub(super) restore_token: u64,
    pub(super) restore_progress: Option<RestoreProgress>,
    /// Source directory of the in-flight restore.  Kept separately from
    /// `restore_progress` so that cancelling the chip does not lose the
    /// context needed by the completion handler.
    pub(super) restore_source_cwd: Option<PathBuf>,
    pub(super) restore: Option<RestoreOverlay>,
    pub(super) create: Option<CreateOverlay>,
    pub(super) rename: Option<RenameOverlay>,
    pub(super) bulk_rename: Option<BulkRenameOverlay>,
    pub(super) search: Option<SearchOverlay>,
    pub(super) search_cache: Option<SearchCache>,
    pub(super) search_loading: bool,
    pub(super) search_token: u64,
    pub(super) directory_token: u64,
    pub(super) directory_fingerprint_token: u64,
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
    // Cursor panel tracked exclusively from MouseEventKind::Moved events.
    // These events come from ?1003h (any-event tracking) and always carry the true
    // cursor position, so this is a reliable fallback when scroll event coordinates
    // are wrong or absent (observed in some Alacritty/Ghostty configurations).
    pub(super) hover_panel: Option<WheelTarget>,
    pub(super) browser_wheel_post_burst_pending: bool,
    pub(super) last_navigation_key: Option<(NavigationRepeatKey, Instant)>,
    pub(super) last_selection_change_at: Instant,
    /// Tracks when keyboard navigation last moved the selection.
    /// Only updated by `move_vertical_keyboard`, `move_by_keyboard`, and `page`
    /// (all keyboard-only paths), not by direct selection or wheel input, so it
    /// does not interfere with wheel auto-focus routing.
    pub(super) last_key_nav_at: Instant,
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
            in_trash: false,
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
                prefetch_ready_at: None,
                result_cache: HashMap::new(),
                result_order: VecDeque::new(),
                line_count_cache: HashMap::new(),
                line_count_order: VecDeque::new(),
                pending_line_counts: HashSet::new(),
                incremental_render_in_flight: false,
                incremental_render_path: None,
            },
            comic_preview: comic::ComicPreviewState::default(),
            epub_preview: epub::EpubPreviewState::default(),
            image_preview: images::ImagePreviewState::default(),
            media_preview: MediaPreviewState::default(),
            pdf_preview: pdf::PdfPreviewState::default(),
            terminal_images: inline_image::TerminalImageState::default(),
            frame_state: FrameState::default(),
            selected_paths: HashSet::new(),
            clipboard: None,
            paste_token: 0,
            paste_progress: None,
            paste_dest_dir: None,
            trash_token: 0,
            trash_progress: None,
            trash_source_cwd: None,
            trash: None,
            restore_token: 0,
            restore_progress: None,
            restore_source_cwd: None,
            restore: None,
            create: None,
            rename: None,
            bulk_rename: None,
            search: None,
            search_cache: None,
            search_loading: false,
            search_token: 0,
            directory_token: 0,
            directory_fingerprint_token: 0,
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
                pending_fingerprint_scan: None,
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
            hover_panel: None,
            browser_wheel_post_burst_pending: false,
            last_navigation_key: None,
            last_selection_change_at: Instant::now(),
            // Initialize to far past so the first keypress is always Immediate.
            last_key_nav_at: Instant::now() - Duration::from_secs(1),
        };
        app.in_trash = App::path_is_trash(&app.cwd);
        let snapshot = crate::fs::load_directory_snapshot(
            &app.cwd,
            app.effective_show_hidden(),
            app.sort_mode,
        )?;
        app.sidebar = crate::fs::build_sidebar_items();
        app.entries = snapshot.entries;
        app.directory_runtime.fingerprint = snapshot.fingerprint;
        app.clamp_selection();
        app.remember_current_directory_view();
        app.refresh_preview();
        app.reset_directory_watch();
        Ok(app)
    }

    pub(in crate::app) fn ffprobe_available(&mut self) -> bool {
        *self
            .media_preview
            .ffprobe_available
            .get_or_insert_with(|| inline_image::command_exists("ffprobe"))
    }

    pub(in crate::app) fn media_ffmpeg_available(&mut self) -> bool {
        *self
            .media_preview
            .ffmpeg_available
            .get_or_insert_with(|| inline_image::command_exists("ffmpeg"))
    }

    #[cfg(test)]
    pub(in crate::app) fn set_media_ffprobe_available_for_tests(&mut self, available: bool) {
        self.media_preview.ffprobe_available = Some(available);
    }

    #[cfg(test)]
    pub(in crate::app) fn set_media_ffmpeg_available_for_tests(&mut self, available: bool) {
        self.media_preview.ffmpeg_available = Some(available);
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
