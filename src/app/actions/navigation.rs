use super::*;
use crate::preview::PreviewContent;

impl App {
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

    pub(in crate::app) fn open_search_with_status(&mut self, scope: SearchScope) {
        if let Err(error) = self.open_fuzzy_finder(scope) {
            self.status = format!("Search unavailable: {error}");
        }
    }

    pub(in crate::app) fn toggle_view_mode(&mut self) {
        self.clear_wheel_scroll();
        self.view_mode = self.view_mode.toggle();
        self.sync_scroll();
        self.status = format!("Switched to {} view", self.view_mode.label());
    }

    pub(in crate::app) fn cycle_sort_mode(&mut self) -> Result<()> {
        self.sort_mode = self.sort_mode.cycle();
        self.reload()?;
        self.status = format!("Sort: {}", self.sort_mode.label());
        Ok(())
    }

    pub(in crate::app) fn toggle_hidden_files(&mut self) -> Result<()> {
        if self.cwd_is_trash() {
            self.status = "Trash shows all files".to_string();
            return Ok(());
        }
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

    pub(in crate::app) fn set_selected(&mut self, index: usize) {
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

    pub(in crate::app) fn set_selected_last(&mut self) {
        if !self.entries.is_empty() {
            let last = self.entries.len() - 1;
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
        if self.entries.is_empty() {
            self.selected = 0;
            self.preview_state.content = PreviewContent::placeholder("No selection");
            self.preview_state.deferred_refresh_at = None;
            return;
        }

        let max_index = self.entries.len().saturating_sub(1) as isize;
        let next = (self.selected as isize + delta).clamp(0, max_index) as usize;
        self.set_selected_with_preview_mode(next, preview_mode);
    }

    pub(in crate::app) fn page(&mut self, direction: isize) {
        let rows = self.frame_state.metrics.rows_visible.max(1) as isize;
        if self.view_mode == ViewMode::Grid {
            self.move_grid_vertical(direction * rows);
        } else {
            self.set_selected_delta(direction * rows);
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
        if self.view_mode == ViewMode::Grid {
            self.move_grid_vertical_with_preview_mode(rows, preview_mode);
        } else {
            self.set_selected_delta_with_preview_mode(rows, preview_mode);
        }
    }

    pub(in crate::app) fn move_by(&mut self, delta: isize) {
        self.set_selected_delta(delta);
    }

    pub(in crate::app) fn move_grid_vertical(&mut self, rows: isize) {
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

    pub(in crate::app) fn adjust_zoom(&mut self, delta: i8) {
        let next = (self.zoom_level as i8 + delta).clamp(0, 2) as u8;
        if next == self.zoom_level {
            self.status = format!("Directory zoom limit: {}", self.zoom_level);
            return;
        }
        self.zoom_level = next;
        self.status = format!("Directory zoom set to {}", self.zoom_level);
        self.sync_scroll();
    }

    pub(in crate::app) fn reset_zoom(&mut self) {
        self.zoom_level = 1;
        self.status = format!("Directory zoom reset to {}", self.zoom_level);
        self.sync_scroll();
    }

    pub(in crate::app) fn select_index(&mut self, index: usize) {
        self.set_selected(index);
    }

    pub(in crate::app) fn select_last(&mut self) {
        self.set_selected_last();
    }

    pub(in crate::app) fn clamp_selection(&mut self) {
        if self.entries.is_empty() {
            self.selected = 0;
            self.scroll_row = 0;
            self.preview_state.content = PreviewContent::placeholder("No selection");
            self.preview_state.scroll = 0;
            self.preview_state.horizontal_scroll = 0;
        } else if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
        self.sync_preview_scroll();
    }

    pub(in crate::app) fn sync_scroll(&mut self) -> bool {
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

    pub(in crate::app) fn step_pinned_place(&mut self, delta: isize) -> Result<()> {
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

    pub(in crate::app) fn go_back(&mut self) -> Result<()> {
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

    pub(in crate::app) fn go_forward(&mut self) -> Result<()> {
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

    pub(in crate::app) fn open_in_system(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry() else {
            return Ok(());
        };

        let target = entry.path.clone();
        for (program, args) in [("gio", vec!["open"]), ("xdg-open", Vec::new())] {
            match crate::fs::detached_open(program, &args, &target) {
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
}
