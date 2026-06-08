use super::*;
use std::path::{Path, PathBuf};

impl App {
    pub fn is_selected(&self, path: &std::path::Path) -> bool {
        self.navigation.selected_paths.contains(path)
    }

    pub fn selection_count(&self) -> usize {
        self.navigation.selected_paths.len()
    }

    pub(in crate::app) fn toggle_selection(&mut self) {
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let path = entry.path.clone();
        if !self.navigation.selected_paths.remove(&path) {
            self.navigation.selected_paths.insert(path);
        }
        if self.navigation.view_mode == ViewMode::List {
            self.move_vertical(1);
        }
    }

    pub(in crate::app) fn select_all(&mut self) {
        self.navigation.selected_paths = self
            .navigation
            .entries
            .iter()
            .map(|e| e.path.clone())
            .collect();
    }

    pub(in crate::app) fn clear_selection(&mut self) {
        self.navigation.selected_paths.clear();
    }

    pub(crate) fn enable_chooser_mode(&mut self) {
        self.chooser_mode = true;
        self.status = "Chooser mode".to_string();
    }

    pub(crate) fn take_chooser_exit(&mut self) -> Option<ChooserExit> {
        self.chooser_exit.take()
    }

    pub(in crate::app) fn confirm_chooser(&mut self) {
        if !self.chooser_mode {
            return;
        }
        self.chooser_exit = Some(ChooserExit::Confirmed(self.chooser_selection_paths()));
        self.should_quit = true;
    }

    pub(in crate::app) fn cancel_chooser(&mut self) {
        if !self.chooser_mode {
            return;
        }
        self.chooser_exit = Some(ChooserExit::Cancelled);
        self.should_change_directory_on_quit = false;
        self.should_quit = true;
    }

    fn chooser_selection_paths(&self) -> Vec<PathBuf> {
        if self.navigation.selected_paths.is_empty() {
            return self
                .selected_entry()
                .map(|entry| vec![self.absolute_chooser_path(&entry.path)])
                .unwrap_or_default();
        }

        let mut paths: Vec<PathBuf> = self
            .navigation
            .selected_paths
            .iter()
            .map(|path| self.absolute_chooser_path(path))
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    fn absolute_chooser_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.navigation.cwd.join(path)
        }
    }
}
