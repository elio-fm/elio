use super::*;
use crate::app::FileClass;
use crate::file_info;
use crate::preview::{PreviewContent, PreviewWorkClass, preview_work_class};

impl App {
    pub fn selection_summary(&self) -> String {
        match self.selected_entry() {
            Some(entry) => {
                let suffix = if entry.is_dir() { "/" } else { "" };
                format!(
                    "{}/{}  {}{}",
                    self.navigation.selected.saturating_add(1),
                    self.navigation.entries.len(),
                    entry.name,
                    suffix,
                )
            }
            None => format!("0/0  {}", self.navigation.cwd.display()),
        }
    }

    pub fn status_message(&self) -> &str {
        &self.status
    }

    pub(in crate::app) fn open_search_with_status(&mut self, scope: SearchScope) {
        if let Err(error) = self.open_fuzzy_finder(scope) {
            self.status = format!("Search unavailable: {error}");
        }
    }

    pub(in crate::app) fn toggle_view_mode(&mut self) {
        self.clear_wheel_scroll();
        self.navigation.view_mode = self.navigation.view_mode.toggle();
        self.sync_scroll();
        self.status = format!("Switched to {} view", self.navigation.view_mode.label());
    }

    pub(in crate::app) fn cycle_sort_mode(&mut self) -> Result<()> {
        self.navigation.sort_mode = self.navigation.sort_mode.cycle();
        self.reload()?;
        self.status = format!("Sort: {}", self.navigation.sort_mode.label());
        Ok(())
    }

    pub(in crate::app) fn toggle_hidden_files(&mut self) -> Result<()> {
        if self.cwd_is_trash() {
            self.status = "Trash shows all files".to_string();
            return Ok(());
        }
        self.navigation.show_hidden = !self.navigation.show_hidden;
        self.reload()?;
        self.status = if self.navigation.show_hidden {
            "Hidden files shown".to_string()
        } else {
            "Hidden files hidden".to_string()
        };
        Ok(())
    }

    pub fn can_go_back(&self) -> bool {
        !self.navigation.navigation_history.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.navigation.navigation_history.forward.is_empty()
    }

    pub(in crate::app) fn set_selected(&mut self, index: usize) {
        self.set_selected_with_preview_mode(index, PreviewRefreshMode::Immediate);
    }

