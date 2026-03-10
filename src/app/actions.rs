use super::*;
use anyhow::{Result, bail};
use std::path::PathBuf;

impl App {
    pub fn reload(&mut self) -> Result<()> {
        let previous_name = self.selected_entry().map(|entry| entry.name.clone());
        self.entries = support::read_entries(&self.cwd, self.show_hidden)?;
        support::sort_entries(&mut self.entries, self.sort_mode);
        self.sidebar = support::build_sidebar_items();
        self.directory_fingerprint = support::entries_fingerprint(&self.entries);
        self.last_auto_reload_at = Instant::now();

        self.selected = match previous_name {
            Some(name) => self
                .entries
                .iter()
                .position(|entry| entry.name == name)
                .unwrap_or(0),
            None => 0,
        };
        self.clamp_selection();
        self.sync_scroll();
        self.refresh_preview();
        self.clear_wheel_scroll();
        Ok(())
    }

    pub fn process_auto_reload(&mut self) -> Result<bool> {
        while let Ok(event) = self.directory_watch_rx.try_recv() {
            match event {
                watching::DirectoryWatchEvent::Changed(paths)
                    if !watching::event_affects_visible_entries(&paths, self.show_hidden) => {}
                _ => {
                    self.pending_directory_reload_at =
                        Some(Instant::now() + watching::directory_watch_debounce());
                }
            }
        }

        if let Some(deadline) = self.pending_directory_reload_at {
            if Instant::now() < deadline {
                return Ok(false);
            }
            self.pending_directory_reload_at = None;
            return self.reload_if_directory_changed();
        }

        if !self.use_polling_reload {
            return Ok(false);
        }

        if self.last_auto_reload_at.elapsed() < AUTO_RELOAD_INTERVAL {
            return Ok(false);
        }
        self.last_auto_reload_at = Instant::now();
        self.reload_if_directory_changed()
    }

