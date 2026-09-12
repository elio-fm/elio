use super::*;
use std::path::{Path, PathBuf};

impl App {
    pub fn is_selected(&self, path: &std::path::Path) -> bool {
        self.file_browser.is_selected(path)
    }

    pub fn selection_count(&self) -> usize {
        if let Some(overlay) = &self.overlays.duplicates {
            return overlay.selected_paths.len();
        }
        self.file_browser.selection_count()
    }

    pub(crate) fn selected_paths_sorted(&self) -> Vec<PathBuf> {
        self.file_browser.selected_paths_sorted()
    }

    #[cfg(unix)]
    pub(crate) fn selected_paths_in_selection_order(&self) -> Vec<PathBuf> {
        self.file_browser.selected_paths_in_selection_order()
    }

    pub(crate) fn current_directory_escape_for_paths(&self, paths: &[PathBuf]) -> Option<PathBuf> {
        self.file_browser.current_directory_escape_for_paths(paths)
    }

    pub(crate) fn toggle_selection(&mut self) {
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let path = entry.path.clone();
        match self.file_browser.toggle_selected_path(path) {
            SelectionChange::NestingConflict => {
                self.status = "Cannot select nested paths".to_string();
            }
            SelectionChange::Inserted | SelectionChange::Removed => {
                self.status.clear();
                if self.file_browser.view_mode == ViewMode::List && !self.preview_fullscreen() {
                    self.move_vertical(1);
                }
            }
        }
    }

    pub(crate) fn select_all(&mut self) {
        let blocked = self.file_browser.select_all_visible();
        if blocked {
            self.status = "Cannot select nested paths".to_string();
        } else {
            self.status.clear();
        }
    }

    pub(crate) fn clear_selection(&mut self) {
        if self.file_browser.clear_selection() {
            self.status.clear();
        }
    }

    pub(crate) fn enable_chooser_mode(&mut self) {
        self.chooser_mode = true;
        self.status = "Chooser mode".to_string();
    }

    pub(crate) fn take_chooser_exit(&mut self) -> Option<ChooserExit> {
        self.chooser_exit.take()
    }

    pub(crate) fn confirm_chooser(&mut self) {
        if !self.chooser_mode {
            return;
        }
        self.chooser_exit = Some(ChooserExit::Confirmed(self.chooser_selection_paths()));
        self.should_quit = true;
    }

    pub(crate) fn confirm_chooser_path(&mut self, path: &Path) {
        if !self.chooser_mode {
            return;
        }
        self.chooser_exit = Some(ChooserExit::Confirmed(vec![
            self.absolute_chooser_path(path),
        ]));
        self.should_quit = true;
    }

    pub(crate) fn cancel_chooser(&mut self) {
        if !self.chooser_mode {
            return;
        }
        self.chooser_exit = Some(ChooserExit::Cancelled);
        self.should_change_directory_on_quit = false;
        self.should_quit = true;
    }

    fn chooser_selection_paths(&self) -> Vec<PathBuf> {
        if self.file_browser.selected_paths.is_empty() {
            return self
                .selected_entry()
                .map(|entry| vec![self.absolute_chooser_path(&entry.path)])
                .unwrap_or_default();
        }

        let mut paths: Vec<PathBuf> = self
            .selected_paths_sorted()
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
            self.file_browser.cwd.join(path)
        }
    }
}
