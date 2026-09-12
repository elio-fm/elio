use super::App;
use crate::chooser::ChooserExit;
use crate::file_browser::{SelectionChange, ViewMode};
use std::path::{Path, PathBuf};

impl App {
    pub fn is_selected(&self, path: &std::path::Path) -> bool {
        self.file_browser.is_selected(path)
    }

    pub fn selection_count(&self) -> usize {
        if let Some(overlay) = &self.duplicate_finder.session {
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
        self.chooser.enable();
        self.status = "Chooser mode".to_string();
    }

    pub(crate) fn take_chooser_exit(&mut self) -> Option<ChooserExit> {
        self.chooser.take_exit()
    }

    pub(crate) fn chooser_mode(&self) -> bool {
        self.chooser.is_enabled()
    }

    #[cfg(test)]
    pub(crate) fn chooser_exit(&self) -> Option<&ChooserExit> {
        self.chooser.exit()
    }

    pub(crate) fn confirm_chooser(&mut self) {
        let cwd = self.file_browser.cwd.clone();
        let focused_path = self.selected_entry().map(|entry| entry.path.clone());
        let selected_paths = self.selected_paths_sorted();
        if self
            .chooser
            .confirm_selection(&cwd, focused_path.as_deref(), selected_paths)
        {
            self.should_quit = true;
        }
    }

    pub(crate) fn confirm_chooser_path(&mut self, path: &Path) {
        if self.chooser.confirm_path(&self.file_browser.cwd, path) {
            self.should_quit = true;
        }
    }

    pub(crate) fn cancel_chooser(&mut self) {
        if self.chooser.cancel() {
            self.should_change_directory_on_quit = false;
            self.should_quit = true;
        }
    }
}
