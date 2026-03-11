use super::*;
use anyhow::{Result, anyhow, bail};
use std::path::{Path, PathBuf};

impl App {
    pub fn reload(&mut self) -> Result<()> {
        self.queue_directory_reload(false)
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
        self.preview_token = self.preview_token.wrapping_add(1);
        let remembered_preview = self.remembered_preview_view_for_selected_entry();
        let mut restore_preview_view = true;
        self.preview_cache = match self.selected_entry().cloned() {
            Some(entry) if preview::should_build_preview_in_background(&entry) => {
                if let Some(preview) = self.cached_preview_for(&entry) {
                    self.preview_metrics.cache_hits += 1;
                    self.preview_load_state = None;
                    preview
                } else if let Some(stale_preview) = self.stale_cached_preview_for(&entry) {
                    self.preview_metrics.cache_misses += 1;
                    let loading_path = entry.path.clone();
                    let request = PreviewRequest {
                        token: self.preview_token,
                        entry,
                        priority: PreviewPriority::High,
                    };
                    if !self.scheduler.submit_preview(request) {
                        self.preview_load_state = None;
                        stale_preview.with_status_note("Refresh unavailable")
                    } else {
                        self.preview_load_state = Some(PreviewLoadState::Refreshing(loading_path));
                        stale_preview.with_status_note("Refreshing in background")
                    }
                } else {
                    self.preview_metrics.cache_misses += 1;
                    let placeholder = preview::loading_preview_for(&entry);
                    let loading_path = entry.path.clone();
                    let request = PreviewRequest {
                        token: self.preview_token,
                        entry,
                        priority: PreviewPriority::High,
                    };
                    if !self.scheduler.submit_preview(request) {
                        self.preview_load_state = None;
                        restore_preview_view = false;
                        preview::PreviewContent::placeholder("Preview worker unavailable")
                    } else {
                        self.preview_load_state = Some(PreviewLoadState::Placeholder(loading_path));
                        restore_preview_view = false;
                        placeholder
                    }
                }
            }
            Some(entry) => {
                self.preview_load_state = None;
                preview::build_preview(&entry)
            }
            None => {
                self.preview_load_state = None;
                restore_preview_view = false;
                preview::PreviewContent::placeholder("No selection")
            }
        };
        if restore_preview_view {
            self.preview_scroll = remembered_preview.as_ref().map_or(0, |view| view.scroll);
            self.preview_horizontal_scroll = remembered_preview
                .as_ref()
                .map_or(0, |view| view.horizontal_scroll);
            self.sync_preview_scroll();
            self.remember_current_preview_view();
        } else {
            self.preview_scroll = 0;
            self.preview_horizontal_scroll = 0;
        }
        self.prefetch_nearby_previews();
    }

    fn cached_preview_for(&self, entry: &Entry) -> Option<preview::PreviewContent> {
        let cached = self.preview_result_cache.get(&entry.path)?;
        if cached.size == entry.size && cached.modified == entry.modified {
            Some(cached.preview.clone())
        } else {
            None
        }
    }

    fn stale_cached_preview_for(&self, entry: &Entry) -> Option<preview::PreviewContent> {
        self.preview_result_cache
            .get(&entry.path)
            .map(|cached| cached.preview.clone())
    }

    pub(super) fn cache_preview_result(
        &mut self,
        entry: &Entry,
        preview: &preview::PreviewContent,
    ) {
        self.preview_result_cache.insert(
            entry.path.clone(),
            CachedPreview {
                size: entry.size,
                modified: entry.modified,
                preview: preview.clone(),
            },
        );
        self.preview_result_order.retain(|path| path != &entry.path);
        self.preview_result_order.push_back(entry.path.clone());

        while self.preview_result_order.len() > PREVIEW_CACHE_LIMIT {
            if let Some(stale_path) = self.preview_result_order.pop_front() {
                self.preview_result_cache.remove(&stale_path);
            }
        }
    }

