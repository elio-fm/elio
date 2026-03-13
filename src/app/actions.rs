use super::*;
use anyhow::{Result, anyhow, bail};
use std::path::{Path, PathBuf};
use std::time::Instant;

impl App {
    pub fn reload(&mut self) -> Result<()> {
        self.queue_directory_reload(false)
    }

    pub fn process_auto_reload(&mut self) -> Result<bool> {
        while let Ok(event) = self.directory_runtime.watch_rx.try_recv() {
            match event {
                watching::DirectoryWatchEvent::Changed(paths)
                    if !watching::event_affects_visible_entries(&paths, self.show_hidden) => {}
                _ => {
                    self.directory_runtime.pending_reload_at =
                        Some(Instant::now() + watching::directory_watch_debounce());
                }
            }
        }

        if let Some(deadline) = self.directory_runtime.pending_reload_at {
            if Instant::now() < deadline {
                return Ok(false);
            }
            self.directory_runtime.pending_reload_at = None;
            return self.reload_if_directory_changed();
        }

        if !self.directory_runtime.use_polling_reload {
            return Ok(false);
        }

        if self.directory_runtime.last_auto_reload_at.elapsed() < AUTO_RELOAD_INTERVAL {
            return Ok(false);
        }
        self.directory_runtime.last_auto_reload_at = Instant::now();
        self.reload_if_directory_changed()
    }

