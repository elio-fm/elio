use super::*;

impl App {
    pub fn is_selected(&self, path: &std::path::Path) -> bool {
        self.selected_paths.contains(path)
    }

    pub fn selection_count(&self) -> usize {
        self.selected_paths.len()
    }

    pub(in crate::app) fn toggle_selection(&mut self) {
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let path = entry.path.clone();
        if !self.selected_paths.remove(&path) {
            self.selected_paths.insert(path);
        }
        if self.view_mode == ViewMode::List {
            self.move_vertical(1);
        }
    }

    pub(in crate::app) fn select_all(&mut self) {
        self.selected_paths = self.entries.iter().map(|e| e.path.clone()).collect();
    }

    pub(in crate::app) fn clear_selection(&mut self) {
        self.selected_paths.clear();
    }
}
