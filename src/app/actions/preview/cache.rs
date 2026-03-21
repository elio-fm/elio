use super::*;
use crate::preview::{PreviewContent, PreviewRequestOptions};

impl App {
    pub(in crate::app) fn cached_preview_for(
        &self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
    ) -> Option<PreviewContent> {
        let cached = self.preview_state.result_cache.get(&PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            code_line_limit: self.preview_code_line_limit_for_entry(entry),
        })?;
        if cached.size == entry.size && cached.modified == entry.modified {
            Some(cached.preview.clone())
        } else {
            None
        }
    }

    pub(super) fn stale_cached_preview_for(
        &self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
    ) -> Option<PreviewContent> {
        self.preview_state
            .result_cache
            .get(&PreviewCacheKey {
                path: entry.path.clone(),
                variant: variant.clone(),
                code_line_limit: self.preview_code_line_limit_for_entry(entry),
            })
            .map(|cached| cached.preview.clone())
    }

    #[cfg(test)]
    pub(in crate::app) fn cache_preview_result(
        &mut self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        preview: &PreviewContent,
    ) {
        let code_line_limit = self.preview_code_line_limit_for_entry(entry);
        self.cache_preview_result_with_code_line_limit(entry, variant, code_line_limit, preview);
    }

    pub(in crate::app) fn cache_preview_result_with_code_line_limit(
        &mut self,
        entry: &Entry,
        variant: &PreviewRequestOptions,
        code_line_limit: usize,
        preview: &PreviewContent,
    ) {
        let key = PreviewCacheKey {
            path: entry.path.clone(),
            variant: variant.clone(),
            code_line_limit,
        };
        self.preview_state.result_cache.insert(
            key.clone(),
            CachedPreview {
                size: entry.size,
                modified: entry.modified,
                preview: preview.clone(),
            },
        );
        self.preview_state
            .result_order
            .retain(|cached| cached != &key);
        self.preview_state.result_order.push_back(key);

        while self.preview_state.result_order.len() > PREVIEW_CACHE_LIMIT {
            if let Some(stale_key) = self.preview_state.result_order.pop_front() {
                self.preview_state.result_cache.remove(&stale_key);
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
        self.preview_state
            .line_count_cache
            .insert(key.clone(), total_lines.max(1));
        self.preview_state
            .line_count_order
            .retain(|cached| cached != &key);
        self.preview_state.line_count_order.push_back(key);

        while self.preview_state.line_count_order.len() > PREVIEW_LINE_COUNT_CACHE_LIMIT {
            if let Some(stale_key) = self.preview_state.line_count_order.pop_front() {
                self.preview_state.line_count_cache.remove(&stale_key);
            }
        }
    }

    #[cfg(test)]
    pub(in crate::app) fn has_cached_preview_for_path(&self, path: &std::path::Path) -> bool {
        self.preview_state
            .result_cache
            .keys()
            .any(|key| key.path == path)
    }
}
