use std::path::PathBuf;

use super::DuplicateFinderState;

impl DuplicateFinderState {
    pub(crate) fn move_selection(&mut self, delta: isize) {
        let count = self.file_count();
        if count == 0 {
            return;
        }
        let max = count.saturating_sub(1) as isize;
        self.selected = (self.selected as isize + delta).clamp(0, max) as usize;
    }

    pub(crate) fn set_selection(&mut self, index: usize) {
        self.selected = index.min(self.file_count().saturating_sub(1));
    }

    pub(crate) fn sync_scroll(&mut self, rows_visible: usize) -> bool {
        let count = self.file_count();
        let previous_selected = self.selected;
        let previous_scroll = self.scroll;
        if count == 0 {
            self.selected = 0;
            self.scroll = 0;
            return previous_selected != self.selected || previous_scroll != self.scroll;
        }
        self.selected = self.selected.min(count - 1);
        let rows_visible = rows_visible.max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + rows_visible {
            self.scroll = self.selected.saturating_sub(rows_visible - 1);
        }
        self.scroll = self.scroll.min(count.saturating_sub(rows_visible));
        previous_selected != self.selected || previous_scroll != self.scroll
    }

    pub(crate) fn toggle_focused_selection(&mut self) -> bool {
        let Some(path) = self.focused_path() else {
            return false;
        };
        if !self.selected_paths.insert(path.clone()) {
            self.selected_paths.remove(&path);
        }
        true
    }

    pub(crate) fn select_all(&mut self) {
        let paths = self
            .groups
            .iter()
            .flat_map(|group| group.files.iter().map(|file| file.path.clone()));
        self.selected_paths.extend(paths);
    }

    pub(crate) fn clear_selection(&mut self) -> bool {
        if self.selected_paths.is_empty() {
            return false;
        }
        self.selected_paths.clear();
        true
    }

    pub(crate) fn action_paths(&self) -> Vec<PathBuf> {
        if !self.selected_paths.is_empty() {
            let mut paths = self.selected_paths.iter().cloned().collect::<Vec<_>>();
            paths.sort();
            return paths;
        }
        self.focused_path().into_iter().collect()
    }
}

#[cfg(test)]
#[path = "tests/result_selection.rs"]
mod tests;
