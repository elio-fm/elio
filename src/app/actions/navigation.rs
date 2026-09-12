use super::*;
use crate::app::FileClass;
use crate::app::preview::{HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY, IMAGE_SELECTION_ACTIVATION_DELAY};
use crate::file_classification;
use crate::preview::{PreviewContent, PreviewWorkClass, preview_work_class};

const KEY_NAV_RAPID_THRESHOLD: std::time::Duration = std::time::Duration::from_millis(250);

impl App {
    pub fn selection_summary(&self) -> String {
        if let Some(overlay) = &self.duplicate_finder.session {
            return match self.duplicate_focused_entry() {
                Some(entry) => format!(
                    "{}/{}  {}",
                    overlay.selected.saturating_add(1),
                    self.duplicate_file_count(),
                    entry.name,
                ),
                None => "0/0  Duplicate Finder".to_string(),
            };
        }

        match self.selected_entry() {
            Some(entry) => {
                let suffix = if entry.is_dir() { "/" } else { "" };
                format!(
                    "{}/{}  {}{}",
                    self.file_browser.selected.saturating_add(1),
                    self.file_browser.entries.len(),
                    entry.name,
                    suffix,
                )
            }
            None => format!(
                "0/0  {}",
                crate::filesystem::display_path(&self.file_browser.cwd)
            ),
        }
    }

    pub fn status_message(&self) -> &str {
        &self.status
    }

    pub(crate) fn open_search_with_status(&mut self, scope: SearchScope) {
        if let Err(error) = self.open_fuzzy_finder(scope) {
            self.status = format!("Search unavailable: {error}");
        }
    }

    pub(crate) fn open_zoxide_selection(&mut self, path: PathBuf) {
        let target = if path.is_absolute() {
            path
        } else {
            self.file_browser.cwd.join(path)
        };
        if let Err(error) = self.set_dir(target) {
            self.status = error.to_string();
        }
    }

