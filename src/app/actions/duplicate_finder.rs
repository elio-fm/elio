use super::super::*;
use crate::background_jobs::job_requests::DuplicateScanRequest;
use crate::file_operations::{BulkRenameItem, BulkRenameOverlay, RenameOverlay, TrashTarget};
use anyhow::Result;
use std::{
    fs,
    path::{Path, PathBuf},
};

impl App {
    pub(in crate::app) fn open_duplicate_targets(&mut self) -> Result<()> {
        let targets = self.duplicate_action_paths();
        if targets.is_empty() {
            return Ok(());
        }
        self.jobs.clipboard = None;
        self.open_paths_in_system(targets)
    }
    pub(in crate::app) fn open_duplicate_open_with_overlay(&mut self) {
        let Some(entry) = self.duplicate_focused_entry() else {
            self.status = "Nothing selected".to_string();
            return;
        };
        self.open_open_with_overlay_for_entry(entry);
    }
    pub(in crate::app) fn open_duplicate_rename(&mut self) {
        if self
            .overlays
            .duplicates
            .as_ref()
            .is_some_and(|overlay| !overlay.selected_paths.is_empty())
        {
            self.open_duplicate_bulk_rename();
            return;
        }
        let Some(entry) = self.duplicate_focused_entry() else {
            return;
        };
        self.overlays.rename = Some(RenameOverlay {
            is_dir: false,
            original_name: entry.name.clone(),
            input: entry.name,
            cursor_col: entry.name_key.chars().count(),
            error: None,
        });
    }
    pub(in crate::app) fn open_duplicate_bulk_rename(&mut self) {
        let paths = self.duplicate_action_paths();
        if paths.is_empty() {
            return;
        }
        let items = paths
            .into_iter()
            .map(|path| BulkRenameItem {
                original_name: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| path.display().to_string()),
                is_dir: false,
                path,
            })
            .collect::<Vec<_>>();
        let new_names = items
            .iter()
            .map(|item| item.original_name.clone())
            .collect::<Vec<_>>();
        let count = items.len();
        self.overlays.bulk_rename = Some(BulkRenameOverlay {
            items,
            new_names,
            root: None,
            cursor_line: 0,
            cursor_col: 0,
            preferred_col: 0,
            line_errors: vec![None; count],
        });
    }
    pub(in crate::app) fn open_duplicate_editor_bulk_rename(&mut self) -> Result<()> {
        let paths = self.duplicate_action_paths();
        if paths.is_empty() {
            return Ok(());
        }
        let saved_selection = self.file_browser.selected_paths.clone();
        self.file_browser.selected_paths.clear();
        for path in paths {
            self.file_browser.selected_paths.insert(path);
        }
        let result = self.open_editor_bulk_rename();
        self.file_browser.selected_paths = saved_selection;
        result
    }
    pub(in crate::app) fn open_duplicate_trash_prompt(&mut self) {
        let Some(targets) = self.duplicate_trash_targets_or_status(false) else {
            return;
        };
        let has_trash = targets
            .iter()
            .any(|target| self.trash_target_is_inside_trash(&target.path));
        let has_normal = targets
            .iter()
            .any(|target| !self.trash_target_is_inside_trash(&target.path));
        match (has_trash, has_normal) {
            (true, true) => {
                self.status = "Selection mixes trash and normal files".to_string();
            }
            (true, false) => self.open_trash_prompt_for_explicit_targets(targets, true),
            _ => self.open_trash_prompt_for_explicit_targets(targets, false),
        }
    }
    pub(in crate::app) fn open_duplicate_delete_permanently_prompt(&mut self) {
        if let Some(targets) = self.duplicate_trash_targets_or_status(true) {
            self.open_trash_prompt_for_explicit_targets(targets, true);
        }
    }
    pub(in crate::app) fn duplicate_trash_targets_or_status(
        &mut self,
        permanent: bool,
    ) -> Option<Vec<TrashTarget>> {
        if self.duplicate_loading() {
            self.status = if permanent {
                "Wait for duplicate scan to finish before deleting results".to_string()
            } else {
                "Wait for duplicate scan to finish before trashing results".to_string()
            };
            return None;
        }
        let action_paths = self.duplicate_action_paths();
        let targets = action_paths
            .into_iter()
            .filter_map(|path| {
                let name = path.file_name()?.to_string_lossy().to_string();
                Some(TrashTarget {
                    path,
                    name,
                    is_dir: false,
                })
            })
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return None;
        }
        Some(targets)
    }
    pub(in crate::app) fn duplicate_action_paths(&self) -> Vec<PathBuf> {
        self.overlays
            .duplicates
            .as_ref()
            .map(crate::duplicate_finder::DuplicateFinderState::action_paths)
            .unwrap_or_default()
    }

    pub(crate) fn confirm_duplicate_rename(
        &mut self,
        original_name: String,
        new_name: String,
    ) -> Result<()> {
        let Some(old_path) = self.duplicate_focused_path() else {
            self.overlays.rename = None;
            return Ok(());
        };
        if old_path.file_name().and_then(|name| name.to_str()) != Some(original_name.as_str()) {
            self.overlays.rename = None;
            return Ok(());
        }
        let new_path = old_path
            .parent()
            .map(|parent| parent.join(&new_name))
            .unwrap_or_else(|| PathBuf::from(&new_name));
        if new_path.exists() {
            if let Some(r) = &mut self.overlays.rename {
                r.error = Some(format!("\"{}\" already exists", new_name));
            }
            return Ok(());
        }
        if let Err(error) = fs::rename(&old_path, &new_path) {
            let msg = match error.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    format!("Permission denied renaming \"{}\"", original_name)
                }
                _ => format!("Could not rename: {error}"),
            };
            if let Some(r) = &mut self.overlays.rename {
                r.error = Some(msg);
            }
            return Ok(());
        }
        self.overlays.rename = None;
        self.apply_duplicate_rename_pairs(vec![(old_path, new_path)]);
        self.status = format!("Renamed \"{}\" → \"{}\"", original_name, new_name);
        Ok(())
    }
}

