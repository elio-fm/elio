use super::*;

impl App {
    pub(in crate::app) fn refresh_preview(&mut self) {
        self.preview_state.deferred_refresh_at = None;
        self.preview_state.prefetch_ready_at = None;
        self.sync_comic_preview_selection();
        self.sync_epub_preview_selection();
        self.sync_pdf_preview_selection();
        self.sync_image_preview_selection_activation();
        self.preview_state.token = self.preview_state.token.wrapping_add(1);
        let preview_options = self.current_preview_request_options();
        self.preview_state.content = match self.selected_entry().cloned() {
            Some(entry) if self.should_defer_static_image_preview(&entry) => {
                self.preview_state.load_state = None;
                PreviewContent::new(PreviewKind::Image, Vec::new()).with_detail(
                    self.static_image_preview_detail(&entry)
                        .unwrap_or("Image preview"),
                )
            }
            Some(entry) if self.should_defer_pdf_document_preview(&entry) => {
                self.preview_state.load_state = None;
                self.cached_preview_for(&entry, &preview_options)
                    .or_else(|| self.stale_cached_preview_for(&entry, &preview_options))
                    .unwrap_or_else(|| {
                        PreviewContent::new(PreviewKind::Document, Vec::new())
                            .with_detail("PDF document")
                    })
            }
            Some(entry) => {
                if let Some(preview) = self.cached_preview_for(&entry, &preview_options) {
                    self.preview_state.metrics.cache_hits += 1;
                    self.preview_state.load_state = None;
                    preview
                } else if let Some(stale_preview) =
                    self.stale_cached_preview_for(&entry, &preview_options)
                {
                    self.preview_state.metrics.cache_misses += 1;
                    let loading_path = entry.path.clone();
                    let work_class = preview_work_class(&entry, &preview_options);
                    let code_line_limit = self.preview_code_line_limit_for_entry(&entry);
                    let request = PreviewRequest {
                        token: self.preview_state.token,
                        entry,
                        variant: preview_options.clone(),
                        code_line_limit,
                        priority: PreviewPriority::High,
                        work_class,
                    };
                    if !self.scheduler.submit_preview(request) {
                        self.preview_state.load_state = None;
                        stale_preview.with_status_note("Refresh unavailable")
                    } else {
                        self.preview_state.load_state =
                            Some(PreviewLoadState::Refreshing(loading_path));
                        stale_preview.with_status_note("Refreshing in background")
                    }
                } else {
                    self.preview_state.metrics.cache_misses += 1;
                    let placeholder = self.apply_current_epub_loading_navigation(
                        self.apply_current_comic_loading_navigation(loading_preview_for(
                            &entry,
                            &preview_options,
                        )),
                    );
                    let loading_path = entry.path.clone();
                    let work_class = preview_work_class(&entry, &preview_options);
                    let code_line_limit = self.preview_code_line_limit_for_entry(&entry);
                    let request = PreviewRequest {
                        token: self.preview_state.token,
                        entry,
                        variant: preview_options.clone(),
                        code_line_limit,
                        priority: PreviewPriority::High,
                        work_class,
                    };
                    if !self.scheduler.submit_preview(request) {
                        self.preview_state.load_state = None;
                        PreviewContent::placeholder("Preview worker unavailable")
                    } else {
                        self.preview_state.load_state =
                            Some(PreviewLoadState::Placeholder(loading_path));
                        placeholder
                    }
                }
            }
            None => {
                self.preview_state.load_state = None;
                PreviewContent::placeholder("No selection")
            }
        };
        self.apply_current_comic_preview_metadata();
        self.apply_current_epub_preview_metadata();
        self.sync_current_preview_line_count();
        self.preview_state.scroll = 0;
        self.preview_state.horizontal_scroll = 0;
        self.sync_preview_scroll();
        self.refresh_static_image_preloads();
        self.schedule_preview_prefetch();
    }

    pub(crate) fn process_preview_refresh_timers(&mut self) -> bool {
        let Some(deadline) = self.preview_state.deferred_refresh_at else {
            return false;
        };
        if Instant::now() < deadline {
            return false;
        }
        self.refresh_preview();
        true
    }

    pub(crate) fn pending_preview_refresh_timer(&self) -> Option<std::time::Duration> {
        self.preview_state
            .deferred_refresh_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    pub(in crate::app) fn apply_preview_line_count_result(
        &mut self,
        path: &std::path::Path,
        size: u64,
        modified: Option<SystemTime>,
        total_lines: Option<usize>,
    ) -> bool {
        let key = PreviewLineCountKey {
            path: path.to_path_buf(),
            size,
            modified,
        };
        self.preview_state.pending_line_counts.remove(&key);
        let Some(total_lines) = total_lines else {
            let should_clear_pending = self.selected_entry().is_some_and(|entry| {
                entry.path == key.path && entry.size == key.size && entry.modified == key.modified
            });
            if should_clear_pending {
                self.preview_state
                    .content
                    .set_total_line_count_pending(false);
                return true;
            }
            return false;
        };
        self.cache_preview_line_count(key.path.clone(), key.size, key.modified, total_lines);

        let is_current_entry = self.selected_entry().is_some_and(|entry| {
            entry.path == key.path && entry.size == key.size && entry.modified == key.modified
        });
        if is_current_entry {
            self.preview_state
                .content
                .apply_total_line_count(total_lines);
            return true;
        }
        false
    }

    pub(in crate::app) fn sync_current_preview_line_count(&mut self) {
        let needs_total_line_count = self.preview_state.content.needs_total_line_count();
        let Some(entry) = self.selected_entry().cloned() else {
            return;
        };
        if !needs_total_line_count {
            self.preview_state
                .content
                .set_total_line_count_pending(false);
            return;
        }

        let key = PreviewLineCountKey {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
        };
        if let Some(total_lines) = self.preview_state.line_count_cache.get(&key).copied() {
            self.preview_state
                .content
                .apply_total_line_count(total_lines);
            return;
        }

        let pending = self.preview_state.pending_line_counts.contains(&key)
            || self
                .scheduler
                .submit_preview_line_count(PreviewLineCountRequest {
                    path: entry.path,
                    size: entry.size,
                    modified: entry.modified,
                });
        if pending {
            self.preview_state.pending_line_counts.insert(key);
        }
        self.preview_state
            .content
            .set_total_line_count_pending(pending);
    }
}
