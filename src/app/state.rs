use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result};

use super::types::*;
use crate::background_jobs::{JobScheduler, job_requests::ArchiveExtractRequest};
use crate::duplicate_finder::{DuplicateGroup, DuplicateScanStats};
#[cfg(unix)]
use crate::file_operations::BulkRenameEditorSession;
use crate::file_operations::{
    ArchiveCreateOverlay, ArchiveCreateProgress, ArchiveExtractProgress, ArchivePasswordOverlay,
    BulkRenameOverlay, Clipboard, CopyOverlay, CreateOverlay, EditorRenameConfirmOverlay,
    PasteProgress, QueuedPaste, RenameOverlay, RestoreOverlay, RestoreProgress, TrashOverlay,
    TrashProgress,
};
use crate::fuzzy_finder::{SearchCandidate, SearchIndexStats};
use crate::preview::PreviewRuntime;
use crate::{
    fs::{Entry, SortMode},
    places::PlaceRow,
};

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
pub(crate) struct DuplicateFinderOverlay {
    pub(crate) cwd: PathBuf,
    pub(crate) groups: Vec<DuplicateGroup>,
    pub(crate) stats: DuplicateScanStats,
    pub(crate) selected: usize,
    pub(crate) scroll: usize,
    pub(crate) selected_paths: HashSet<PathBuf>,
    pub(crate) loading: bool,
    pub(crate) partial: bool,
    pub(crate) error: Option<String>,
    pub(crate) preview_visible: bool,
    pub(crate) preview_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct DuplicateRow {
    pub index: usize,
    pub group_rank: usize,
    pub group_first: bool,
    pub path: PathBuf,
    pub name: String,
    pub parent: String,
    pub size: u64,
    pub selected: bool,
    pub focused: bool,
}

pub(crate) struct SearchOverlay {
    pub(in crate::app) scope: SearchScope,
    pub(in crate::app) query: String,
    pub(in crate::app) query_cursor: usize,
    pub(in crate::app) candidates: Arc<Vec<SearchCandidate>>,
    pub(in crate::app) matches: Vec<usize>,
    pub(in crate::app) cached_matches: HashMap<String, SearchMatchCacheEntry>,
    pub(in crate::app) selected: usize,
    pub(in crate::app) scroll: usize,
    pub(in crate::app) loading: bool,
    pub(in crate::app) error: Option<String>,
    pub(in crate::app) stats: SearchIndexStats,
}

#[derive(Clone, Debug)]
pub(super) struct SearchMatchCacheEntry {
    pub(super) pool: Vec<usize>,
    pub(super) matches: Vec<usize>,
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
pub(crate) struct GoToOverlay {
    pub(in crate::app) title: String,
    pub(in crate::app) rows: Vec<GoToOverlayRow>,
}

#[derive(Clone, Debug)]
pub(super) struct OpenWithRow {
    pub(super) shortcut: Option<char>,
    pub(super) label: String,
    pub(super) app: crate::opening::open_with::OpenWithApplication,
}

#[derive(Clone, Debug)]
pub(crate) struct OpenWithOverlay {
    pub(in crate::app) title: String,
    pub(in crate::app) rows: Vec<OpenWithRow>,
    pub(in crate::app) selected: usize,
}

#[derive(Clone, Debug)]
pub(super) struct SearchCache {
    pub(super) cwd: PathBuf,
    pub(super) scope: SearchScope,
    pub(super) show_hidden: bool,
    pub(super) fingerprint: crate::fs::DirectoryFingerprint,
    pub(super) candidates: Arc<Vec<SearchCandidate>>,
    pub(super) stats: SearchIndexStats,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct DirectoryItemCountKey {
    pub(super) path: PathBuf,
    pub(super) modified: Option<SystemTime>,
    pub(super) show_hidden: bool,
}

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
pub(crate) struct SelectedPaths {
    inner: HashSet<PathBuf>,
    order: Vec<PathBuf>,
    ancestor_counts: HashMap<PathBuf, usize>,
}

impl SelectedPaths {
    pub(in crate::app) fn len(&self) -> usize {
        self.inner.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(crate) fn contains(&self, path: &std::path::Path) -> bool {
        self.inner.contains(path)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &PathBuf> {
        self.inner.iter()
    }

    #[cfg(unix)]
    pub(in crate::app) fn ordered(&self) -> impl Iterator<Item = &PathBuf> {
        self.order.iter()
    }

    pub(crate) fn clear(&mut self) {
        self.inner.clear();
        self.order.clear();
        self.ancestor_counts.clear();
    }

    pub(crate) fn insert(&mut self, path: PathBuf) -> bool {
        if self.has_nesting_conflict(&path) {
            return false;
        }
        if !self.inner.insert(path.clone()) {
            return false;
        }
        self.order.push(path.clone());
        self.add_ancestors(&path);
        true
    }

    pub(in crate::app) fn remove(&mut self, path: &std::path::Path) -> bool {
        if !self.inner.remove(path) {
            return false;
        }
        self.order.retain(|selected| selected != path);
        self.remove_ancestors(path);
        true
    }

    pub(in crate::app) fn has_nesting_conflict(&self, path: &std::path::Path) -> bool {
        self.ancestor_counts.contains_key(path)
            || path
                .ancestors()
                .skip(1)
                .any(|ancestor| self.inner.contains(ancestor))
    }

    fn add_ancestors(&mut self, path: &std::path::Path) {
        for ancestor in path.ancestors().skip(1) {
            *self
                .ancestor_counts
                .entry(ancestor.to_path_buf())
                .or_default() += 1;
        }
    }

    fn remove_ancestors(&mut self, path: &std::path::Path) {
        for ancestor in path.ancestors().skip(1) {
            let Some(count) = self.ancestor_counts.get_mut(ancestor) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.ancestor_counts.remove(ancestor);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DirectoryCountViewport {
    pub(super) fingerprint: crate::fs::DirectoryFingerprint,
    pub(super) scroll_row: usize,
    pub(super) cols: usize,
    pub(super) rows_visible: usize,
    pub(super) show_hidden: bool,
}

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct LocalFilter {
    pub(in crate::app) active: bool,
    pub(in crate::app) query: String,
    pub(in crate::app) cursor: usize,
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
pub(super) struct PendingDirectoryFingerprintScan {
    pub(super) token: u64,
    pub(super) cwd: PathBuf,
    pub(super) show_hidden: bool,
}

pub(crate) struct DirectoryRuntime {
    pub(super) fingerprint: crate::fs::DirectoryFingerprint,
    pub(super) watch_tx: std::sync::mpsc::Sender<crate::fs::DirectoryWatchEvent>,
    pub(super) watch_rx: std::sync::mpsc::Receiver<crate::fs::DirectoryWatchEvent>,
    pub(crate) watch: Option<crate::fs::DirectoryWatcher>,
    pub(super) pending_reload_at: Option<Instant>,
    pub(super) pending_fingerprint_scan: Option<PendingDirectoryFingerprintScan>,
    pub(crate) pending_load: Option<PendingDirectoryLoad>,
    pub(super) use_polling_reload: bool,
    pub(super) last_auto_reload_at: Instant,
}

pub(crate) struct NavigationState {
    pub(crate) cwd: PathBuf,
    pub(crate) entries: Vec<Entry>,
    pub(in crate::app) unfiltered_entries: Vec<Entry>,
    pub(in crate::app) local_filter: LocalFilter,
    pub(crate) sidebar: Vec<PlaceRow>,
    pub(crate) selected: usize,
    pub(crate) scroll_row: usize,
    pub(crate) view_mode: ViewMode,
    pub(crate) zoom_level: u8,
    pub(crate) sort_mode: SortMode,
    pub(crate) show_hidden: bool,
    /// True when the loaded directory is the trash folder.
    /// Set in apply_directory_snapshot so it's only true once the load completes.
    pub(crate) in_trash: bool,
    pub(in crate::app) navigation_history: NavigationHistory,
    pub(crate) selected_paths: SelectedPaths,
    pub(in crate::app) directory_item_count_cache: HashMap<DirectoryItemCountKey, Option<usize>>,
    pub(in crate::app) directory_item_count_order: VecDeque<DirectoryItemCountKey>,
    pub(in crate::app) directory_count_viewport: Option<DirectoryCountViewport>,
    pub(in crate::app) directory_item_count_ready_at: Option<Instant>,
    pub(in crate::app) directory_view_memory: HashMap<PathBuf, DirectoryViewMemory>,
    pub(crate) directory_runtime: DirectoryRuntime,
    pub(in crate::app) last_sidebar_refresh_at: Instant,
}

#[derive(Default)]
pub(crate) struct OverlayState {
    pub(crate) trash: Option<TrashOverlay>,
    pub(crate) restore: Option<RestoreOverlay>,
    pub(crate) archive_create: Option<ArchiveCreateOverlay>,
    pub(crate) archive_password: Option<ArchivePasswordOverlay>,
    pub(crate) create: Option<CreateOverlay>,
    pub(crate) rename: Option<RenameOverlay>,
    pub(crate) bulk_rename: Option<BulkRenameOverlay>,
    pub(crate) editor_rename_confirm: Option<EditorRenameConfirmOverlay>,
    pub(crate) goto: Option<GoToOverlay>,
    pub(crate) copy: Option<CopyOverlay>,
    pub(crate) open_with: Option<OpenWithOverlay>,
    pub(crate) search: Option<SearchOverlay>,
    pub(crate) duplicates: Option<DuplicateFinderOverlay>,
    pub(crate) help: bool,
    pub(crate) help_scroll: usize,
}

pub(crate) struct JobRuntime {
    pub(in crate::app) directory_token: u64,
    pub(in crate::app) directory_fingerprint_token: u64,
    pub(in crate::app) search_token: u64,
    pub(in crate::app) search_loading: bool,
    pub(in crate::app) search_cache: Option<SearchCache>,
    pub(in crate::app) duplicate_token: u64,
    pub(crate) scheduler: JobScheduler,
    pub(crate) clipboard: Option<Clipboard>,
    pub(crate) archive_create_token: u64,
    pub(crate) archive_create_progress: Option<ArchiveCreateProgress>,
    pub(crate) archive_create_source_cwd: Option<PathBuf>,
    pub(crate) archive_create_path: Option<PathBuf>,
    pub(crate) archive_extract_token: u64,
    pub(crate) archive_extract_progress: Option<ArchiveExtractProgress>,
    pub(crate) archive_extract_source_cwd: Option<PathBuf>,
    pub(crate) archive_extract_request: Option<ArchiveExtractRequest>,
    pub(crate) paste_token: u64,
    pub(crate) paste_progress: Option<PasteProgress>,
    pub(crate) queued_pastes: VecDeque<QueuedPaste>,
    /// Destination directory of the in-flight paste. Kept separately from
    /// `paste_progress` so that cancelling the chip does not lose the context
    /// needed by the completion handler to reload the right directory.
    pub(crate) paste_dest_dir: Option<PathBuf>,
    pub(crate) trash_token: u64,
    pub(crate) trash_progress: Option<TrashProgress>,
    /// Source directory of the in-flight trash. Kept separately from
    /// `trash_progress` for the same reason as `paste_dest_dir`.
    pub(crate) trash_source_cwd: Option<PathBuf>,
    pub(crate) restore_token: u64,
    pub(crate) restore_progress: Option<RestoreProgress>,
    /// Source directory of the in-flight restore. Kept separately from
    /// `restore_progress` so that cancelling the chip does not lose the
    /// context needed by the completion handler.
    pub(crate) restore_source_cwd: Option<PathBuf>,
}

pub(crate) struct InputRuntime {
    pub(crate) frame_state: FrameState,
    pub(in crate::app) last_click: Option<ClickState>,
    pub(in crate::app) drag_candidate: Option<PathBuf>,
    pub(in crate::app) drag_paths: Vec<PathBuf>,
    pub(in crate::app) drag_suppressed_until_up: bool,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingTerminalTask {
    Command {
        program: String,
        args: Vec<String>,
    },
    Commands(Vec<(String, Vec<String>)>),
    #[cfg(unix)]
    EditorBulkRename {
        program: String,
        args: Vec<String>,
        session: BulkRenameEditorSession,
    },
    ShellHere {
        cwd: PathBuf,
    },
    Zoxide,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChooserExit {
    Confirmed(Vec<PathBuf>),
    Cancelled,
}

pub(in crate::app) struct GitRuntime {
    pub(in crate::app) token: u64,
    pub(in crate::app) cwd: PathBuf,
    pub(in crate::app) branch: Option<String>,
    pub(in crate::app) dirty: bool,
}

pub struct App {
    pub(crate) navigation: NavigationState,
    pub(in crate::app) preview: PreviewRuntime,
    pub(crate) overlays: OverlayState,
    pub(crate) jobs: JobRuntime,
    pub(crate) input: InputRuntime,
    pub(in crate::app) git: GitRuntime,
    pub(crate) status: String,
    pub(crate) should_quit: bool,
    pub(crate) should_change_directory_on_quit: bool,
    pub(crate) chooser_mode: bool,
    pub(crate) chooser_exit: Option<ChooserExit>,
    /// Set by features that need direct terminal control.  The event loop in
    /// `lib.rs` drains this, suspends the TUI, runs the task, then restores the TUI.
    pub(crate) pending_terminal_task: Option<PendingTerminalTask>,
}

impl App {
    pub fn new() -> Result<Self> {
        let cwd = env::current_dir().context("failed to read current directory")?;
        Self::new_at(cwd)
    }

    pub fn new_at(cwd: PathBuf) -> Result<Self> {
        Self::new_at_startup(cwd, None, false)
    }

    pub(crate) fn new_at_startup(
        cwd: PathBuf,
        start_focus: Option<PathBuf>,
        reveal_hidden_start_focus: bool,
    ) -> Result<Self> {
        let scheduler = JobScheduler::new();
        let (directory_watch_tx, directory_watch_rx) = std::sync::mpsc::channel();
        let mut app = Self {
            navigation: NavigationState {
                cwd,
                entries: Vec::new(),
                unfiltered_entries: Vec::new(),
                local_filter: LocalFilter::default(),
                sidebar: Vec::new(),
                selected: 0,
                scroll_row: 0,
                view_mode: startup_view_mode(crate::config::ui().start_in_grid),
                zoom_level: crate::config::ui().grid_zoom,
                sort_mode: SortMode::Name,
                show_hidden: crate::config::ui().show_hidden || reveal_hidden_start_focus,
                in_trash: false,
                navigation_history: NavigationHistory::default(),
                selected_paths: SelectedPaths::default(),
                directory_item_count_cache: HashMap::new(),
                directory_item_count_order: VecDeque::new(),
                directory_count_viewport: None,
                directory_item_count_ready_at: None,
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
            preview: PreviewRuntime::new(),
            overlays: OverlayState::default(),
            jobs: JobRuntime {
                directory_token: 0,
                directory_fingerprint_token: 0,
                search_token: 0,
                search_loading: false,
                search_cache: None,
                duplicate_token: 0,
                scheduler,
                clipboard: None,
                archive_create_token: 0,
                archive_create_progress: None,
                archive_create_source_cwd: None,
                archive_create_path: None,
                archive_extract_token: 0,
                archive_extract_progress: None,
                archive_extract_source_cwd: None,
                archive_extract_request: None,
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
                drag_candidate: None,
                drag_paths: Vec::new(),
                drag_suppressed_until_up: false,
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
            git: GitRuntime {
                token: 0,
                cwd: PathBuf::new(),
                branch: None,
                dirty: false,
            },
            status: String::new(),
            should_quit: false,
            should_change_directory_on_quit: true,
            chooser_mode: false,
            chooser_exit: None,
            pending_terminal_task: None,
        };
        app.navigation.in_trash = App::path_is_trash(&app.navigation.cwd);
        let snapshot = crate::fs::load_directory_snapshot(
            &app.navigation.cwd,
            app.effective_show_hidden(),
            app.navigation.sort_mode,
        )?;
        app.navigation.sidebar = crate::places::build_place_rows();
        app.navigation.last_sidebar_refresh_at = Instant::now();
        app.navigation.unfiltered_entries = snapshot.entries;
        app.apply_local_filter_preserving_selection();
        app.navigation.directory_runtime.fingerprint = snapshot.fingerprint;
        if let Some(start_focus) = start_focus
            && let Some(index) = app
                .navigation
                .entries
                .iter()
                .position(|entry| entry.path == start_focus)
        {
            app.navigation.selected = index;
        }
        app.clamp_selection();
        app.sync_scroll();
        app.remember_current_directory_view();
        app.refresh_preview();
        app.reset_directory_watch();
        Ok(app)
    }

    pub(in crate::app) fn ffprobe_available(&mut self) -> bool {
        *self.preview.media.ffprobe_available.get_or_insert_with(|| {
            crate::terminal_runtime::terminal_images::command_exists("ffprobe")
        })
    }

    pub(in crate::app) fn media_ffmpeg_available(&mut self) -> bool {
        *self.preview.media.ffmpeg_available.get_or_insert_with(|| {
            crate::terminal_runtime::terminal_images::command_exists("ffmpeg")
        })
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
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-state-{label}-{unique}"))
    }

    #[test]
    fn startup_view_mode_defaults_to_list() {
        assert_eq!(startup_view_mode(false), ViewMode::List);
    }

    #[test]
    fn startup_view_mode_can_start_in_grid() {
        assert_eq!(startup_view_mode(true), ViewMode::Grid);
    }

    #[test]
    fn startup_focus_selects_and_scrolls_entry_without_status_history_or_multi_selection() {
        let root = temp_path("startup-focus");
        fs::create_dir_all(&root).expect("temp directory should be created");
        for index in 0..8 {
            fs::write(root.join(format!("file-{index}.txt")), format!("{index}"))
                .expect("file should be created");
        }
        let target = root.join("file-6.txt");

        let app = App::new_at_startup(root.clone(), Some(target.clone()), false)
            .expect("app should initialize");

        assert_eq!(
            app.selected_entry().map(|entry| entry.path.as_path()),
            Some(target.as_path())
        );
        assert_eq!(app.navigation.scroll_row, app.navigation.selected);
        assert!(app.navigation.selected_paths.is_empty());
        assert!(app.navigation.navigation_history.back.is_empty());
        assert!(app.navigation.navigation_history.forward.is_empty());
        assert_eq!(app.status_message(), "");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn startup_focus_can_reveal_hidden_targets_without_persisted_config() {
        let root = temp_path("startup-hidden-focus");
        fs::create_dir_all(&root).expect("temp directory should be created");
        let visible = root.join("visible.txt");
        let hidden = root.join(".env");
        fs::write(&visible, "visible").expect("visible file should be created");
        fs::write(&hidden, "secret").expect("hidden file should be created");

        let app = App::new_at_startup(root.clone(), Some(hidden.clone()), true)
            .expect("app should initialize");

        assert!(app.navigation.show_hidden);
        assert_eq!(
            app.selected_entry().map(|entry| entry.path.as_path()),
            Some(hidden.as_path())
        );
        assert!(
            app.navigation
                .entries
                .iter()
                .any(|entry| entry.path == hidden)
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
