use std::{
    env,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

use super::types::*;
use crate::background_jobs::JobScheduler;
use crate::chooser::ChooserState;
use crate::duplicate_finder::DuplicateFinderState;
use crate::file_browser::FileBrowserState;
#[cfg(unix)]
use crate::file_operations::BulkRenameEditorSession;
use crate::file_operations::FileOperationsState;
use crate::fuzzy_finder::FuzzyFinderState;
use crate::goto_menu::GotoMenu;
use crate::opening::open_with::ApplicationSelection;
use crate::places::PlacesState;
use crate::preview::PreviewRuntime;

#[derive(Clone, Debug)]
pub(crate) struct ClickState {
    pub(crate) path: PathBuf,
    pub(crate) at: Instant,
}

#[derive(Clone, Debug)]
pub(crate) struct ScrollLane {
    pub(crate) pending: isize,
    pub(crate) remainder: isize,
    pub(crate) last_step_at: Option<Instant>,
    pub(crate) last_input_at: Option<Instant>,
    pub(crate) last_input_direction: isize,
    pub(crate) burst_count: u8,
}

impl ScrollLane {
    pub(crate) fn new() -> Self {
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
pub(crate) struct ScrollState {
    pub(crate) horizontal: ScrollLane,
    pub(crate) vertical: ScrollLane,
    pub(crate) preview: ScrollLane,
    pub(crate) preview_horizontal: ScrollLane,
    pub(crate) search: ScrollLane,
}

impl ScrollState {
    pub(crate) const BURST_WINDOW: Duration = Duration::from_millis(150);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WheelTarget {
    Entries,
    Preview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WheelProfile {
    Default,
    HighFrequency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NavigationRepeatKey {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
}

#[derive(Default)]
pub(crate) struct OverlayState {
    pub(crate) goto: Option<GotoMenu>,
    pub(crate) open_with: Option<ApplicationSelection>,
    pub(crate) help: bool,
    pub(crate) help_scroll: usize,
}

pub(crate) struct InputRuntime {
    pub(crate) frame_state: FrameState,
    pub(crate) last_click: Option<ClickState>,
    pub(crate) wheel_scroll: ScrollState,
    pub(crate) wheel_profile: WheelProfile,
    pub(crate) last_wheel_target: Option<WheelTarget>,
    // Cursor panel tracked exclusively from MouseEventKind::Moved events.
    // These events come from ?1003h (any-event tracking) and always carry the true
    // cursor position, so this is a reliable fallback when scroll event coordinates
    // are wrong or absent (observed in some Alacritty/Ghostty configurations).
    pub(crate) hover_panel: Option<WheelTarget>,
    pub(crate) browser_wheel_post_burst_pending: bool,
    pub(crate) last_navigation_key: Option<(NavigationRepeatKey, Instant)>,
    pub(crate) last_selection_change_at: Instant,
    /// Tracks when keyboard navigation last moved the selection.
    /// Only updated by `move_vertical_keyboard`, `move_by_keyboard`, and `page`
    /// (all keyboard-only paths), not by direct selection or wheel input, so it
    /// does not interfere with wheel auto-focus routing.
    pub(crate) last_key_nav_at: Instant,
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

pub struct App {
    pub(crate) file_browser: FileBrowserState,
    pub(crate) places: PlacesState,
    pub(crate) preview: PreviewRuntime,
    pub(crate) file_operations: FileOperationsState,
    pub(crate) fuzzy_finder: FuzzyFinderState,
    pub(crate) duplicate_finder: DuplicateFinderState,
    pub(crate) overlays: OverlayState,
    pub(crate) job_scheduler: JobScheduler,
    pub(crate) input: InputRuntime,
    pub(crate) status: String,
    pub(crate) should_quit: bool,
    pub(crate) should_change_directory_on_quit: bool,
    pub(crate) chooser: ChooserState,
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
        let mut app = Self {
            file_browser: FileBrowserState::new(
                cwd,
                crate::config::ui().start_in_grid,
                crate::config::ui().grid_zoom,
                crate::config::ui().show_hidden || reveal_hidden_start_focus,
            ),
            places: PlacesState::new(),
            preview: PreviewRuntime::new(),
            file_operations: FileOperationsState::default(),
            fuzzy_finder: FuzzyFinderState::default(),
            duplicate_finder: DuplicateFinderState::default(),
            overlays: OverlayState::default(),
            job_scheduler: scheduler,
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
            should_change_directory_on_quit: true,
            chooser: ChooserState::default(),
            pending_terminal_task: None,
        };
        app.file_browser.in_trash = App::path_is_trash(&app.file_browser.cwd);
        let snapshot = crate::filesystem::load_directory_snapshot(
            &app.file_browser.cwd,
            app.effective_show_hidden(),
            app.file_browser.sort_mode,
        )?;
        app.places.refresh();
        app.file_browser.unfiltered_entries = snapshot.entries;
        app.apply_local_filter_preserving_selection();
        app.file_browser.directory_runtime.fingerprint = snapshot.fingerprint;
        if let Some(start_focus) = start_focus
            && let Some(index) = app
                .file_browser
                .entries
                .iter()
                .position(|entry| entry.path == start_focus)
        {
            app.file_browser.selected = index;
        }
        app.clamp_selection();
        app.sync_scroll();
        app.remember_current_directory_view();
        app.refresh_preview();
        app.reset_directory_watch();
        Ok(app)
    }

    pub(crate) fn ffprobe_available(&mut self) -> bool {
        *self.preview.media.ffprobe_available.get_or_insert_with(|| {
            crate::terminal_runtime::terminal_images::command_exists("ffprobe")
        })
    }

    pub(crate) fn media_ffmpeg_available(&mut self) -> bool {
        *self.preview.media.ffmpeg_available.get_or_insert_with(|| {
            crate::terminal_runtime::terminal_images::command_exists("ffmpeg")
        })
    }

    #[cfg(test)]
    pub(crate) fn set_media_ffprobe_available_for_tests(&mut self, available: bool) {
        self.preview.media.ffprobe_available = Some(available);
    }

    #[cfg(test)]
    pub(crate) fn set_media_ffmpeg_available_for_tests(&mut self, available: bool) {
        self.preview.media.ffmpeg_available = Some(available);
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
        assert_eq!(app.file_browser.scroll_row, app.file_browser.selected);
        assert!(app.file_browser.selected_paths.is_empty());
        assert!(app.file_browser.directory_history.back.is_empty());
        assert!(app.file_browser.directory_history.forward.is_empty());
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

        assert!(app.file_browser.show_hidden);
        assert_eq!(
            app.selected_entry().map(|entry| entry.path.as_path()),
            Some(hidden.as_path())
        );
        assert!(
            app.file_browser
                .entries
                .iter()
                .any(|entry| entry.path == hidden)
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
