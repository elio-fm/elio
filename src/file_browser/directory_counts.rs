use crate::filesystem::{DirectoryFingerprint, Entry};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

const DIRECTORY_ITEM_COUNT_CACHE_LIMIT: usize = 128;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DirectoryItemCountKey {
    pub(crate) path: PathBuf,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) show_hidden: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryCountViewport {
    pub(crate) fingerprint: DirectoryFingerprint,
    pub(crate) scroll_row: usize,
    pub(crate) cols: usize,
    pub(crate) rows_visible: usize,
    pub(crate) show_hidden: bool,
}

impl super::FileBrowserState {
    pub(crate) fn cache_directory_item_count(
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

    pub(crate) fn update_directory_count_viewport(
        &mut self,
        cols: usize,
        rows_visible: usize,
        show_hidden: bool,
        ready_at: Instant,
    ) {
        let viewport = DirectoryCountViewport {
            fingerprint: self.directory_runtime.fingerprint,
            scroll_row: self.scroll_row,
            cols: cols.max(1),
            rows_visible: rows_visible.max(1),
            show_hidden,
        };
        if self.directory_count_viewport == Some(viewport) {
            return;
        }
        self.directory_count_viewport = Some(viewport);
        self.directory_item_count_ready_at.get_or_insert(ready_at);
    }

    pub(crate) fn directory_count_timer_ready(&mut self, now: Instant) -> bool {
        let Some(deadline) = self.directory_item_count_ready_at else {
            return false;
        };
        if now < deadline {
            return false;
        }
        self.directory_item_count_ready_at = None;
        true
    }

    pub(crate) fn pending_directory_count_timer(&self, now: Instant) -> Option<Duration> {
        self.directory_item_count_ready_at
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    pub(crate) fn should_redraw_for_directory_item_count(
        &self,
        path: &Path,
        modified: Option<SystemTime>,
        show_hidden: bool,
        effective_show_hidden: bool,
        cols: usize,
        rows_visible: usize,
    ) -> bool {
        if effective_show_hidden != show_hidden {
            return false;
        }

        self.visible_entry_indices(cols, rows_visible)
            .into_iter()
            .any(|index| {
                self.entries.get(index).is_some_and(|entry| {
                    entry.is_dir() && entry.path == path && entry.modified == modified
                })
            })
    }

    pub(crate) fn directory_item_count(&self, entry: &Entry, show_hidden: bool) -> Option<usize> {
        let key = Self::directory_item_count_key(entry, show_hidden)?;
        self.directory_item_count_cache.get(&key).copied().flatten()
    }

    pub(crate) fn directory_item_count_key(
        entry: &Entry,
        show_hidden: bool,
    ) -> Option<DirectoryItemCountKey> {
        entry.is_dir().then(|| DirectoryItemCountKey {
            path: entry.path.clone(),
            modified: entry.modified,
            show_hidden,
        })
    }

    pub(crate) fn directory_item_count_is_cached(&self, key: &DirectoryItemCountKey) -> bool {
        self.directory_item_count_cache.contains_key(key)
    }

    pub(crate) fn visible_entry_indices(&self, cols: usize, rows_visible: usize) -> Vec<usize> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        let cols = cols.max(1);
        let rows_visible = rows_visible.max(1);
        let start = self.scroll_row.saturating_mul(cols);
        let limit = rows_visible.saturating_mul(cols);
        (start..self.entries.len()).take(limit).collect()
    }
}