    pub(super) fn queue_directory_load(&mut self, mut load: PendingDirectoryLoad) -> Result<()> {
        self.directory_token = self.directory_token.wrapping_add(1);
        load.token = self.directory_token;
        let request = jobs::DirectoryRequest {
            token: load.token,
            cwd: load.target_cwd.clone(),
            show_hidden: self.show_hidden,
            sort_mode: self.sort_mode,
        };
        if !self.scheduler.submit_directory(request) {
            bail!("Directory worker unavailable");
        }
        self.pending_directory_load = Some(load);
        Ok(())
    }

    fn queue_directory_reload(&mut self, refresh_search: bool) -> Result<()> {
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: self.cwd.clone(),
            previous_cwd: self.cwd.clone(),
            previous_selected_path: self.selected_entry().map(|entry| entry.path.clone()),
            previous_selection_name: self.selected_entry().map(|entry| entry.name.clone()),
            reselect_path: None,
            history_mode: DirectoryHistoryMode::None,
            refresh_search,
            completion: DirectoryLoadCompletion::Keep,
        })
    }

    pub(super) fn apply_directory_snapshot(
        &mut self,
        load: PendingDirectoryLoad,
        snapshot: support::DirectorySnapshot,
    ) {
        let cwd_changed = load.target_cwd != self.cwd;
        let remembered_view = self.remembered_view_for(&load.target_cwd);
        self.cwd = load.target_cwd.clone();
        self.entries = snapshot.entries;
        self.sidebar = support::build_sidebar_items();
        self.directory_fingerprint = snapshot.fingerprint;
        self.last_auto_reload_at = Instant::now();

        self.selected = if let Some(path) = &load.reselect_path {
            self.entries
                .iter()
                .position(|entry| entry.path == *path)
                .unwrap_or(0)
        } else if let Some(path) = remembered_view
            .as_ref()
            .and_then(|view| view.selected_path.as_ref())
        {
            self.entries
                .iter()
                .position(|entry| entry.path == *path)
                .unwrap_or(0)
        } else if let Some(name) = &load.previous_selection_name {
            self.entries
                .iter()
                .position(|entry| entry.name == *name)
                .unwrap_or(0)
        } else {
            0
        };
        self.scroll_row = remembered_view.map_or(0, |view| view.scroll_row);
        self.clamp_selection();
        self.sync_scroll();
        self.remember_current_directory_view();
        self.refresh_preview();
        self.clear_wheel_scroll();

        if cwd_changed {
            self.reset_directory_watch();
        }

        match load.history_mode {
            DirectoryHistoryMode::None => {}
            DirectoryHistoryMode::PushCurrent => {
                self.back_history.push(HistoryEntry {
                    cwd: load.previous_cwd,
                    selected_path: load.previous_selected_path,
                });
                self.forward_history.clear();
            }
            DirectoryHistoryMode::GoBack => {
                if !self.back_history.is_empty() {
                    self.back_history.pop();
                }
                self.forward_history.push(HistoryEntry {
                    cwd: load.previous_cwd,
                    selected_path: load.previous_selected_path,
                });
            }
            DirectoryHistoryMode::GoForward => {
                if !self.forward_history.is_empty() {
                    self.forward_history.pop();
                }
                self.back_history.push(HistoryEntry {
                    cwd: load.previous_cwd,
                    selected_path: load.previous_selected_path,
                });
            }
        }

        if load.refresh_search {
            self.refresh_search_after_directory_reload();
        }

        match load.completion {
            DirectoryLoadCompletion::Keep => {}
            DirectoryLoadCompletion::Clear => self.status.clear(),
            DirectoryLoadCompletion::Status(status) => self.status = status,
        }
    }

    fn prefetch_nearby_previews(&mut self) {
        let mut queued = 0;
        for offset in [1isize, -1, 2, -2, 3, -3] {
            if queued >= PREVIEW_PREFETCH_LIMIT {
                break;
            }

            let target = self.selected as isize + offset;
            if target < 0 {
                continue;
            }
            let Some(entry) = self.entries.get(target as usize).cloned() else {
                continue;
            };
            if !preview::should_build_preview_in_background(&entry)
                || self.cached_preview_for(&entry).is_some()
            {
                continue;
            }

            let request = PreviewRequest {
                token: self.preview_token,
                entry,
                priority: PreviewPriority::Low,
            };
            if self.scheduler.submit_preview(request) {
                queued += 1;
            }
        }
    }

    #[cfg(test)]
    pub(super) fn has_cached_preview_for_path(&self, path: &std::path::Path) -> bool {
        self.preview_result_cache.contains_key(path)
    }

    fn remembered_view_for(&self, cwd: &Path) -> Option<DirectoryViewMemory> {
        self.directory_view_memory.get(cwd).cloned()
    }

    pub(super) fn remembered_preview_view_for(&self, path: &Path) -> Option<PreviewViewMemory> {
        self.preview_view_memory.get(path).cloned()
    }

    fn remembered_preview_view_for_selected_entry(&self) -> Option<PreviewViewMemory> {
        self.selected_entry()
            .and_then(|entry| self.remembered_preview_view_for(&entry.path))
    }

    pub(super) fn remember_current_directory_view(&mut self) {
        self.directory_view_memory.insert(
            self.cwd.clone(),
            DirectoryViewMemory {
                selected_path: self.selected_entry().map(|entry| entry.path.clone()),
                scroll_row: self.scroll_row,
            },
        );
    }

    pub(super) fn remember_current_preview_view(&mut self) {
        let Some(path) = self.selected_entry().map(|entry| entry.path.clone()) else {
            return;
        };
        if self.preview_load_state.as_ref() == Some(&PreviewLoadState::Placeholder(path.clone())) {
            return;
        }

        self.preview_view_memory.insert(
            path.clone(),
            PreviewViewMemory {
                scroll: self.preview_scroll,
                horizontal_scroll: self.preview_horizontal_scroll,
            },
        );
        self.preview_view_order
            .retain(|queued_path| queued_path != &path);
        self.preview_view_order.push_back(path);

        while self.preview_view_order.len() > PREVIEW_VIEW_MEMORY_LIMIT {
            if let Some(stale_path) = self.preview_view_order.pop_front() {
                self.preview_view_memory.remove(&stale_path);
            }
        }
    }

    pub fn selection_summary(&self) -> String {
        match self.selected_entry() {
            Some(entry) => {
                let suffix = if entry.is_dir() { "/" } else { "" };
                format!(
                    "{}/{}  {}{}",
                    self.selected.saturating_add(1),
                    self.entries.len(),
                    entry.name,
                    suffix,
                )
            }
            None => format!("0/0  {}", self.cwd.display()),
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
            self.remember_current_preview_view();
        }
        if next != self.selected {
            self.selected = next;
            self.refresh_preview();
        } else {
            self.selected = next;
        }
        self.sync_scroll();
        self.remember_current_directory_view();
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
            self.preview_horizontal_scroll = 0;
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
        self.set_dir_transition(
            path,
            DirectoryHistoryMode::PushCurrent,
            None,
            DirectoryLoadCompletion::Clear,
        )
    }

    pub(super) fn set_dir_transition(
        &mut self,
        path: PathBuf,
        history_mode: DirectoryHistoryMode,
        reselect_path: Option<PathBuf>,
        completion: DirectoryLoadCompletion,
    ) -> Result<()> {
        let metadata = std::fs::metadata(&path).map_err(|error| {
            anyhow!(
                "Cannot open {}: {}",
                path.display(),
                support::describe_io_error(&error)
            )
        })?;
        if !metadata.is_dir() {
            bail!("{} is not a directory", path.display());
        }
        let normalized = path.canonicalize().unwrap_or(path);
        if normalized == self.cwd && self.pending_directory_load.is_none() {
            self.status = format!("Already in {}", self.cwd.display());
            return Ok(());
        }
        if self
            .pending_directory_load
            .as_ref()
            .is_some_and(|load| load.target_cwd == normalized)
        {
            self.status = format!("Already opening {}", normalized.display());
            return Ok(());
        }

        let reselect_path = reselect_path.or_else(|| {
            self.remembered_view_for(&normalized)
                .and_then(|view| view.selected_path)
        });
        self.remember_current_preview_view();
        self.status = format!("Opening {}", normalized.display());
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: normalized,
            previous_cwd: self.cwd.clone(),
            previous_selected_path: self.selected_entry().map(|entry| entry.path.clone()),
            previous_selection_name: None,
            reselect_path,
            history_mode,
            refresh_search: false,
            completion,
        })
    }

    pub(super) fn go_parent(&mut self) -> Result<()> {
        let current = self.cwd.clone();
        let Some(parent) = self.cwd.parent() else {
            self.status = "Already at filesystem root".to_string();
            return Ok(());
        };
        self.set_dir_transition(
            parent.to_path_buf(),
            DirectoryHistoryMode::PushCurrent,
            Some(current),
            DirectoryLoadCompletion::Clear,
        )
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
        let Some(previous) = self.back_history.last().cloned() else {
            self.status = "No previous folder".to_string();
            return Ok(());
        };
        self.set_dir_transition(
            previous.cwd,
            DirectoryHistoryMode::GoBack,
            previous.selected_path.or_else(|| Some(self.cwd.clone())),
            DirectoryLoadCompletion::Clear,
        )
    }

    pub(super) fn go_forward(&mut self) -> Result<()> {
        let Some(next) = self.forward_history.last().cloned() else {
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
        if self.pending_directory_load.is_some() {
            return Ok(false);
        }
        let fingerprint = match support::scan_directory_fingerprint(&self.cwd, self.show_hidden) {
            Ok(fingerprint) => fingerprint,
            Err(_) => return Ok(false),
        };
        if fingerprint == self.directory_fingerprint {
            return Ok(false);
        }

        self.queue_directory_reload(true)?;
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

    fn wait_for_directory_load(app: &mut App) {
        for _ in 0..100 {
            let _ = app.process_background_jobs();
            if app.pending_directory_load.is_none() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for directory load");
    }

    #[test]
    fn watcher_reload_detects_new_visible_entries() {
        let root = temp_path("auto-reload-visible");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.directory_watch = None;
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
        wait_for_directory_load(&mut app);
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
        app.directory_watch = None;
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
        wait_for_directory_load(&mut app);
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
        app.directory_watch = None;
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
        wait_for_directory_load(&mut app);
        assert_eq!(app.entries.len(), 2);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn selection_summary_is_compact_for_files() {
        let root = temp_path("selection-summary-file");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("note.txt"), "hello").expect("failed to write file");

        let app = App::new_at(root.clone()).expect("failed to create app");
        assert_eq!(app.selection_summary(), "1/1  note.txt");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn selection_summary_marks_directories_with_trailing_slash() {
        let root = temp_path("selection-summary-dir");
        let child = root.join("child");
        fs::create_dir_all(&child).expect("failed to create temp dirs");

        let app = App::new_at(root.clone()).expect("failed to create app");
        assert_eq!(app.selection_summary(), "1/1  child/");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn set_dir_failure_keeps_previous_directory_state() {
        let root = temp_path("set-dir-missing");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("note.txt"), "hello").expect("failed to write file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let missing = root.join("missing");

        assert!(app.set_dir(missing).is_err());
        assert_eq!(app.cwd, root);
        assert_eq!(app.entries.len(), 1);
        assert!(app.back_history.is_empty());
        assert!(app.forward_history.is_empty());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn go_back_failure_preserves_history() {
        let root = temp_path("history-missing");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let missing = root.join("missing");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.back_history.push(HistoryEntry {
            cwd: missing.clone(),
            selected_path: None,
        });

        assert!(app.go_back().is_err());
        assert_eq!(app.cwd, root);
        assert_eq!(
            app.back_history,
            vec![HistoryEntry {
                cwd: missing,
                selected_path: None,
            }]
        );
        assert!(app.forward_history.is_empty());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn reload_restores_latest_remembered_view_state() {
        let root = temp_path("reload-latest-view-state");
        fs::create_dir_all(&root).expect("failed to create temp root");
        for index in 0..8 {
            fs::write(root.join(format!("file-{index}.txt")), format!("{index}"))
                .expect("failed to write file");
        }

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.view_mode = ViewMode::List;
        app.set_frame_state(FrameState {
            metrics: ViewMetrics {
                cols: 1,
                rows_visible: 3,
            },
            ..FrameState::default()
        });

        app.reload().expect("reload should queue successfully");
        app.select_index(6);
        wait_for_directory_load(&mut app);

        assert_eq!(app.selected, 6);
        assert_eq!(app.scroll_row, 4);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