    pub(crate) fn set_status_message(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    pub(crate) fn toggle_view_mode(&mut self) {
        self.clear_wheel_scroll();
        let view_mode = self.file_browser.toggle_view_mode();
        self.sync_scroll();
        self.status = format!("Switched to {} view", view_mode.label());
    }

    pub(crate) fn cycle_sort_mode(&mut self) -> Result<()> {
        let sort_mode = self.file_browser.cycle_sort_mode();
        self.reload()?;
        self.status = format!("Sort: {}", sort_mode.label());
        Ok(())
    }

    pub(crate) fn toggle_hidden_files(&mut self) -> Result<()> {
        if self.cwd_is_trash() {
            self.status = "Trash shows all files".to_string();
            return Ok(());
        }
        let show_hidden = self.file_browser.toggle_hidden_files();
        self.reload()?;
        self.status = if show_hidden {
            "Hidden files shown".to_string()
        } else {
            "Hidden files hidden".to_string()
        };
        Ok(())
    }

    pub fn can_go_back(&self) -> bool {
        self.file_browser.can_go_back()
    }

    pub fn can_go_forward(&self) -> bool {
        self.file_browser.can_go_forward()
    }

    pub(crate) fn set_selected(&mut self, index: usize) {
        self.set_selected_with_preview_mode(index, PreviewRefreshMode::Immediate);
    }

    fn set_selected_with_preview_mode(&mut self, index: usize, preview_mode: PreviewRefreshMode) {
        let next = self.file_browser.clamped_selection_index(index);
        if next != self.file_browser.selected {
            let preview_mode =
                self.effective_preview_refresh_mode_for_selection(next, preview_mode);
            self.file_browser.set_selected_index(next);
            self.input.last_selection_change_at = Instant::now();
            self.preview.image.selection_activation_delay = match preview_mode {
                PreviewRefreshMode::Immediate => std::time::Duration::ZERO,
                PreviewRefreshMode::Deferred => IMAGE_SELECTION_ACTIVATION_DELAY,
            };
            match preview_mode {
                PreviewRefreshMode::Immediate => self.refresh_preview(),
                PreviewRefreshMode::Deferred => {
                    self.clear_preview_directory_stats();
                    self.preview.state.deferred_refresh_at =
                        Some(Instant::now() + HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY);
                }
            }
        } else {
            self.file_browser.set_selected_index(next);
        }
        self.sync_scroll();
        if matches!(preview_mode, PreviewRefreshMode::Deferred) {
            self.refresh_static_image_preloads();
        }
        self.remember_current_directory_view();
    }

    fn effective_preview_refresh_mode_for_selection(
        &mut self,
        index: usize,
        preview_mode: PreviewRefreshMode,
    ) -> PreviewRefreshMode {
        if preview_mode != PreviewRefreshMode::Immediate {
            return preview_mode;
        }
        let Some(entry) = self.file_browser.entries.get(index).cloned() else {
            return preview_mode;
        };
        let variant = self.preview_request_options_for_entry(&entry);
        let facts = file_classification::inspect_entry_cached(&entry);
        let cold_navigation_heavy_preview =
            matches!(facts.builtin_class, FileClass::Audio | FileClass::Video)
                || matches!(
                    facts.specific_type_label,
                    Some("RAR archive" | "Comic RAR archive")
                );
        let cold_heavy_preview = cold_navigation_heavy_preview
            && preview_work_class(&entry, &variant) == PreviewWorkClass::Heavy
            && self.cached_preview_for(&entry, &variant).is_none();
        let sixel_static_image = self.sixel_static_image_preview_for_entry(&entry);
        let cold_sixel_comic_preview = self.uses_sixel_image_protocol()
            && variant.comic_page_index().is_some()
            && self.cached_preview_for(&entry, &variant).is_none();
        if cold_heavy_preview || sixel_static_image || cold_sixel_comic_preview {
            PreviewRefreshMode::Deferred
        } else {
            PreviewRefreshMode::Immediate
        }
    }

    /// Upgrade `Immediate → Deferred` when the selection changed recently,
    /// indicating rapid keyboard/grid navigation.  The first move in any
    /// sequence stays Immediate so single keypresses feel instant; only
    /// sustained movement defers the preview until motion pauses.
    fn rapid_nav_preview_mode(&self, mode: PreviewRefreshMode) -> PreviewRefreshMode {
        if mode == PreviewRefreshMode::Immediate
            && self.input.last_key_nav_at.elapsed() < KEY_NAV_RAPID_THRESHOLD
        {
            PreviewRefreshMode::Deferred
        } else {
            mode
        }
    }

    pub(crate) fn set_selected_last(&mut self) {
        if !self.file_browser.entries.is_empty() {
            let last = self.file_browser.entries.len() - 1;
            self.set_selected(last);
        }
    }

    pub(crate) fn set_selected_delta(&mut self, delta: isize) {
        self.set_selected_delta_with_preview_mode(delta, PreviewRefreshMode::Immediate);
    }

    fn set_selected_delta_with_preview_mode(
        &mut self,
        delta: isize,
        preview_mode: PreviewRefreshMode,
    ) {
        if self.file_browser.entries.is_empty() {
            self.file_browser.selected = 0;
            self.preview.state.content = PreviewContent::placeholder("No selection");
            self.clear_preview_directory_stats();
            self.preview.state.deferred_refresh_at = None;
            return;
        }

        let next = self
            .file_browser
            .selection_offset(delta)
            .expect("non-empty browser has a selection target");
        self.set_selected_with_preview_mode(next, preview_mode);
    }

    pub(crate) fn page(&mut self, direction: isize) {
        let rows = self.input.screen_regions.metrics.rows_visible.max(1) as isize;
        let mode = self.rapid_nav_preview_mode(PreviewRefreshMode::Immediate);
        let prev = self.file_browser.selected;
        if self.file_browser.view_mode == ViewMode::Grid {
            self.move_grid_vertical_with_preview_mode(direction * rows, mode);
        } else {
            self.set_selected_delta_with_preview_mode(direction * rows, mode);
        }
        if self.file_browser.selected != prev {
            self.input.last_key_nav_at = Instant::now();
        }
    }

    /// Keyboard-only: applies rapid-nav deferred preview for Up/Down/j/k, then moves.
    pub(crate) fn move_vertical_keyboard(&mut self, rows: isize) {
        let mode = self.rapid_nav_preview_mode(PreviewRefreshMode::Immediate);
        let prev = self.file_browser.selected;
        self.move_vertical_with_preview_mode(rows, mode);
        if self.file_browser.selected != prev {
            self.input.last_key_nav_at = Instant::now();
        }
    }

    /// Keyboard-only: applies rapid-nav deferred preview for grid h/l navigation, then moves.
    pub(crate) fn move_by_keyboard(&mut self, delta: isize) {
        let mode = self.rapid_nav_preview_mode(PreviewRefreshMode::Immediate);
        let prev = self.file_browser.selected;
        self.set_selected_delta_with_preview_mode(delta, mode);
        if self.file_browser.selected != prev {
            self.input.last_key_nav_at = Instant::now();
        }
    }

    pub(crate) fn move_vertical(&mut self, rows: isize) {
        self.move_vertical_with_preview_mode(rows, PreviewRefreshMode::Immediate);
    }

    pub(crate) fn move_vertical_with_preview_mode(
        &mut self,
        rows: isize,
        preview_mode: PreviewRefreshMode,
    ) {
        if self.file_browser.view_mode == ViewMode::Grid {
            self.move_grid_vertical_with_preview_mode(rows, preview_mode);
        } else {
            self.set_selected_delta_with_preview_mode(rows, preview_mode);
        }
    }

    pub(crate) fn move_by(&mut self, delta: isize) {
        self.set_selected_delta(delta);
    }

    fn move_grid_vertical_with_preview_mode(
        &mut self,
        rows: isize,
        preview_mode: PreviewRefreshMode,
    ) {
        if self.file_browser.entries.is_empty() {
            self.file_browser.selected = 0;
            return;
        }

        let Some(target_index) = self
            .file_browser
            .grid_selection_offset(rows, self.input.screen_regions.metrics.cols)
        else {
            return;
        };

        self.set_selected_with_preview_mode(target_index, preview_mode);
    }

    pub(crate) fn adjust_zoom(&mut self, delta: i8) {
        if !self.file_browser.adjust_zoom(delta) {
            self.status = format!("Grid zoom limit: {}", self.file_browser.zoom_level);
            return;
        }
        self.status = format!("Grid zoom set to {}", self.file_browser.zoom_level);
        self.sync_scroll();
    }

    pub(crate) fn select_index(&mut self, index: usize) {
        self.set_selected(index);
    }

    pub(crate) fn jump_last(&mut self) {
        self.set_selected_last();
    }

    pub(crate) fn clamp_selection(&mut self) {
        if self.file_browser.clamp_selection() {
            self.preview.state.content = PreviewContent::placeholder("No selection");
            self.clear_preview_directory_stats();
            self.preview.state.scroll = 0;
            self.preview.state.horizontal_scroll = 0;
        }
        self.sync_preview_scroll();
    }

    pub(crate) fn sync_scroll(&mut self) -> bool {
        self.file_browser.sync_scroll(
            self.input.screen_regions.metrics.cols,
            self.input.screen_regions.metrics.rows_visible,
        )
    }

    pub(crate) fn step_sidebar_place(&mut self, delta: isize) -> Result<()> {
        let places = self
            .places
            .rows
            .iter()
            .filter_map(|row| row.item())
            .collect::<Vec<_>>();
        if places.is_empty() {
            return Ok(());
        }

        let current = places
            .iter()
            .position(|item| item.identity_path == self.file_browser.cwd);
        let next = if delta >= 0 {
            current.map(|index| (index + 1) % places.len()).unwrap_or(0)
        } else {
            current
                .map(|index| {
                    if index == 0 {
                        places.len() - 1
                    } else {
                        index - 1
                    }
                })
                .unwrap_or(places.len() - 1)
        };

        self.set_dir(places[next].path.clone())
    }

    pub(crate) fn go_back(&mut self) -> Result<()> {
        let Some(previous) = self.file_browser.directory_history.back.last().cloned() else {
            self.status = "No previous folder".to_string();
            return Ok(());
        };
        self.set_dir_transition(
            previous.cwd,
            DirectoryHistoryMode::GoBack,
            previous
                .selected_path
                .or_else(|| Some(self.file_browser.cwd.clone())),
            DirectoryLoadCompletion::Clear,
        )
    }

    pub(crate) fn go_forward(&mut self) -> Result<()> {
        let Some(next) = self.file_browser.directory_history.forward.last().cloned() else {
            self.status = "No next folder".to_string();
            return Ok(());
        };
        self.set_dir_transition(
            next.cwd,
            DirectoryHistoryMode::GoForward,
            next.selected_path,
            DirectoryLoadCompletion::Clear,
        )
    }

    pub(crate) fn open_entry_at_index(&mut self, index: usize) -> Result<()> {
        let Some(entry) = self.file_browser.entries.get(index).cloned() else {
            return Ok(());
        };

        if entry.is_dir() {
            self.set_dir(entry.path)
        } else {
            self.open_entry_in_system(&entry)
        }
    }

    pub(crate) fn open_in_system(&mut self) -> Result<()> {
        let entries = self.open_target_entries();
        match crate::opening::plans_for_entries(&entries) {
            Ok(plans)
                if plans
                    .iter()
                    .any(|plan| !matches!(plan, crate::opening::OpenPlan::System { .. })) =>
            {
                return self.run_open_plans(plans);
            }
            Err(error) => {
                self.status = error;
                return Ok(());
            }
            _ => {}
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        if let Some(entry) = self.single_file_open_target_entry() {
            if self.queue_terminal_default_open_if_needed(&entry) {
                return Ok(());
            }
            if !crate::opening::open_with::has_applications_for(&entry) {
                if self.queue_editor_fallback_open_if_needed(&entry) {
                    return Ok(());
                }
                self.status = "No app found".to_string();
                return Ok(());
            }
        }

        let targets = self.open_in_system_targets();
        self.open_paths_in_system(targets)
    }

    fn open_entry_in_system(&mut self, entry: &Entry) -> Result<()> {
        match crate::opening::plans_for_entries(std::slice::from_ref(entry)) {
            Ok(plans)
                if plans
                    .iter()
                    .any(|plan| !matches!(plan, crate::opening::OpenPlan::System { .. })) =>
            {
                return self.run_open_plans(plans);
            }
            Err(error) => {
                self.status = error;
                return Ok(());
            }
            _ => {}
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if self.queue_terminal_default_open_if_needed(entry) {
                return Ok(());
            }
            if !crate::opening::open_with::has_applications_for(entry) {
                if self.queue_editor_fallback_open_if_needed(entry) {
                    return Ok(());
                }
                self.status = "No app found".to_string();
                return Ok(());
            }
        }

        self.open_paths_in_system(vec![entry.path.clone()])
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn queue_terminal_default_open_if_needed(&mut self, entry: &Entry) -> bool {
        let Some(app) = crate::opening::open_with::default_application_for(entry)
            .filter(|app| app.requires_terminal)
        else {
            return false;
        };

        self.queue_terminal_open(app)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn queue_editor_fallback_open_if_needed(&mut self, entry: &Entry) -> bool {
        let Some(app) = crate::opening::open_with::editor_fallback_for(entry) else {
            return false;
        };

        self.queue_terminal_open(app)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn queue_terminal_open(&mut self, app: crate::opening::open_with::OpenWithApplication) -> bool {
        self.pending_terminal_task = Some(crate::app::PendingTerminalTask::Command {
            program: app.program,
            args: app.args,
        });
        self.status.clear();
        true
    }

    fn run_open_plans(&mut self, plans: Vec<crate::opening::OpenPlan>) -> Result<()> {
        let mut opened = 0usize;
        let mut total = 0usize;
        let mut last_error = None;
        let mut terminal_commands = Vec::new();
        let mut detached_started = false;

        for plan in plans {
            match plan {
                crate::opening::OpenPlan::System { paths } => {
                    total += paths.len();
                    for path in &paths {
                        match crate::opening::open_in_system(path) {
                            Ok(()) => opened += 1,
                            Err(error) => last_error = Some(error),
                        }
                    }
                }
                crate::opening::OpenPlan::Detached { program, args } => {
                    total += 1;
                    match crate::opening::launch_application(&program, &args) {
                        Ok(()) => {
                            opened += 1;
                            detached_started = true;
                        }
                        Err(error) => last_error = Some(format!("{program}: {error}")),
                    }
                }
                crate::opening::OpenPlan::Terminal { program, args } => {
                    total += 1;
                    opened += 1;
                    terminal_commands.push((program, args));
                }
            }
        }

        if !terminal_commands.is_empty() {
            self.pending_terminal_task = if terminal_commands.len() == 1 {
                let (program, args) = terminal_commands.remove(0);
                Some(crate::app::PendingTerminalTask::Command { program, args })
            } else {
                Some(crate::app::PendingTerminalTask::Commands(terminal_commands))
            };
            self.status.clear();
            return Ok(());
        }

        let started_status = if detached_started {
            "Opening"
        } else {
            "Opened"
        };
        self.status = match (total, opened, last_error) {
            (0, _, _) => String::new(),
            (1, 1, _) => format!("{started_status} item"),
            (_, opened, None) => format!("{started_status} {opened} items"),
            (1, 0, Some(error)) => error,
            (_, 0, Some(error)) => format!("Failed to open {total} items: {error}"),
            (_, opened, Some(error)) => {
                format!("{started_status} {opened}/{total} items; last error: {error}")
            }
        };
        Ok(())
    }

    pub(crate) fn open_paths_in_system(&mut self, targets: Vec<PathBuf>) -> Result<()> {
        if targets.is_empty() {
            return Ok(());
        }

        let total = targets.len();
        let mut opened = 0;
        let mut last_error = None;
        for target in &targets {
            match crate::opening::open_in_system(target) {
                Ok(()) => opened += 1,
                Err(error) => last_error = Some(error),
            }
        }

        self.status = match (total, opened, last_error) {
            (1, 1, _) => format!("Opened {}", open_status_name(&targets[0])),
            (_, opened, None) => format!("Opened {opened} items"),
            (1, 0, Some(error)) => error,
            (_, 0, Some(error)) => format!("Failed to open {total} items: {error}"),
            (_, opened, Some(error)) => {
                format!("Opened {opened}/{total} items; last error: {error}")
            }
        };
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn single_file_open_target_entry(&self) -> Option<Entry> {
        if self.file_browser.selected_paths.len() == 1 {
            let path = self.file_browser.selected_paths.iter().next()?;
            return self
                .file_browser
                .entries
                .iter()
                .find(|entry| &entry.path == path)
                .filter(|entry| !entry.is_dir())
                .cloned()
                .or_else(|| entry_from_existing_file(path));
        }

        if !self.file_browser.selected_paths.is_empty() {
            return None;
        }

        self.selected_entry()
            .filter(|entry| !entry.is_dir())
            .cloned()
    }

    fn open_in_system_targets(&self) -> Vec<PathBuf> {
        if !self.file_browser.selected_paths.is_empty() {
            return self.selected_paths_sorted();
        }

        self.selected_entry()
            .map(|entry| vec![entry.path.clone()])
            .unwrap_or_default()
    }

    fn open_target_entries(&self) -> Vec<Entry> {
        if !self.file_browser.selected_paths.is_empty() {
            return self
                .selected_paths_sorted()
                .into_iter()
                .filter_map(|path| {
                    self.file_browser
                        .entries
                        .iter()
                        .find(|entry| entry.path == path)
                        .cloned()
                        .or_else(|| entry_from_existing_path(&path))
                })
                .collect();
        }

        self.selected_entry().cloned().into_iter().collect()
    }
}

fn open_status_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| crate::filesystem::display_path(path))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn entry_from_existing_file(path: &Path) -> Option<Entry> {
    entry_from_existing_path(path).filter(|entry| !entry.is_dir())
}

fn entry_from_existing_path(path: &Path) -> Option<Entry> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    crate::filesystem::entry_from_path(path.to_path_buf(), name).ok()
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
        std::env::temp_dir().join(format!("elio-navigation-{label}-{unique}"))
    }

    #[cfg(windows)]
    fn successful_detached_command() -> (String, Vec<String>) {
        (
            "cmd.exe".to_string(),
            vec!["/C".to_string(), "exit 0".to_string()],
        )
    }

    #[cfg(not(windows))]
    fn successful_detached_command() -> (String, Vec<String>) {
        (
            "/bin/sh".to_string(),
            vec!["-c".to_string(), "true".to_string()],
        )
    }

    #[test]
    fn detached_open_rule_reports_command_launch_failure() {
        let root = temp_path("detached-open-rule-failure");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let mut app = App::new_at(root.clone()).expect("failed to create app");

        app.run_open_plans(vec![crate::opening::OpenPlan::Detached {
            program: "definitely-not-real-elio-command".to_string(),
            args: Vec::new(),
        }])
        .expect("open plan should be handled");

        assert!(
            app.status_message()
                .contains("definitely-not-real-elio-command"),
            "status should report detached command failure, got: {}",
            app.status_message()
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn detached_open_rule_uses_opening_status_after_successful_launch() {
        let root = temp_path("detached-open-rule-opening");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let mut app = App::new_at(root.clone()).expect("failed to create app");

        let (program, args) = successful_detached_command();
        app.run_open_plans(vec![crate::opening::OpenPlan::Detached { program, args }])
            .expect("open plan should be handled");

        assert_eq!(app.status_message(), "Opening item");

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn cold_rar_preview_navigation_defers_preview_job_until_idle() {
        let root = temp_path("rar-deferred-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("a.txt"), "ready").expect("failed to write text fixture");
        let rar = root.join("b.rar");
        fs::write(&rar, b"not-a-real-rar").expect("failed to write rar fixture");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let before = app.scheduler_metrics();
        let rar_index = app
            .file_browser
            .entries
            .iter()
            .position(|entry| entry.path == rar)
            .expect("rar fixture should be visible");

        app.set_selected(rar_index);
        let after = app.scheduler_metrics();

        assert!(app.preview.state.deferred_refresh_at.is_some());
        assert_eq!(
            after.preview_jobs_submitted_high, before.preview_jobs_submitted_high,
            "cold RAR selection should wait for the deferred refresh instead of immediately queuing heavy preview work"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
