use super::*;
use crate::app::FileClass;
use crate::file_info;
use crate::preview::{PreviewWorkClass, preview_work_class, should_build_preview_in_background};

const AUDIO_ENTRY_PREFETCH_OFFSETS: [isize; 4] = [1, -1, 2, -2];

impl App {
    pub(crate) fn process_preview_prefetch_timers(&mut self) -> bool {
        let Some(deadline) = self.preview.state.prefetch_ready_at else {
            return false;
        };
        if Instant::now() < deadline
            || self.preview.state.deferred_refresh_at.is_some()
            || self.browser_wheel_burst_active()
        {
            return false;
        }

        self.preview.state.prefetch_ready_at = None;
        self.prefetch_nearby_comic_pages();
        self.prefetch_nearby_comic_entries();
        self.prefetch_nearby_epub_sections();
        self.prefetch_nearby_audio_previews();
        self.prefetch_nearby_previews();
        false
    }

    pub(crate) fn pending_preview_prefetch_timer(&self) -> Option<std::time::Duration> {
        self.preview
            .state
            .prefetch_ready_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    pub(in crate::app) fn schedule_preview_prefetch(&mut self) {
        self.preview.state.prefetch_ready_at = self
            .selected_entry()
            .map(|_| Instant::now() + PREVIEW_PREFETCH_IDLE_DELAY);
    }

    fn prefetch_nearby_previews(&mut self) {
        let mut queued = 0;
        for offset in [1isize, -1, 2, -2, 3, -3] {
            if queued >= PREVIEW_PREFETCH_LIMIT {
                break;
            }

            let target = self.navigation.selected as isize + offset;
            if target < 0 {
                continue;
            }
            let Some(entry) = self.navigation.entries.get(target as usize).cloned() else {
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

            let request = self.build_full_preview_request(
                entry,
                variant,
                PreviewPriority::Low,
                PreviewWorkClass::Light,
            );
            if self.jobs.scheduler.submit_preview(request) {
                queued += 1;
            }
        }
    }

    pub(in crate::app) fn prefetch_nearby_audio_previews(&mut self) {
        let Some(current_entry) = self.selected_entry() else {
            return;
        };
        if !is_audio_entry(current_entry) {
            return;
        }
        let current_variant = self.current_preview_request_options();
        if self
            .cached_preview_for(current_entry, &current_variant)
            .is_none()
        {
            return;
        }

        for entry in self.nearby_audio_candidates() {
            let variant = self.preview_request_options_for_entry(&entry);
            if self.cached_preview_for(&entry, &variant).is_some() {
                continue;
            }

            let request = self.build_full_preview_request(
                entry.clone(),
                variant.clone(),
                PreviewPriority::Low,
                preview_work_class(&entry, &variant),
            );
            let _ = self.jobs.scheduler.submit_preview(request);
        }
    }

    pub(in crate::app) fn nearby_audio_preview_visual_overlay_requests(
        &self,
    ) -> Vec<crate::app::overlays::images::StaticImageOverlayRequest> {
        let Some(entry) = self.selected_entry() else {
            return Vec::new();
        };
        if !is_audio_entry(entry) {
            return Vec::new();
        }
        let Some(area) = self.input.frame_state.preview_media_area else {
            return Vec::new();
        };

        self.nearby_audio_candidates()
            .into_iter()
            .filter_map(|entry| {
                let variant = self.preview_request_options_for_entry(&entry);
                let cached = self.cached_preview_for(&entry, &variant)?;
                let visual = cached.preview_visual.as_ref()?;
                (cached.kind == crate::preview::PreviewKind::Audio
                    && visual.kind == crate::preview::PreviewVisualKind::Cover)
                    .then(|| {
                        self.preview_visual_overlay_request_for_visual(cached.kind, visual, area)
                    })
            })
            .collect()
    }

    pub(in crate::app) fn refreshes_image_preloads_for_nearby_audio_preview(
        &self,
        entry: &Entry,
        variant: &crate::preview::PreviewRequestOptions,
    ) -> bool {
        self.nearby_audio_candidates().into_iter().any(|candidate| {
            candidate.path == entry.path
                && candidate.size == entry.size
                && candidate.modified == entry.modified
                && variant == &self.preview_request_options_for_entry(&candidate)
        })
    }

    fn nearby_audio_candidates(&self) -> Vec<Entry> {
        AUDIO_ENTRY_PREFETCH_OFFSETS
            .into_iter()
            .filter_map(|offset| {
                let target = self.navigation.selected as isize + offset;
                if target < 0 {
                    return None;
                }
                let entry = self.navigation.entries.get(target as usize)?.clone();
                is_audio_entry(&entry).then_some(entry)
            })
            .collect()
    }
}

fn is_audio_entry(entry: &Entry) -> bool {
    file_info::inspect_path_cached(&entry.path, entry.kind, entry.size, entry.modified)
        .builtin_class
        == FileClass::Audio
}