impl App {
    pub fn duplicates_is_open(&self) -> bool {
        self.overlays.duplicates.is_some()
    }

    pub(in crate::app) fn open_duplicate_finder(&mut self) {
        self.clear_selection();
        self.queue_terminal_image_geometry_clear();
        self.clear_wheel_scroll();
        self.overlays.help = false;
        self.overlays.search = None;
        self.jobs.duplicate_token = self.jobs.duplicate_token.wrapping_add(1);
        let cwd = self.file_browser.cwd.clone();
        let show_hidden = self.effective_show_hidden();
        self.overlays.duplicates = Some(crate::duplicate_finder::DuplicateFinderState::new(
            cwd.clone(),
        ));
        let submitted = self
            .jobs
            .scheduler
            .submit_duplicate_scan(DuplicateScanRequest {
                token: self.jobs.duplicate_token,
                cwd,
                show_hidden,
            });
        if let Some(duplicates) = self.overlays.duplicates.as_mut().filter(|_| !submitted) {
            duplicates.loading = false;
            duplicates.error = Some("Duplicate worker unavailable".to_string());
        }
        self.refresh_duplicate_preview();
    }

    pub(in crate::app) fn stop_duplicate_scan_with_partial_results(&mut self) {
        let Some(duplicates) = self
            .overlays
            .duplicates
            .as_mut()
            .filter(|duplicates| duplicates.loading)
        else {
            return;
        };
        self.jobs.duplicate_token = self.jobs.duplicate_token.wrapping_add(1);
        self.jobs.scheduler.cancel_duplicate_scan();
        duplicates.stop_with_partial_results();
        self.status = "Duplicate scan stopped".to_string();
        self.sync_duplicate_scroll();
        self.refresh_duplicate_preview();
    }

    pub(in crate::app) fn close_duplicate_finder(&mut self) {
        self.queue_terminal_image_geometry_clear();
        self.jobs.duplicate_token = self.jobs.duplicate_token.wrapping_add(1);
        self.jobs.scheduler.cancel_duplicate_scan();
        self.overlays.duplicates = None;
        self.refresh_preview();
        self.clear_wheel_scroll();
    }

    pub(in crate::app) fn apply_duplicate_batch(
        &mut self,
        batch: crate::duplicate_finder::DuplicateScanBatch,
    ) {
        let became_non_empty = self
            .overlays
            .duplicates
            .as_mut()
            .is_some_and(|duplicates| duplicates.apply_batch(batch));
        if became_non_empty {
            self.queue_terminal_image_geometry_clear();
        }
        self.sync_duplicate_scroll();
        if became_non_empty {
            self.refresh_duplicate_preview();
        }
    }

    pub(in crate::app) fn apply_duplicate_result(
        &mut self,
        result: Result<crate::duplicate_finder::DuplicateScanResult, String>,
    ) {
        let file_presence_changed = self
            .overlays
            .duplicates
            .as_mut()
            .is_some_and(|duplicates| duplicates.apply_result(result));
        if file_presence_changed {
            self.queue_terminal_image_geometry_clear();
        }
        self.sync_duplicate_scroll();
        self.refresh_duplicate_preview();
    }

