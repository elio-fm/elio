#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    Grid,
    List,
}

impl ViewMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Grid => Self::List,
            Self::List => Self::Grid,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Grid => "Grid",
            Self::List => "List",
        }
    }

    pub(crate) fn from_start_in_grid(start_in_grid: bool) -> Self {
        if start_in_grid {
            Self::Grid
        } else {
            Self::List
        }
    }
}

impl super::FileBrowserState {
    pub(crate) fn toggle_view_mode(&mut self) -> ViewMode {
        self.view_mode = self.view_mode.toggle();
        self.view_mode
    }

    pub(crate) fn cycle_sort_mode(&mut self) -> crate::fs::SortMode {
        self.sort_mode = self.sort_mode.cycle();
        self.sort_mode
    }

    pub(crate) fn toggle_hidden_files(&mut self) -> bool {
        self.show_hidden = !self.show_hidden;
        self.show_hidden
    }

    pub(crate) fn can_go_back(&self) -> bool {
        !self.directory_history.back.is_empty()
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        !self.directory_history.forward.is_empty()
    }

    pub(crate) fn clamped_selection_index(&self, index: usize) -> usize {
        index.min(self.entries.len().saturating_sub(1))
    }

    pub(crate) fn set_selected_index(&mut self, index: usize) -> bool {
        let next = self.clamped_selection_index(index);
        let changed = next != self.selected;
        self.selected = next;
        changed
    }

    pub(crate) fn selection_offset(&self, delta: isize) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let max_index = self.entries.len().saturating_sub(1) as isize;
        Some((self.selected as isize + delta).clamp(0, max_index) as usize)
    }

    pub(crate) fn grid_selection_offset(&self, rows: isize, cols: usize) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }

        let cols = cols.max(1);
        let current_row = self.selected / cols;
        let current_col = self.selected % cols;
        let total_rows = self.entries.len().div_ceil(cols);
        let target_row = current_row as isize + rows;
        if target_row < 0 || target_row >= total_rows as isize {
            return None;
        }

        let target_index = target_row as usize * cols + current_col;
        (target_index < self.entries.len()).then_some(target_index)
    }

    pub(crate) fn adjust_zoom(&mut self, delta: i8) -> bool {
        let next = (self.zoom_level as i8 + delta).clamp(0, 2) as u8;
        let changed = next != self.zoom_level;
        self.zoom_level = next;
        changed
    }

    pub(crate) fn clamp_selection(&mut self) -> bool {
        if self.entries.is_empty() {
            self.selected = 0;
            self.scroll_row = 0;
            return true;
        }
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
        false
    }

    pub(crate) fn sync_scroll(&mut self, cols: usize, rows_visible: usize) -> bool {
        let previous = self.scroll_row;
        if self.entries.is_empty() {
            self.scroll_row = 0;
            return previous != self.scroll_row;
        }

        let cols = cols.max(1);
        let rows_visible = rows_visible.max(1);
        let selected_row = self.selected / cols;
        if selected_row < self.scroll_row {
            self.scroll_row = selected_row;
        } else if selected_row >= self.scroll_row + rows_visible {
            self.scroll_row = selected_row + 1 - rows_visible;
        }
        self.scroll_row = self
            .scroll_row
            .min(self.maximum_scroll_row(cols, rows_visible));
        previous != self.scroll_row
    }

    fn maximum_scroll_row(&self, cols: usize, rows_visible: usize) -> usize {
        if self.entries.is_empty() {
            return 0;
        }
        let total_rows = self.entries.len().div_ceil(cols.max(1));
        total_rows.saturating_sub(rows_visible.max(1))
    }
}
