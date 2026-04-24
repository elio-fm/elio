use super::*;
use std::{path::Path, time::Instant};

impl App {
    pub(crate) fn directory_item_count_label(&self, entry: &Entry) -> Option<String> {
        self.directory_item_count_value(entry)
            .map(format_item_count)
    }

    pub(crate) fn directory_item_count_value(&self, entry: &Entry) -> Option<usize> {
        self.directory_item_count(entry)
    }

    pub(super) fn cache_directory_item_count(
        &mut self,
        path: PathBuf,
        modified: Option<SystemTime>,
        show_hidden: bool,
        item_count: Option<usize>,
    ) {
        let key = DirectoryItemCountKey {
            path,
            modified,
            show_hidden,
        };
        self.navigation
            .directory_item_count_cache
            .insert(key.clone(), item_count);
        self.navigation
            .directory_item_count_order
            .retain(|queued| queued != &key);
        self.navigation
            .directory_item_count_order
            .push_back(key.clone());

        while self.navigation.directory_item_count_order.len() > DIRECTORY_ITEM_COUNT_CACHE_LIMIT {
            if let Some(stale_key) = self.navigation.directory_item_count_order.pop_front() {
                self.navigation
                    .directory_item_count_cache
                    .remove(&stale_key);
            }
        }
    }

    pub(super) fn queue_visible_directory_item_counts(&mut self) {
        let viewport = DirectoryCountViewport {
            fingerprint: self.navigation.directory_runtime.fingerprint,
            scroll_row: self.navigation.scroll_row,
            cols: self.input.frame_state.metrics.cols.max(1),
            rows_visible: self.input.frame_state.metrics.rows_visible.max(1),
            show_hidden: self.effective_show_hidden(),
        };
        if self.navigation.directory_count_viewport == Some(viewport) {
            return;
        }
        self.navigation.directory_count_viewport = Some(viewport);
        self.navigation.directory_item_count_ready_at =
            Some(Instant::now() + DIRECTORY_ITEM_COUNT_IDLE_DELAY);
    }

    pub(crate) fn process_directory_item_count_timer(&mut self) -> bool {
        let Some(deadline) = self.navigation.directory_item_count_ready_at else {
            return false;
        };
        if Instant::now() < deadline
            || self.browser_wheel_burst_active()
            || self.preview.state.deferred_refresh_at.is_some()
        {
            return false;
        }

        self.navigation.directory_item_count_ready_at = None;
        self.submit_visible_directory_item_counts();
        false
    }

    pub(crate) fn pending_directory_item_count_timer(&self) -> Option<std::time::Duration> {
        self.navigation
            .directory_item_count_ready_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    fn submit_visible_directory_item_counts(&mut self) {
        let requests = self
            .visible_entry_indices()
            .into_iter()
            .filter_map(|index| {
                self.navigation.entries.get(index).and_then(|entry| {
                    entry.is_dir().then_some((
                        index.abs_diff(self.navigation.selected),
                        index,
                        entry,
                    ))
                })
            })
            .collect::<Vec<_>>();
        let mut requests = requests
            .into_iter()
            .filter_map(|(distance, index, entry)| {
                self.directory_item_count_request_for(entry)
                    .map(|request| (distance, index, request))
            })
            .collect::<Vec<_>>();
        requests.sort_by_key(|(distance, index, _)| (*distance, *index));

        for (_, _, request) in requests {
            let _ = self.jobs.scheduler.submit_directory_item_count(request);
        }
    }

    pub(super) fn should_redraw_for_directory_item_count(
        &self,
        path: &Path,
        modified: Option<SystemTime>,
        show_hidden: bool,
    ) -> bool {
        if self.effective_show_hidden() != show_hidden {
            return false;
        }

        self.visible_entry_indices().into_iter().any(|index| {
            self.navigation.entries.get(index).is_some_and(|entry| {
                entry.is_dir() && entry.path == path && entry.modified == modified
            })
        })
    }

    fn directory_item_count(&self, entry: &Entry) -> Option<usize> {
        let key = self.directory_item_count_key_for(entry)?;
        self.navigation
            .directory_item_count_cache
            .get(&key)
            .copied()
            .flatten()
    }

    fn directory_item_count_request_for(
        &self,
        entry: &Entry,
    ) -> Option<jobs::DirectoryItemCountRequest> {
        let key = self.directory_item_count_key_for(entry)?;
        if self
            .navigation
            .directory_item_count_cache
            .contains_key(&key)
        {
            return None;
        }
        Some(jobs::DirectoryItemCountRequest {
            path: key.path,
            modified: key.modified,
            show_hidden: key.show_hidden,
        })
    }

    fn directory_item_count_key_for(&self, entry: &Entry) -> Option<DirectoryItemCountKey> {
        entry.is_dir().then(|| DirectoryItemCountKey {
            path: entry.path.clone(),
            modified: entry.modified,
            show_hidden: self.effective_show_hidden(),
        })
    }

    pub(super) fn visible_entry_indices(&self) -> Vec<usize> {
        if self.navigation.entries.is_empty() {
            return Vec::new();
        }

        let cols = self.input.frame_state.metrics.cols.max(1);
        let rows_visible = self.input.frame_state.metrics.rows_visible.max(1);
        let start = self.navigation.scroll_row.saturating_mul(cols);
        let limit = rows_visible.saturating_mul(cols);
        (start..self.navigation.entries.len()).take(limit).collect()
    }
}