    pub(in crate::app) fn duplicate_focused_path(&self) -> Option<PathBuf> {
        self.overlays
            .duplicates
            .as_ref()
            .and_then(crate::duplicate_finder::DuplicateFinderState::focused_path)
    }

    pub fn duplicate_focused_entry(&self) -> Option<Entry> {
        self.overlays
            .duplicates
            .as_ref()
            .and_then(crate::duplicate_finder::DuplicateFinderState::focused_entry)
    }

    pub(in crate::app) fn active_preview_entry(&self) -> Option<Entry> {
        if let Some(duplicates) = &self.overlays.duplicates {
            return duplicates
                .preview_visible
                .then(|| duplicates.focused_entry())
                .flatten();
        }
        self.selected_entry().cloned()
    }

    pub fn duplicate_rows(&self, max_rows: usize) -> Vec<DuplicateRow> {
        self.overlays
            .duplicates
            .as_ref()
            .map(|duplicates| duplicates.rows(max_rows))
            .unwrap_or_default()
    }

    pub fn duplicate_group_count(&self) -> usize {
        self.overlays
            .duplicates
            .as_ref()
            .map_or(0, |duplicates| duplicates.groups.len())
    }

    pub fn duplicate_file_count(&self) -> usize {
        self.overlays
            .duplicates
            .as_ref()
            .map_or(0, crate::duplicate_finder::DuplicateFinderState::file_count)
    }

    pub fn duplicate_stats(&self) -> Option<crate::duplicate_finder::DuplicateScanStats> {
        self.overlays
            .duplicates
            .as_ref()
            .map(|duplicates| duplicates.stats)
    }

    pub fn duplicate_loading(&self) -> bool {
        self.overlays
            .duplicates
            .as_ref()
            .is_some_and(|duplicates| duplicates.loading)
    }

    pub fn duplicate_partial(&self) -> bool {
        self.overlays
            .duplicates
            .as_ref()
            .is_some_and(|duplicates| duplicates.partial)
    }

    pub fn duplicate_error(&self) -> Option<&str> {
        self.overlays
            .duplicates
            .as_ref()
            .and_then(|duplicates| duplicates.error.as_deref())
    }

    pub fn duplicate_preview_visible(&self) -> bool {
        self.overlays
            .duplicates
            .as_ref()
            .is_some_and(|duplicates| duplicates.preview_visible)
    }

    pub(in crate::app) fn duplicate_preview_rendered(&self) -> bool {
        self.overlays.duplicates.is_some()
            && self.duplicate_preview_visible()
            && self.input.frame_state.preview_panel.is_some()
    }

    pub fn duplicate_cwd(&self) -> Option<&Path> {
        self.overlays
            .duplicates
            .as_ref()
            .map(|duplicates| duplicates.cwd.as_path())
    }

    pub fn duplicate_scroll_top(&self) -> usize {
        self.overlays
            .duplicates
            .as_ref()
            .map_or(0, |duplicates| duplicates.scroll)
    }

    pub(crate) fn apply_duplicate_rename_pairs(&mut self, pairs: Vec<(PathBuf, PathBuf)>) {
        if pairs.is_empty() {
            return;
        }
        if let Some(duplicates) = &mut self.overlays.duplicates {
            duplicates.apply_rename_pairs(pairs);
            self.refresh_duplicate_preview();
        }
    }

    pub(in crate::app) fn remove_duplicate_paths(&mut self, paths: &[PathBuf]) {
        if paths.is_empty() || self.overlays.duplicates.is_none() {
            return;
        }
        if let Some(duplicates) = &mut self.overlays.duplicates {
            duplicates.remove_paths(paths);
        }
        self.queue_terminal_image_geometry_clear();
        self.sync_duplicate_scroll();
        self.refresh_duplicate_preview();
    }

    pub(in crate::app) fn refresh_duplicate_preview(&mut self) {
        let Some(path) = self.duplicate_focused_path() else {
            return;
        };
        let should_refresh = self.overlays.duplicates.as_ref().is_some_and(|duplicates| {
            duplicates.preview_visible && duplicates.preview_path.as_ref() != Some(&path)
        });
        if !should_refresh {
            return;
        }
        if let Some(duplicates) = &mut self.overlays.duplicates {
            duplicates.preview_path = Some(path);
        }
        self.refresh_preview();
    }
}
