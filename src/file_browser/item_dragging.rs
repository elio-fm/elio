use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct ItemDragState {
    candidate: Option<PathBuf>,
    paths: Vec<PathBuf>,
    suppressed_until_up: bool,
}

impl super::FileBrowserState {
    pub(crate) fn remember_drag_candidate(&mut self, path: PathBuf) {
        if !self.item_drag.suppressed_until_up {
            self.item_drag.paths = self.drag_paths_for_candidate(&path);
            self.item_drag.candidate = Some(path);
        }
    }

    pub(crate) fn clear_drag_candidate(&mut self) {
        self.item_drag.candidate = None;
        self.item_drag.paths.clear();
    }

    pub(crate) fn clear_drag_state(&mut self) {
        self.clear_drag_candidate();
        self.item_drag.suppressed_until_up = false;
    }

    pub(crate) fn suppress_drag_until_button_up(&mut self) {
        self.clear_drag_candidate();
        self.item_drag.suppressed_until_up = true;
    }

    pub(crate) fn take_drag_export_paths(
        &mut self,
        fallback_candidate: Option<PathBuf>,
    ) -> Vec<PathBuf> {
        if self.item_drag.suppressed_until_up {
            self.clear_drag_candidate();
            return Vec::new();
        }

        if let Some(candidate) = self.item_drag.candidate.take() {
            let snapshot = std::mem::take(&mut self.item_drag.paths);
            if !snapshot.is_empty() {
                return snapshot;
            }
            return self.drag_paths_for_candidate(&candidate);
        }

        fallback_candidate
            .map(|candidate| self.drag_paths_for_candidate(&candidate))
            .unwrap_or_default()
    }

    fn drag_paths_for_candidate(&self, candidate: &Path) -> Vec<PathBuf> {
        if !self.selected_paths.is_empty() && self.selected_paths.contains(candidate) {
            return self.selected_paths_sorted();
        }

        vec![candidate.to_path_buf()]
    }

    #[cfg(test)]
    pub(crate) fn drag_export_paths(&self) -> Vec<PathBuf> {
        if !self.selected_paths.is_empty() {
            return self.selected_paths_sorted();
        }
        self.selected_entry()
            .map(|entry| vec![entry.path.clone()])
            .unwrap_or_default()
    }
}
