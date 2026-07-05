use std::path::Path;

use super::*;

impl App {
    pub(in crate::app) fn remember_drag_candidate(&mut self, path: PathBuf) {
        if !self.input.drag_suppressed_until_up {
            self.input.drag_paths = self.drag_paths_for_candidate(&path);
            self.input.drag_candidate = Some(path);
        }
    }

    pub(crate) fn clear_drag_candidate(&mut self) {
        self.input.drag_candidate = None;
        self.input.drag_paths.clear();
    }

    pub(crate) fn clear_drag_state(&mut self) {
        self.input.drag_candidate = None;
        self.input.drag_paths.clear();
        self.input.drag_suppressed_until_up = false;
    }

    pub(in crate::app) fn suppress_drag_until_button_up(&mut self) {
        self.input.drag_candidate = None;
        self.input.drag_paths.clear();
        self.input.drag_suppressed_until_up = true;
    }

    pub(crate) fn take_drag_export_paths_at(&mut self, column: u16, row: u16) -> Vec<PathBuf> {
        if self.input.drag_suppressed_until_up {
            self.input.drag_candidate = None;
            self.input.drag_paths.clear();
            return Vec::new();
        }

        if let Some(candidate) = self.input.drag_candidate.take() {
            let snapshot = std::mem::take(&mut self.input.drag_paths);
            if !snapshot.is_empty() {
                return snapshot;
            }
            return self.drag_paths_for_candidate(&candidate);
        }

        let candidate = self.entry_path_at(column, row);
        let Some(candidate) = candidate else {
            return Vec::new();
        };

        self.drag_paths_for_candidate(&candidate)
    }

    fn drag_paths_for_candidate(&self, candidate: &Path) -> Vec<PathBuf> {
        if !self.navigation.selected_paths.is_empty()
            && self.navigation.selected_paths.contains(candidate)
        {
            return self.selected_paths_sorted();
        }

        vec![candidate.to_path_buf()]
    }

    fn entry_path_at(&self, column: u16, row: u16) -> Option<PathBuf> {
        self.input
            .frame_state
            .entry_hits
            .iter()
            .find(|hit| rect_contains(hit.rect, column, row))
            .and_then(|hit| self.navigation.entries.get(hit.index))
            .map(|entry| entry.path.clone())
    }

    #[cfg(test)]
    pub(crate) fn drag_export_paths(&self) -> Vec<PathBuf> {
        if !self.navigation.selected_paths.is_empty() {
            return self.selected_paths_sorted();
        }
        self.selected_entry()
            .map(|entry| vec![entry.path.clone()])
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-drag-{label}-{unique}"))
    }

    fn make_app_with_entries() -> (App, PathBuf, PathBuf, PathBuf) {
        let root = temp_path("export");
        fs::create_dir_all(&root).expect("temp root");
        let alpha = root.join("alpha.txt");
        let beta = root.join("beta.txt");
        let gamma = root.join("gamma.txt");
        fs::write(&alpha, "a").expect("alpha");
        fs::write(&beta, "b").expect("beta");
        fs::write(&gamma, "g").expect("gamma");
        let mut app = App::new_at(root).expect("app should initialize");
        app.navigation.entries = vec![
            Entry {
                path: alpha.clone(),
                name: "alpha.txt".to_string(),
                name_key: "alpha.txt".to_string(),
                ..Entry::default()
            },
            Entry {
                path: beta.clone(),
                name: "beta.txt".to_string(),
                name_key: "beta.txt".to_string(),
                ..Entry::default()
            },
            Entry {
                path: gamma.clone(),
                name: "gamma.txt".to_string(),
                name_key: "gamma.txt".to_string(),
                ..Entry::default()
            },
        ];
        app.navigation.selected = 1;
        (app, alpha, beta, gamma)
    }

    #[test]
    fn drag_without_selection_exports_focused_entry() {
        let (app, _alpha, beta, _gamma) = make_app_with_entries();

        assert_eq!(app.drag_export_paths(), vec![beta]);
    }

    #[test]
    fn drag_with_selection_exports_sorted_selection() {
        let (mut app, alpha, _beta, gamma) = make_app_with_entries();
        app.navigation.selected_paths.insert(gamma.clone());
        app.navigation.selected_paths.insert(alpha.clone());

        assert_eq!(app.drag_export_paths(), vec![alpha, gamma]);
    }

    #[test]
    fn drag_with_no_entries_exports_nothing() {
        let (mut app, _alpha, _beta, _gamma) = make_app_with_entries();
        app.navigation.entries.clear();

        assert!(app.drag_export_paths().is_empty());
    }

    #[test]
    fn drag_offer_exports_clicked_entry_when_it_is_not_selected() {
        let (mut app, _alpha, beta, _gamma) = make_app_with_entries();
        app.remember_drag_candidate(beta.clone());

        assert_eq!(app.take_drag_export_paths_at(0, 0), vec![beta]);
    }

    #[test]
    fn drag_offer_exports_selection_when_clicked_entry_is_selected() {
        let (mut app, alpha, _beta, gamma) = make_app_with_entries();
        app.navigation.selected_paths.insert(gamma.clone());
        app.navigation.selected_paths.insert(alpha.clone());
        app.remember_drag_candidate(gamma.clone());

        assert_eq!(app.take_drag_export_paths_at(0, 0), vec![alpha, gamma]);
    }

    #[test]
    fn drag_offer_uses_mouse_down_selection_snapshot() {
        let (mut app, alpha, _beta, gamma) = make_app_with_entries();
        app.navigation.selected_paths.insert(gamma.clone());
        app.navigation.selected_paths.insert(alpha.clone());
        app.remember_drag_candidate(gamma.clone());
        app.navigation.selected_paths.clear();

        assert_eq!(app.take_drag_export_paths_at(0, 0), vec![alpha, gamma]);
    }

    #[test]
    fn drag_offer_without_click_candidate_can_use_entry_hit() {
        let (mut app, _alpha, _beta, _gamma) = make_app_with_entries();
        app.set_frame_state(FrameState {
            entry_hits: vec![EntryHit {
                rect: Rect::new(2, 3, 12, 1),
                index: 1,
            }],
            ..FrameState::default()
        });

        let expected = app.navigation.entries[1].path.clone();
        assert_eq!(app.take_drag_export_paths_at(3, 3), vec![expected]);
    }

    #[test]
    fn drag_offer_outside_entry_exports_nothing() {
        let (mut app, _alpha, _beta, _gamma) = make_app_with_entries();

        assert!(app.take_drag_export_paths_at(0, 0).is_empty());
    }

    #[test]
    fn suppressed_drag_offer_exports_nothing_until_cleared() {
        let (mut app, _alpha, beta, _gamma) = make_app_with_entries();
        app.remember_drag_candidate(beta.clone());
        app.suppress_drag_until_button_up();

        assert!(app.take_drag_export_paths_at(0, 0).is_empty());

        app.clear_drag_state();
        app.remember_drag_candidate(beta.clone());
        assert_eq!(app.take_drag_export_paths_at(0, 0), vec![beta]);
    }
}
