use super::*;

impl App {
    pub(crate) fn process_preview_prefetch_timers(&mut self) -> bool {
        let Some(deadline) = self.preview_state.prefetch_ready_at else {
            return false;
        };
        if Instant::now() < deadline
            || self.preview_state.deferred_refresh_at.is_some()
            || self.browser_wheel_burst_active()
        {
            return false;
        }

        self.preview_state.prefetch_ready_at = None;
        self.prefetch_nearby_comic_pages();
        self.prefetch_nearby_epub_sections();
        self.prefetch_nearby_previews();
        false
    }

    pub(crate) fn pending_preview_prefetch_timer(&self) -> Option<std::time::Duration> {
        self.preview_state
            .prefetch_ready_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    pub(in crate::app) fn schedule_preview_prefetch(&mut self) {
        self.preview_state.prefetch_ready_at = self
            .selected_entry()
            .map(|_| Instant::now() + PREVIEW_PREFETCH_IDLE_DELAY);
    }

    fn prefetch_nearby_previews(&mut self) {
        let mut queued = 0;
        for offset in [1isize, -1, 2, -2, 3, -3] {
            if queued >= PREVIEW_PREFETCH_LIMIT {
                break;
            }

            let target = self.selected as isize + offset;
            if target < 0 {
                continue;
            }
            let Some(entry) = self.entries.get(target as usize).cloned() else {
                continue;
            };
            let variant = self.preview_request_options_for_entry(&entry);
            let work_class = preview_work_class(&entry, &variant);
            if !should_build_preview_in_background(&entry)
                || work_class == PreviewWorkClass::Heavy
                || self.cached_preview_for(&entry, &variant).is_some()
            {
                continue;
            }

            let code_line_limit = self.preview_code_line_limit_for_entry(&entry);
            let request = PreviewRequest {
                token: self.preview_state.token,
                entry,
                variant,
                code_line_limit,
                priority: PreviewPriority::Low,
                work_class: PreviewWorkClass::Light,
            };
            if self.scheduler.submit_preview(request) {
                queued += 1;
            }
        }
    }
}
