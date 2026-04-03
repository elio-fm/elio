use super::*;
use crate::preview::{PreviewContent, PreviewRequestOptions};

impl App {
    /// Look up a cached preview, preferring complete renders
    /// (`code_render_limit == code_line_limit`) over partial ones.
    pub(in crate::app) fn cached_preview_for(
        &self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
    ) -> Option<PreviewContent> {
        let code_line_limit = self.preview_code_line_limit_for_entry(entry);

        // Try the complete render first.
        let complete_key = PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            code_line_limit,
            code_render_limit: code_line_limit,
        };
        if let Some(cached) = self.preview.state.result_cache.get(&complete_key)
            && cached.size == entry.size
            && cached.modified == entry.modified
        {
            return Some(cached.preview.clone());
        }

        // Fall back to any matching entry (may be a partial render).
        let partial_key = PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            code_line_limit,
            code_render_limit: 0, // placeholder — will search by prefix below
        };
        let _ = partial_key; // not used directly; iterate instead
        self.preview
            .state
            .result_cache
            .iter()
            .find(|(key, cached)| {
                key.path == entry.path
                    && key.variant == *variant
                    && key.code_line_limit == code_line_limit
                    && cached.size == entry.size
                    && cached.modified == entry.modified
            })
            .map(|(_, cached)| cached.preview.clone())
    }

    pub(super) fn stale_cached_preview_for(
        &self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
    ) -> Option<PreviewContent> {
        let code_line_limit = self.preview_code_line_limit_for_entry(entry);

        // Try the complete render first (prefer it even when stale).
        let complete_key = PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            code_line_limit,
            code_render_limit: code_line_limit,
        };
        if let Some(cached) = self.preview.state.result_cache.get(&complete_key) {
            return Some(cached.preview.clone());
        }

        // Fall back to any matching partial.
        self.preview
            .state
            .result_cache
            .iter()
            .find(|(key, _)| {
                key.path == entry.path
                    && key.variant == *variant
                    && key.code_line_limit == code_line_limit
            })
            .map(|(_, cached)| cached.preview.clone())
    }

    #[cfg(test)]
    pub(in crate::app) fn cache_preview_result(
        &mut self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        preview: &PreviewContent,
    ) {
        let code_line_limit = self.preview_code_line_limit_for_entry(entry);
        self.cache_preview_result_with_limits(
            entry,
            variant,
            code_line_limit,
            code_line_limit,
            preview,
        );
    }

    #[cfg(test)]
    pub(in crate::app) fn cache_preview_result_with_code_line_limit(
        &mut self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        code_line_limit: usize,
        preview: &PreviewContent,
    ) {
        // For backwards-compatibility test callers that don't know the render limit,
        // use the limit stored in the preview itself (if any), otherwise assume complete.
        let code_render_limit = preview.incremental_render_limit.unwrap_or(code_line_limit);
        self.cache_preview_result_with_limits(
            entry,
            variant,
            code_line_limit,
            code_render_limit,
            preview,
        );
    }

    pub(in crate::app) fn cache_preview_result_with_limits(
        &mut self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        code_line_limit: usize,
        code_render_limit: usize,
        preview: &PreviewContent,
    ) {
        let key = PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            code_line_limit,
            code_render_limit,
        };
        self.preview.state.result_cache.insert(
            key.clone(),
            CachedPreview {
                size: entry.size,
                modified: entry.modified,
                preview: preview.clone(),
            },
        );
        self.preview
            .state
            .result_order
            .retain(|cached| cached != &key);
        self.preview.state.result_order.push_back(key);

        while self.preview.state.result_order.len() > PREVIEW_CACHE_LIMIT {
            if let Some(stale_key) = self.preview.state.result_order.pop_front() {
                self.preview.state.result_cache.remove(&stale_key);
            }
        }
    }

    pub(in crate::app) fn cache_preview_line_count(
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
        self.preview
            .state
            .line_count_cache
            .insert(key.clone(), total_lines.max(1));
        self.preview
            .state
            .line_count_order
            .retain(|cached| cached != &key);
        self.preview.state.line_count_order.push_back(key);

        while self.preview.state.line_count_order.len() > PREVIEW_LINE_COUNT_CACHE_LIMIT {
            if let Some(stale_key) = self.preview.state.line_count_order.pop_front() {
                self.preview.state.line_count_cache.remove(&stale_key);
            }
        }
    }

    #[cfg(test)]
    pub(in crate::app) fn has_cached_preview_for_path(&self, path: &std::path::Path) -> bool {
        self.preview
            .state
            .result_cache
            .keys()
            .any(|key| key.path == path)
    }
}
