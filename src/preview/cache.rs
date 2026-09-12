use super::state::{CachedPreview, PreviewCacheKey, PreviewLineCountKey, PreviewState};
use super::{PreviewContent, PreviewRequestOptions};
use crate::{
    file_classification::{FileClass, inspect_entry_cached},
    fs::Entry,
};
use std::{path::PathBuf, time::SystemTime};

pub(crate) const PREVIEW_CACHE_LIMIT: usize = 24;
pub(crate) const PREVIEW_LINE_COUNT_CACHE_LIMIT: usize = 64;

impl PreviewState {
    /// Look up a cached preview, preferring complete renders over partial ones.
    pub(crate) fn cached_preview(
        &self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        code_line_limit: usize,
        ffmpeg_available: bool,
    ) -> Option<PreviewContent> {
        let ffmpeg_available = ffmpeg_available && entry_uses_ffmpeg(entry);
        let complete_key = PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            ffmpeg_available,
            code_line_limit,
            code_render_limit: code_line_limit,
        };
        if let Some(cached) = self.result_cache.get(&complete_key)
            && cached.size == entry.size
            && cached.modified == entry.modified
        {
            return Some(cached.preview.clone());
        }

        self.result_cache
            .iter()
            .find(|(key, cached)| {
                key.path == entry.path
                    && key.variant == *variant
                    && key.ffmpeg_available == ffmpeg_available
                    && key.code_line_limit == code_line_limit
                    && cached.size == entry.size
                    && cached.modified == entry.modified
            })
            .map(|(_, cached)| cached.preview.clone())
    }

    pub(crate) fn stale_cached_preview(
        &self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        code_line_limit: usize,
        ffmpeg_available: bool,
    ) -> Option<PreviewContent> {
        let ffmpeg_available = ffmpeg_available && entry_uses_ffmpeg(entry);
        let complete_key = PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            ffmpeg_available,
            code_line_limit,
            code_render_limit: code_line_limit,
        };
        if let Some(cached) = self.result_cache.get(&complete_key) {
            return Some(cached.preview.clone());
        }

        self.result_cache
            .iter()
            .find(|(key, _)| {
                key.path == entry.path
                    && key.variant == *variant
                    && key.ffmpeg_available == ffmpeg_available
                    && key.code_line_limit == code_line_limit
            })
            .map(|(_, cached)| cached.preview.clone())
    }

    pub(crate) fn remember_preview(
        &mut self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        code_line_limit: usize,
        code_render_limit: usize,
        ffmpeg_available: bool,
        preview: &PreviewContent,
    ) {
        let key = PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            ffmpeg_available: ffmpeg_available && entry_uses_ffmpeg(entry),
            code_line_limit,
            code_render_limit,
        };
        self.result_cache.insert(
            key.clone(),
            CachedPreview {
                size: entry.size,
                modified: entry.modified,
                preview: preview.clone(),
            },
        );
        self.result_order.retain(|cached| cached != &key);
        self.result_order.push_back(key);

        while self.result_order.len() > PREVIEW_CACHE_LIMIT {
            if let Some(stale_key) = self.result_order.pop_front() {
                self.result_cache.remove(&stale_key);
            }
        }
    }

    pub(crate) fn remember_line_count(
        &mut self,
        path: PathBuf,
        size: u64,
        modified: Option<SystemTime>,
        total_lines: usize,
    ) {
        let key = PreviewLineCountKey {
            path,
            size,
            modified,
        };
        self.line_count_cache
            .insert(key.clone(), total_lines.max(1));
        self.line_count_order.retain(|cached| cached != &key);
        self.line_count_order.push_back(key);

        while self.line_count_order.len() > PREVIEW_LINE_COUNT_CACHE_LIMIT {
            if let Some(stale_key) = self.line_count_order.pop_front() {
                self.line_count_cache.remove(&stale_key);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn has_cached_preview_for_path(&self, path: &std::path::Path) -> bool {
        self.result_cache.keys().any(|key| key.path == path)
    }
}

fn entry_uses_ffmpeg(entry: &Entry) -> bool {
    matches!(
        inspect_entry_cached(entry).builtin_class,
        FileClass::Audio | FileClass::Video
    )
}