    pub fn preview_lines(&self) -> Vec<Line<'static>> {
        self.preview_state.content.lines()
    }

    pub fn preview_section_label(&self) -> &'static str {
        self.preview_state.content.section_label()
    }

    pub fn preview_scroll_offset(&self) -> usize {
        self.preview_state.scroll
    }

    pub fn preview_horizontal_scroll_offset(&self) -> usize {
        self.preview_state.horizontal_scroll
    }

    pub fn preview_total_lines(&self, visible_cols: usize) -> usize {
        self.preview_state.content.visual_line_count(visible_cols)
    }

    pub fn preview_wraps(&self) -> bool {
        self.preview_state.content.kind.wraps_in_preview()
    }

    pub fn preview_allows_horizontal_scroll(&self) -> bool {
        self.preview_state.content.kind.allows_horizontal_scroll()
    }

    pub fn preview_max_horizontal_scroll(&self, visible_cols: usize) -> usize {
        if !self.preview_allows_horizontal_scroll() {
            return 0;
        }
        self.preview_state
            .content
            .max_line_width()
            .saturating_sub(visible_cols.max(1))
    }

    pub fn preview_header_detail(&self, visible_rows: usize) -> Option<String> {
        let detail = self
            .preview_state
            .content
            .header_detail(self.preview_state.scroll, visible_rows);
        if let Some(pdf_detail) = self.pdf_preview_header_detail() {
            return Some(match detail {
                Some(detail) if !detail.is_empty() => format!("{detail}  •  {pdf_detail}"),
                _ => pdf_detail,
            });
        }
        if let Some(image_detail) = self.static_image_preview_header_detail() {
            return Some(match detail {
                Some(detail) if !detail.is_empty() => format!("{detail}  •  {image_detail}"),
                _ => image_detail,
            });
        }
        detail
    }

    pub(super) fn refresh_preview(&mut self) {
        self.preview_state.deferred_refresh_at = None;
        self.sync_pdf_preview_selection();
        self.sync_image_preview_selection_activation();
        self.preview_state.token = self.preview_state.token.wrapping_add(1);
        self.preview_state.content = match self.selected_entry().cloned() {
            Some(entry) if self.should_defer_static_image_preview(&entry) => {
                self.preview_state.load_state = None;
                preview::PreviewContent::new(preview::PreviewKind::Image, Vec::new()).with_detail(
                    self.static_image_preview_detail(&entry)
                        .unwrap_or("Image preview"),
                )
            }
            Some(entry) if self.should_defer_pdf_document_preview(&entry) => {
                self.preview_state.load_state = None;
                self.cached_preview_for(&entry)
                    .or_else(|| self.stale_cached_preview_for(&entry))
                    .unwrap_or_else(|| {
                        preview::PreviewContent::new(preview::PreviewKind::Document, Vec::new())
                            .with_detail("PDF document")
                    })
            }
            Some(entry) if preview::should_build_preview_in_background(&entry) => {
                if let Some(preview) = self.cached_preview_for(&entry) {
                    self.preview_state.metrics.cache_hits += 1;
                    self.preview_state.load_state = None;
                    preview
                } else if let Some(stale_preview) = self.stale_cached_preview_for(&entry) {
                    self.preview_state.metrics.cache_misses += 1;
                    let loading_path = entry.path.clone();
                    let request = PreviewRequest {
                        token: self.preview_state.token,
                        entry,
                        priority: PreviewPriority::High,
                    };
                    if !self.scheduler.submit_preview(request) {
                        self.preview_state.load_state = None;
                        stale_preview.with_status_note("Refresh unavailable")
                    } else {
                        self.preview_state.load_state =
                            Some(PreviewLoadState::Refreshing(loading_path));
                        stale_preview.with_status_note("Refreshing in background")
                    }
                } else {
                    self.preview_state.metrics.cache_misses += 1;
                    let placeholder = preview::loading_preview_for(&entry);
                    let loading_path = entry.path.clone();
                    let request = PreviewRequest {
                        token: self.preview_state.token,
                        entry,
                        priority: PreviewPriority::High,
                    };
                    if !self.scheduler.submit_preview(request) {
                        self.preview_state.load_state = None;
                        preview::PreviewContent::placeholder("Preview worker unavailable")
                    } else {
                        self.preview_state.load_state =
                            Some(PreviewLoadState::Placeholder(loading_path));
                        placeholder
                    }
                }
            }
            Some(entry) => {
                self.preview_state.load_state = None;
                preview::build_preview(&entry)
            }
            None => {
                self.preview_state.load_state = None;
                preview::PreviewContent::placeholder("No selection")
            }
        };
        self.preview_state.scroll = 0;
        self.preview_state.horizontal_scroll = 0;
        self.sync_preview_scroll();
        self.refresh_static_image_preloads();
        self.prefetch_nearby_previews();
    }

    pub(crate) fn process_preview_refresh_timers(&mut self) -> bool {
        let Some(deadline) = self.preview_state.deferred_refresh_at else {
            return false;
        };
        if Instant::now() < deadline {
            return false;
        }
        self.refresh_preview();
        true
    }

    pub(crate) fn pending_preview_refresh_timer(&self) -> Option<std::time::Duration> {
        self.preview_state
            .deferred_refresh_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    fn cached_preview_for(&self, entry: &Entry) -> Option<preview::PreviewContent> {
        let cached = self.preview_state.result_cache.get(&entry.path)?;
        if cached.size == entry.size && cached.modified == entry.modified {
            Some(cached.preview.clone())
        } else {
            None
        }
    }

    fn stale_cached_preview_for(&self, entry: &Entry) -> Option<preview::PreviewContent> {
        self.preview_state
            .result_cache
            .get(&entry.path)
            .map(|cached| cached.preview.clone())
    }

    pub(super) fn cache_preview_result(
        &mut self,
        entry: &Entry,
        preview: &preview::PreviewContent,
    ) {
        self.preview_state.result_cache.insert(
            entry.path.clone(),
            CachedPreview {
                size: entry.size,
                modified: entry.modified,
                preview: preview.clone(),
            },
        );
        self.preview_state
            .result_order
            .retain(|path| path != &entry.path);
        self.preview_state
            .result_order
            .push_back(entry.path.clone());

        while self.preview_state.result_order.len() > PREVIEW_CACHE_LIMIT {
            if let Some(stale_path) = self.preview_state.result_order.pop_front() {
                self.preview_state.result_cache.remove(&stale_path);
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
        self.directory_runtime.pending_load = Some(load);
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
        self.directory_runtime.fingerprint = snapshot.fingerprint;
        self.directory_runtime.last_auto_reload_at = Instant::now();

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
        self.last_selection_change_at = Instant::now();
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
                self.navigation_history.back.push(HistoryEntry {
                    cwd: load.previous_cwd,
                    selected_path: load.previous_selected_path,
                });
                self.navigation_history.forward.clear();
            }
            DirectoryHistoryMode::GoBack => {
                if !self.navigation_history.back.is_empty() {
                    self.navigation_history.back.pop();
                }
                self.navigation_history.forward.push(HistoryEntry {
                    cwd: load.previous_cwd,
                    selected_path: load.previous_selected_path,
                });
            }
            DirectoryHistoryMode::GoForward => {
                if !self.navigation_history.forward.is_empty() {
                    self.navigation_history.forward.pop();
                }
                self.navigation_history.back.push(HistoryEntry {
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
                token: self.preview_state.token,
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
        self.preview_state.result_cache.contains_key(path)
    }

    fn remembered_view_for(&self, cwd: &Path) -> Option<DirectoryViewMemory> {
        self.directory_view_memory.get(cwd).cloned()
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

    pub(super) fn open_search_with_status(&mut self, scope: SearchScope) {
        if let Err(error) = self.open_fuzzy_finder(scope) {
            self.status = format!("Search unavailable: {error}");
        }
    }

    pub(super) fn toggle_view_mode(&mut self) {
        self.clear_wheel_scroll();
        self.view_mode = self.view_mode.toggle();
        self.sync_scroll();
        self.status = format!("Switched to {} view", self.view_mode.label());
    }

    pub(super) fn cycle_sort_mode(&mut self) -> Result<()> {
        self.sort_mode = self.sort_mode.cycle();
        self.reload()?;
        self.status = format!("Sort: {}", self.sort_mode.label());
        Ok(())
    }

    pub(super) fn toggle_hidden_files(&mut self) -> Result<()> {
        self.show_hidden = !self.show_hidden;
        self.reload()?;
        self.status = if self.show_hidden {
            "Hidden files shown".to_string()
        } else {
            "Hidden files hidden".to_string()
        };
        Ok(())
    }

    pub fn can_go_back(&self) -> bool {
        !self.navigation_history.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.navigation_history.forward.is_empty()
    }

    pub(super) fn set_selected(&mut self, index: usize) {
        self.set_selected_with_preview_mode(index, PreviewRefreshMode::Immediate);
    }

    fn set_selected_with_preview_mode(&mut self, index: usize, preview_mode: PreviewRefreshMode) {
        let next = index.min(self.entries.len().saturating_sub(1));
        if next != self.selected {
            self.selected = next;
            self.last_selection_change_at = Instant::now();
            match preview_mode {
                PreviewRefreshMode::Immediate => self.refresh_preview(),
                PreviewRefreshMode::Deferred => {
                    self.preview_state.deferred_refresh_at =
                        Some(Instant::now() + HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY);
                }
            }
        } else {
            self.selected = next;
        }
        self.sync_scroll();
        self.refresh_static_image_preloads();
        self.remember_current_directory_view();
    }

    pub(super) fn set_selected_last(&mut self) {
        if !self.entries.is_empty() {
            let last = self.entries.len() - 1;
            self.set_selected(last);
        }
    }

    pub(super) fn set_selected_delta(&mut self, delta: isize) {
        self.set_selected_delta_with_preview_mode(delta, PreviewRefreshMode::Immediate);
    }

    fn set_selected_delta_with_preview_mode(
        &mut self,
        delta: isize,
        preview_mode: PreviewRefreshMode,
    ) {
        if self.entries.is_empty() {
            self.selected = 0;
            self.preview_state.content = preview::PreviewContent::placeholder("No selection");
            self.preview_state.deferred_refresh_at = None;
            return;
        }

        let max_index = self.entries.len().saturating_sub(1) as isize;
        let next = (self.selected as isize + delta).clamp(0, max_index) as usize;
        self.set_selected_with_preview_mode(next, preview_mode);
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
        self.move_vertical_with_preview_mode(rows, PreviewRefreshMode::Immediate);
    }

    pub(super) fn move_vertical_with_preview_mode(
        &mut self,
        rows: isize,
        preview_mode: PreviewRefreshMode,
    ) {
        if self.view_mode == ViewMode::Grid {
            self.move_grid_vertical_with_preview_mode(rows, preview_mode);
        } else {
            self.set_selected_delta_with_preview_mode(rows, preview_mode);
        }
    }

    pub(super) fn move_by(&mut self, delta: isize) {
        self.set_selected_delta(delta);
    }

    pub(super) fn move_grid_vertical(&mut self, rows: isize) {
        self.move_grid_vertical_with_preview_mode(rows, PreviewRefreshMode::Immediate);
    }

    fn move_grid_vertical_with_preview_mode(
        &mut self,
        rows: isize,
        preview_mode: PreviewRefreshMode,
    ) {
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

        self.set_selected_with_preview_mode(target_index, preview_mode);
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
            self.preview_state.content = preview::PreviewContent::placeholder("No selection");
            self.preview_state.scroll = 0;
            self.preview_state.horizontal_scroll = 0;
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
        if normalized == self.cwd && self.directory_runtime.pending_load.is_none() {
            self.status = format!("Already in {}", self.cwd.display());
            return Ok(());
        }
        if self
            .directory_runtime
            .pending_load
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
        let Some(previous) = self.navigation_history.back.last().cloned() else {
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
        let Some(next) = self.navigation_history.forward.last().cloned() else {
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
        self.directory_runtime.watch = None;
        self.directory_runtime.pending_reload_at = None;
        while self.directory_runtime.watch_rx.try_recv().is_ok() {}

        match watching::start_directory_watcher(&self.cwd, &self.directory_runtime.watch_tx) {
            Ok(watcher) => {
                self.directory_runtime.watch = Some(watcher);
                self.directory_runtime.use_polling_reload = false;
            }
            Err(_) => {
                self.directory_runtime.use_polling_reload = true;
            }
        }
    }

    fn reload_if_directory_changed(&mut self) -> Result<bool> {
        if self.directory_runtime.pending_load.is_some() {
            return Ok(false);
        }
        let fingerprint = match support::scan_directory_fingerprint(&self.cwd, self.show_hidden) {
            Ok(fingerprint) => fingerprint,
            Err(_) => return Ok(false),
        };
        if fingerprint == self.directory_runtime.fingerprint {
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
mod tests;