    pub fn preview_lines(&self) -> Vec<Line<'static>> {
        self.preview_cache.lines()
    }

    pub fn preview_section_label(&self) -> &'static str {
        self.preview_cache.section_label()
    }

    pub fn preview_scroll_offset(&self) -> usize {
        self.preview_scroll
    }

    pub fn preview_horizontal_scroll_offset(&self) -> usize {
        self.preview_horizontal_scroll
    }

    pub fn preview_total_lines(&self, visible_cols: usize) -> usize {
        self.preview_cache.visual_line_count(visible_cols)
    }

    pub fn preview_wraps(&self) -> bool {
        self.preview_cache.kind.wraps_in_preview()
    }

    pub fn preview_allows_horizontal_scroll(&self) -> bool {
        self.preview_cache.kind.allows_horizontal_scroll()
    }

    pub fn preview_max_horizontal_scroll(&self, visible_cols: usize) -> usize {
        if !self.preview_allows_horizontal_scroll() {
            return 0;
        }
        self.preview_cache
            .max_line_width()
            .saturating_sub(visible_cols.max(1))
    }

    pub fn preview_directory_counts(&self) -> Option<(usize, usize, usize)> {
        Some((
            self.preview_cache.item_count?,
            self.preview_cache.folder_count?,
            self.preview_cache.file_count?,
        ))
    }

    pub fn preview_header_detail(&self, visible_rows: usize) -> Option<String> {
        self.preview_cache
            .header_detail(self.preview_scroll, visible_rows)
    }

    pub(super) fn refresh_preview(&mut self) {
        self.preview_cache = match self.selected_entry() {
            Some(entry) => preview::build_preview(entry),
            None => preview::PreviewContent::placeholder("No selection"),
        };
        self.preview_scroll = 0;
        self.preview_horizontal_scroll = 0;
    }

    pub fn selection_summary(&self) -> String {
        match self.selected_entry() {
            Some(entry) => format!(
                "{} of {} selected  •  {}  •  {}",
                self.selected.saturating_add(1),
                self.entries.len(),
                entry.kind_label(),
                entry.name,
            ),
            None => format!("0 items  •  {}", self.cwd.display()),
        }
    }

    pub fn status_message(&self) -> &str {
        &self.status
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_history.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_history.is_empty()
    }

    pub(super) fn set_selected(&mut self, index: usize) {
        let next = index.min(self.entries.len().saturating_sub(1));
        if next != self.selected {
            self.selected = next;
            self.refresh_preview();
        } else {
            self.selected = next;
        }
        self.sync_scroll();
    }

    pub(super) fn set_selected_last(&mut self) {
        if !self.entries.is_empty() {
            let last = self.entries.len() - 1;
            self.set_selected(last);
        }
    }

    pub(super) fn set_selected_delta(&mut self, delta: isize) {
        if self.entries.is_empty() {
            self.selected = 0;
            self.preview_cache = preview::PreviewContent::placeholder("No selection");
            return;
        }

        let max_index = self.entries.len().saturating_sub(1) as isize;
        let next = (self.selected as isize + delta).clamp(0, max_index) as usize;
        self.set_selected(next);
    }

    pub(super) fn page(&mut self, direction: isize) {
        let rows = self.frame_state.metrics.rows_visible.max(1) as isize;
        if self.view_mode == ViewMode::Grid {
            self.move_grid_vertical(direction * rows);
        } else {
            self.set_selected_delta(direction * rows);
        }
    }

    pub(super) fn move_vertical(&mut self, rows: isize) {
        if self.view_mode == ViewMode::Grid {
            self.move_grid_vertical(rows);
        } else {
            self.set_selected_delta(rows);
        }
    }

    pub(super) fn move_by(&mut self, delta: isize) {
        self.set_selected_delta(delta);
    }

    pub(super) fn move_grid_vertical(&mut self, rows: isize) {
        if self.entries.is_empty() {
            self.selected = 0;
            return;
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let current_row = self.selected / cols;
        let current_col = self.selected % cols;
        let total_rows = self.entries.len().div_ceil(cols);
        let target_row = current_row as isize + rows;

        if target_row < 0 || target_row >= total_rows as isize {
            return;
        }

        let target_index = target_row as usize * cols + current_col;
        if target_index >= self.entries.len() {
            return;
        }

        self.set_selected(target_index);
    }

    pub(super) fn adjust_zoom(&mut self, delta: i8) {
        let next = (self.zoom_level as i8 + delta).clamp(0, 2) as u8;
        if next == self.zoom_level {
            self.status = format!("Directory zoom limit: {}", self.zoom_level);
            return;
        }
        self.zoom_level = next;
        self.status = format!("Directory zoom set to {}", self.zoom_level);
        self.sync_scroll();
    }

    pub(super) fn reset_zoom(&mut self) {
        self.zoom_level = 1;
        self.status = format!("Directory zoom reset to {}", self.zoom_level);
        self.sync_scroll();
    }

    pub(super) fn select_index(&mut self, index: usize) {
        self.set_selected(index);
    }

    pub(super) fn select_last(&mut self) {
        self.set_selected_last();
    }

    pub(super) fn clamp_selection(&mut self) {
        if self.entries.is_empty() {
            self.selected = 0;
            self.scroll_row = 0;
            self.preview_cache = preview::PreviewContent::placeholder("No selection");
            self.preview_scroll = 0;
        } else if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
        self.sync_preview_scroll();
    }

    pub(super) fn sync_scroll(&mut self) -> bool {
        let previous = self.scroll_row;
        if self.entries.is_empty() {
            self.scroll_row = 0;
            return previous != self.scroll_row;
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let rows_visible = self.frame_state.metrics.rows_visible.max(1);
        let selected_row = self.selected / cols;
        if selected_row < self.scroll_row {
            self.scroll_row = selected_row;
        } else if selected_row >= self.scroll_row + rows_visible {
            self.scroll_row = selected_row + 1 - rows_visible;
        }
        self.scroll_row = self.scroll_row.min(self.max_scroll_row());
        previous != self.scroll_row
    }

    fn max_scroll_row(&self) -> usize {
        if self.entries.is_empty() {
            return 0;
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let rows_visible = self.frame_state.metrics.rows_visible.max(1);
        let total_rows = self.entries.len().div_ceil(cols);
        total_rows.saturating_sub(rows_visible)
    }

    pub(super) fn set_dir(&mut self, path: PathBuf) -> Result<()> {
        self.set_dir_with_history(path, true)
    }

    pub(super) fn set_dir_with_history(
        &mut self,
        path: PathBuf,
        record_history: bool,
    ) -> Result<()> {
        if !path.is_dir() {
            bail!("{} is not a directory", path.display());
        }
        let normalized = path.canonicalize().unwrap_or(path);
        if normalized == self.cwd {
            self.status = format!("Already in {}", self.cwd.display());
            return Ok(());
        }
        if record_history {
            self.back_history.push(self.cwd.clone());
            self.forward_history.clear();
        }
        self.cwd = normalized;
        self.selected = 0;
        self.scroll_row = 0;
        self.reload()?;
        self.reset_directory_watch();
        self.status.clear();
        Ok(())
    }

    pub(super) fn go_parent(&mut self) -> Result<()> {
        let Some(parent) = self.cwd.parent() else {
            self.status = "Already at filesystem root".to_string();
            return Ok(());
        };
        self.set_dir(parent.to_path_buf())
    }

    pub(super) fn step_pinned_place(&mut self, delta: isize) -> Result<()> {
        if self.sidebar.is_empty() {
            return Ok(());
        }

        let current = self.sidebar.iter().position(|item| item.path == self.cwd);
        let next = if delta >= 0 {
            current
                .map(|index| (index + 1) % self.sidebar.len())
                .unwrap_or(0)
        } else {
            current
                .map(|index| {
                    if index == 0 {
                        self.sidebar.len() - 1
                    } else {
                        index - 1
                    }
                })
                .unwrap_or(self.sidebar.len() - 1)
        };

        self.set_dir(self.sidebar[next].path.clone())
    }

    pub(super) fn go_back(&mut self) -> Result<()> {
        let Some(previous) = self.back_history.pop() else {
            self.status = "No previous folder".to_string();
            return Ok(());
        };
        self.forward_history.push(self.cwd.clone());
        self.set_dir_with_history(previous, false)
    }

    pub(super) fn go_forward(&mut self) -> Result<()> {
        let Some(next) = self.forward_history.pop() else {
            self.status = "No next folder".to_string();
            return Ok(());
        };
        self.back_history.push(self.cwd.clone());
        self.set_dir_with_history(next, false)
    }

    pub(super) fn open_in_system(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry() else {
            return Ok(());
        };

        let target = entry.path.clone();
        for (program, args) in [("gio", vec!["open"]), ("xdg-open", Vec::new())] {
            match support::detached_open(program, &args, &target) {
                Ok(()) => {
                    self.status = format!("Opened {}", target.display());
                    return Ok(());
                }
                Err(error) => {
                    self.status = format!(
                        "Failed to open {} with {}: {}",
                        target.display(),
                        program,
                        error
                    );
                }
            }
        }
        Ok(())
    }

    pub(super) fn reset_directory_watch(&mut self) {
        self.directory_watch = None;
        self.pending_directory_reload_at = None;
        while self.directory_watch_rx.try_recv().is_ok() {}

        match watching::start_directory_watcher(&self.cwd, &self.directory_watch_tx) {
            Ok(watcher) => {
                self.directory_watch = Some(watcher);
                self.use_polling_reload = false;
            }
            Err(_) => {
                self.use_polling_reload = true;
            }
        }
    }

    fn reload_if_directory_changed(&mut self) -> Result<bool> {
        let fingerprint = match support::scan_directory_fingerprint(&self.cwd, self.show_hidden) {
            Ok(fingerprint) => fingerprint,
            Err(_) => return Ok(false),
        };
        if fingerprint == self.directory_fingerprint {
            return Ok(false);
        }

        self.reload()?;
        self.refresh_search_after_directory_reload();
        Ok(true)
    }

    fn refresh_search_after_directory_reload(&mut self) {
        let Some(scope) = self.search.as_ref().map(|search| search.scope) else {
            return;
        };

        if let Some(search) = &mut self.search {
            search.candidates = Arc::new(Vec::new());
            search.matches.clear();
            search.cached_matches = HashMap::from([(String::new(), Vec::new())]);
            search.selected = 0;
            search.scroll = 0;
            search.loading = true;
            search.error = None;
        }
        self.prewarm_search_index(scope);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-actions-{label}-{unique}"))
    }

    fn make_auto_reload_ready(app: &mut App) {
        app.last_auto_reload_at = Instant::now() - AUTO_RELOAD_INTERVAL - Duration::from_millis(1);
    }

    #[test]
    fn watcher_reload_detects_new_visible_entries() {
        let root = temp_path("auto-reload-visible");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        assert_eq!(app.entries.len(), 1);

        let second = root.join("two.txt");
        fs::write(&second, "world").expect("failed to write second file");
        app.directory_watch_tx
            .send(watching::DirectoryWatchEvent::Changed(vec![second]))
            .expect("failed to queue watch event");

        assert!(
            !app.process_auto_reload()
                .expect("watch processing should succeed"),
            "watch processing should debounce before reloading",
        );
        app.pending_directory_reload_at = Some(Instant::now() - Duration::from_millis(1));

        assert!(
            app.process_auto_reload()
                .expect("auto reload should succeed"),
            "watch-driven reload should report a change",
        );
        assert_eq!(app.entries.len(), 2);
        assert!(app.entries.iter().any(|entry| entry.name == "two.txt"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn watcher_rescan_event_triggers_reload() {
        let root = temp_path("auto-reload-rescan");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        assert_eq!(app.entries.len(), 1);

        fs::write(root.join("two.txt"), "world").expect("failed to write second file");
        app.directory_watch_tx
            .send(watching::DirectoryWatchEvent::Rescan)
            .expect("failed to queue rescan event");

        assert!(
            !app.process_auto_reload()
                .expect("watch processing should succeed"),
            "watch processing should debounce before reloading",
        );
        app.pending_directory_reload_at = Some(Instant::now() - Duration::from_millis(1));

        assert!(
            app.process_auto_reload()
                .expect("auto reload should succeed"),
            "rescan-driven reload should report a change",
        );
        assert_eq!(app.entries.len(), 2);
        assert!(app.entries.iter().any(|entry| entry.name == "two.txt"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn watcher_reload_ignores_hidden_entries_when_hidden_files_are_off() {
        let root = temp_path("auto-reload-hidden");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("visible.txt"), "hello").expect("failed to write visible file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        assert!(!app.show_hidden);
        assert_eq!(app.entries.len(), 1);

        let hidden = root.join(".secret");
        fs::write(&hidden, "hidden").expect("failed to write hidden file");
        app.directory_watch_tx
            .send(watching::DirectoryWatchEvent::Changed(vec![hidden]))
            .expect("failed to queue watch event");

        assert!(
            !app.process_auto_reload()
                .expect("watch processing should succeed"),
            "hidden-only changes should not trigger a reload schedule",
        );
        assert!(app.pending_directory_reload_at.is_none());
        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].name, "visible.txt");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn polling_fallback_respects_its_throttle_window() {
        let root = temp_path("auto-reload-throttle");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.directory_watch = None;
        app.use_polling_reload = true;
        fs::write(root.join("two.txt"), "world").expect("failed to write second file");

        assert!(
            !app.process_auto_reload()
                .expect("auto reload should succeed"),
            "reload should stay idle inside the throttle window",
        );
        assert_eq!(app.entries.len(), 1);

        make_auto_reload_ready(&mut app);
        assert!(
            app.process_auto_reload()
                .expect("auto reload should succeed"),
            "reload should run once the throttle window has elapsed",
        );
        assert_eq!(app.entries.len(), 2);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