    fn set_selected_with_preview_mode(&mut self, index: usize, preview_mode: PreviewRefreshMode) {
        let next = index.min(self.navigation.entries.len().saturating_sub(1));
        if next != self.navigation.selected {
            let preview_mode =
                self.effective_preview_refresh_mode_for_selection(next, preview_mode);
            self.navigation.selected = next;
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
            self.navigation.selected = next;
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
        let Some(entry) = self.navigation.entries.get(index).cloned() else {
            return preview_mode;
        };
        let variant = self.preview_request_options_for_entry(&entry);
        let builtin_class = file_info::inspect_entry_cached(&entry).builtin_class;
        let cold_heavy_preview = matches!(builtin_class, FileClass::Audio | FileClass::Video)
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

    pub(in crate::app) fn set_selected_last(&mut self) {
        if !self.navigation.entries.is_empty() {
            let last = self.navigation.entries.len() - 1;
            self.set_selected(last);
        }
    }

    pub(in crate::app) fn set_selected_delta(&mut self, delta: isize) {
        self.set_selected_delta_with_preview_mode(delta, PreviewRefreshMode::Immediate);
    }

    fn set_selected_delta_with_preview_mode(
        &mut self,
        delta: isize,
        preview_mode: PreviewRefreshMode,
    ) {
        if self.navigation.entries.is_empty() {
            self.navigation.selected = 0;
            self.preview.state.content = PreviewContent::placeholder("No selection");
            self.clear_preview_directory_stats();
            self.preview.state.deferred_refresh_at = None;
            return;
        }

        let max_index = self.navigation.entries.len().saturating_sub(1) as isize;
        let next = (self.navigation.selected as isize + delta).clamp(0, max_index) as usize;
        self.set_selected_with_preview_mode(next, preview_mode);
    }

    pub(in crate::app) fn page(&mut self, direction: isize) {
        let rows = self.input.frame_state.metrics.rows_visible.max(1) as isize;
        let mode = self.rapid_nav_preview_mode(PreviewRefreshMode::Immediate);
        let prev = self.navigation.selected;
        if self.navigation.view_mode == ViewMode::Grid {
            self.move_grid_vertical_with_preview_mode(direction * rows, mode);
        } else {
            self.set_selected_delta_with_preview_mode(direction * rows, mode);
        }
        if self.navigation.selected != prev {
            self.input.last_key_nav_at = Instant::now();
        }
    }

    /// Keyboard-only: applies rapid-nav deferred preview for Up/Down/j/k, then moves.
    pub(in crate::app) fn move_vertical_keyboard(&mut self, rows: isize) {
        let mode = self.rapid_nav_preview_mode(PreviewRefreshMode::Immediate);
        let prev = self.navigation.selected;
        self.move_vertical_with_preview_mode(rows, mode);
        if self.navigation.selected != prev {
            self.input.last_key_nav_at = Instant::now();
        }
    }

    /// Keyboard-only: applies rapid-nav deferred preview for grid h/l navigation, then moves.
    pub(in crate::app) fn move_by_keyboard(&mut self, delta: isize) {
        let mode = self.rapid_nav_preview_mode(PreviewRefreshMode::Immediate);
        let prev = self.navigation.selected;
        self.set_selected_delta_with_preview_mode(delta, mode);
        if self.navigation.selected != prev {
            self.input.last_key_nav_at = Instant::now();
        }
    }

    pub(in crate::app) fn move_vertical(&mut self, rows: isize) {
        self.move_vertical_with_preview_mode(rows, PreviewRefreshMode::Immediate);
    }

    pub(in crate::app) fn move_vertical_with_preview_mode(
        &mut self,
        rows: isize,
        preview_mode: PreviewRefreshMode,
    ) {
        if self.navigation.view_mode == ViewMode::Grid {
            self.move_grid_vertical_with_preview_mode(rows, preview_mode);
        } else {
            self.set_selected_delta_with_preview_mode(rows, preview_mode);
        }
    }

    pub(in crate::app) fn move_by(&mut self, delta: isize) {
        self.set_selected_delta(delta);
    }

    fn move_grid_vertical_with_preview_mode(
        &mut self,
        rows: isize,
        preview_mode: PreviewRefreshMode,
    ) {
        if self.navigation.entries.is_empty() {
            self.navigation.selected = 0;
            return;
        }

        let cols = self.input.frame_state.metrics.cols.max(1);
        let current_row = self.navigation.selected / cols;
        let current_col = self.navigation.selected % cols;
        let total_rows = self.navigation.entries.len().div_ceil(cols);
        let target_row = current_row as isize + rows;

        if target_row < 0 || target_row >= total_rows as isize {
            return;
        }

        let target_index = target_row as usize * cols + current_col;
        if target_index >= self.navigation.entries.len() {
            return;
        }

        self.set_selected_with_preview_mode(target_index, preview_mode);
    }

    pub(in crate::app) fn adjust_zoom(&mut self, delta: i8) {
        let next = (self.navigation.zoom_level as i8 + delta).clamp(0, 2) as u8;
        if next == self.navigation.zoom_level {
            self.status = format!("Grid zoom limit: {}", self.navigation.zoom_level);
            return;
        }
        self.navigation.zoom_level = next;
        self.status = format!("Grid zoom set to {}", self.navigation.zoom_level);
        self.sync_scroll();
    }

    pub(in crate::app) fn select_index(&mut self, index: usize) {
        self.set_selected(index);
    }

    pub(in crate::app) fn select_last(&mut self) {
        self.set_selected_last();
    }

    pub(in crate::app) fn clamp_selection(&mut self) {
        if self.navigation.entries.is_empty() {
            self.navigation.selected = 0;
            self.navigation.scroll_row = 0;
            self.preview.state.content = PreviewContent::placeholder("No selection");
            self.clear_preview_directory_stats();
            self.preview.state.scroll = 0;
            self.preview.state.horizontal_scroll = 0;
        } else if self.navigation.selected >= self.navigation.entries.len() {
            self.navigation.selected = self.navigation.entries.len() - 1;
        }
        self.sync_preview_scroll();
    }

    pub(in crate::app) fn sync_scroll(&mut self) -> bool {
        let previous = self.navigation.scroll_row;
        if self.navigation.entries.is_empty() {
            self.navigation.scroll_row = 0;
            return previous != self.navigation.scroll_row;
        }

        let cols = self.input.frame_state.metrics.cols.max(1);
        let rows_visible = self.input.frame_state.metrics.rows_visible.max(1);
        let selected_row = self.navigation.selected / cols;
        if selected_row < self.navigation.scroll_row {
            self.navigation.scroll_row = selected_row;
        } else if selected_row >= self.navigation.scroll_row + rows_visible {
            self.navigation.scroll_row = selected_row + 1 - rows_visible;
        }
        self.navigation.scroll_row = self.navigation.scroll_row.min(self.max_scroll_row());
        previous != self.navigation.scroll_row
    }

    fn max_scroll_row(&self) -> usize {
        if self.navigation.entries.is_empty() {
            return 0;
        }

        let cols = self.input.frame_state.metrics.cols.max(1);
        let rows_visible = self.input.frame_state.metrics.rows_visible.max(1);
        let total_rows = self.navigation.entries.len().div_ceil(cols);
        total_rows.saturating_sub(rows_visible)
    }

    pub(in crate::app) fn step_sidebar_place(&mut self, delta: isize) -> Result<()> {
        let places = self
            .navigation
            .sidebar
            .iter()
            .filter_map(|row| row.item())
            .collect::<Vec<_>>();
        if places.is_empty() {
            return Ok(());
        }

        let current = places
            .iter()
            .position(|item| item.path == self.navigation.cwd);
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

    pub(in crate::app) fn go_back(&mut self) -> Result<()> {
        let Some(previous) = self.navigation.navigation_history.back.last().cloned() else {
            self.status = "No previous folder".to_string();
            return Ok(());
        };
        self.set_dir_transition(
            previous.cwd,
            DirectoryHistoryMode::GoBack,
            previous
                .selected_path
                .or_else(|| Some(self.navigation.cwd.clone())),
            DirectoryLoadCompletion::Clear,
        )
    }

    pub(in crate::app) fn go_forward(&mut self) -> Result<()> {
        let Some(next) = self.navigation.navigation_history.forward.last().cloned() else {
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

    pub(in crate::app) fn open_in_system(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry() else {
            return Ok(());
        };

        let target = entry.path.clone();
        match crate::fs::open_in_system(&target) {
            Ok(()) => self.status = format!("Opened {}", target.display()),
            Err(e) => self.status = e,
        }
        Ok(())
    }
}
