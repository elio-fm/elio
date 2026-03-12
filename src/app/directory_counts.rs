use super::*;
use std::path::Path;

impl App {
    pub(crate) fn directory_item_count_label(&self, entry: &Entry) -> Option<String> {
        self.directory_item_count(entry).map(format_item_count)
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
        self.directory_item_count_cache
            .insert(key.clone(), item_count);
        self.directory_item_count_order
            .retain(|queued| queued != &key);
        self.directory_item_count_order.push_back(key.clone());

        while self.directory_item_count_order.len() > DIRECTORY_ITEM_COUNT_CACHE_LIMIT {
            if let Some(stale_key) = self.directory_item_count_order.pop_front() {
                self.directory_item_count_cache.remove(&stale_key);
            }
        }
    }

    pub(super) fn queue_visible_directory_item_counts(&mut self) {
        let viewport = DirectoryCountViewport {
            fingerprint: self.directory_runtime.fingerprint,
            scroll_row: self.scroll_row,
            cols: self.frame_state.metrics.cols.max(1),
            rows_visible: self.frame_state.metrics.rows_visible.max(1),
        };
        if self.directory_count_viewport == Some(viewport) {
            return;
        }
        self.directory_count_viewport = Some(viewport);

        let requests = self
            .visible_entry_indices()
            .into_iter()
            .filter_map(|index| self.entries.get(index))
            .filter(|entry| entry.is_dir())
            .filter_map(|entry| self.directory_item_count_request_for(entry))
            .collect::<Vec<_>>();

        for request in requests {
            let _ = self.scheduler.submit_directory_item_count(request);
        }
    }

    pub(super) fn should_redraw_for_directory_item_count(
        &self,
        path: &Path,
        modified: Option<SystemTime>,
        show_hidden: bool,
    ) -> bool {
        if self.show_hidden != show_hidden {
            return false;
        }

        self.visible_entry_indices().into_iter().any(|index| {
            self.entries.get(index).is_some_and(|entry| {
                entry.is_dir() && entry.path == path && entry.modified == modified
            })
        })
    }

    fn directory_item_count(&self, entry: &Entry) -> Option<usize> {
        let key = self.directory_item_count_key_for(entry)?;
        self.directory_item_count_cache.get(&key).copied().flatten()
    }

    fn directory_item_count_request_for(
        &self,
        entry: &Entry,
    ) -> Option<jobs::DirectoryItemCountRequest> {
        let key = self.directory_item_count_key_for(entry)?;
        if self.directory_item_count_cache.contains_key(&key) {
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
            show_hidden: self.show_hidden,
        })
    }

    pub(super) fn visible_entry_indices(&self) -> Vec<usize> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let rows_visible = self.frame_state.metrics.rows_visible.max(1);
        let start = self.scroll_row.saturating_mul(cols);
        let limit = rows_visible.saturating_mul(cols);
        (start..self.entries.len()).take(limit).collect()
    }
}
