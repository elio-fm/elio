use super::types::StaticImagePreloadViewport;
use super::{StaticImageKey, StaticImageOverlayRequest};
use crate::app::{App, jobs};
use std::collections::HashSet;

impl App {
    pub(in crate::app) fn refresh_static_image_preloads(&mut self) {
        let current_static = self.active_static_image_overlay_request();
        let current_preview_visual = self.active_preview_visual_overlay_request();
        let current = current_static
            .as_ref()
            .cloned()
            .or(current_preview_visual.as_ref().cloned());
        let nearby = if let Some(request) = current_static.as_ref() {
            self.nearby_static_image_overlay_requests(Some(request))
        } else if current_preview_visual.is_some() {
            let mut requests = self.nearby_comic_preview_visual_overlay_requests();
            requests.extend(self.nearby_epub_preview_visual_overlay_requests());
            requests
        } else {
            Vec::new()
        };
        let desired = current
            .iter()
            .map(StaticImageKey::from_request)
            .chain(nearby.iter().map(StaticImageKey::from_request))
            .collect::<HashSet<_>>();
        self.image_preview
            .pending_prepares
            .retain(|key| desired.contains(key));
        let current_job = current
            .as_ref()
            .map(|request| self.image_prepare_request_for_overlay(request));
        let nearby_jobs = nearby
            .iter()
            .map(|request| self.image_prepare_request_for_overlay(request))
            .collect::<Vec<_>>();
        self.scheduler
            .retain_image_prepares(current_job.as_ref(), &nearby_jobs);

        if let Some(request) = current.as_ref()
            && self.static_image_requires_prepare(request)
        {
            self.ensure_static_image_preload(request, jobs::ImageJobPriority::Current);
        }
        for request in &nearby {
            if self.static_image_requires_prepare(request) {
                self.ensure_static_image_preload(request, jobs::ImageJobPriority::Nearby);
            }
        }
    }

    pub(in crate::app) fn refresh_static_image_preloads_if_needed(&mut self) {
        let viewport = StaticImagePreloadViewport {
            selected: self.selected,
            scroll_row: self.scroll_row,
            cols: self.frame_state.metrics.cols.max(1),
            rows_visible: self.frame_state.metrics.rows_visible.max(1),
            preview_content_area: self.frame_state.preview_content_area,
            preview_media_area: self.frame_state.preview_media_area,
            protocol: self.terminal_images.protocol,
            window: self.cached_terminal_window(),
        };
        if self.image_preview.preload_viewport == Some(viewport) {
            return;
        }
        self.image_preview.preload_viewport = Some(viewport);
        self.refresh_static_image_preloads();
    }

    pub(in crate::app) fn apply_image_prepare_build(
        &mut self,
        build: jobs::ImagePrepareBuild,
    ) -> bool {
        let key = StaticImageKey::from_parts(
            build.path.clone(),
            build.size,
            build.modified,
            build.target_width_px,
            build.target_height_px,
            build.force_render_to_cache,
            build.prepare_inline_payload,
        );
        self.image_preview.pending_prepares.remove(&key);
        let is_current = self
            .active_static_image_overlay_request()
            .as_ref()
            .is_some_and(|request| StaticImageKey::from_request(request) == key)
            || self
                .active_preview_visual_overlay_request_unchecked()
                .as_ref()
                .is_some_and(|request| StaticImageKey::from_request(request) == key);
        if build.canceled {
            self.refresh_static_image_preloads();
            return is_current;
        }

        match build.result {
            Some(prepared) => {
                self.image_preview.failed_images.remove(&key);
                self.image_preview
                    .dimensions
                    .insert(key.clone(), prepared.dimensions);
                if let Some(payload) = prepared.inline_payload {
                    self.remember_static_image_inline_payload(key.clone(), payload);
                }
                if prepared.display_path != build.path {
                    self.remember_rendered_static_image(key, prepared.display_path);
                }
                self.refresh_static_image_preloads();
                is_current
            }
            None => {
                self.image_preview.failed_images.insert(key);
                if is_current {
                    self.refresh_preview();
                    true
                } else {
                    false
                }
            }
        }
    }

    fn nearby_static_image_overlay_requests(
        &self,
        current: Option<&StaticImageOverlayRequest>,
    ) -> Vec<StaticImageOverlayRequest> {
        let current_path = current.as_ref().map(|request| &request.path);
        let mut requests = self
            .visible_entry_indices()
            .into_iter()
            .filter(|&index| index != self.selected)
            .filter_map(|index| {
                self.entries
                    .get(index)
                    .and_then(|entry| self.static_image_overlay_request_for_entry(entry))
                    .map(|request| (index.abs_diff(self.selected), request))
            })
            .filter(|(_, request)| current_path != Some(&request.path))
            .collect::<Vec<_>>();
        requests.sort_by_key(|(distance, _)| *distance);
        requests
            .into_iter()
            .map(|(_, request)| request)
            .take(super::STATIC_IMAGE_PRELOAD_LIMIT)
            .collect()
    }

    fn ensure_static_image_preload(
        &mut self,
        request: &StaticImageOverlayRequest,
        priority: jobs::ImageJobPriority,
    ) {
        let key = StaticImageKey::from_request(request);
        if self.image_preview.failed_images.contains(&key)
            || self.image_preview.pending_prepares.contains(&key)
            || self
                .cached_prepared_static_image_for_overlay(&key, request)
                .is_some()
        {
            return;
        }

        let job = self.image_prepare_request_for_overlay(request);
        let submit = match priority {
            jobs::ImageJobPriority::Current => self.scheduler.submit_image_prepare(job),
            jobs::ImageJobPriority::Nearby => self.scheduler.submit_nearby_image_prepare(job),
        };
        if submit {
            self.image_preview.pending_prepares.insert(key);
        }
    }

    fn image_prepare_request_for_overlay(
        &mut self,
        request: &StaticImageOverlayRequest,
    ) -> jobs::ImagePrepareRequest {
        jobs::ImagePrepareRequest {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            target_width_px: request.target_width_px,
            target_height_px: request.target_height_px,
            ffmpeg_available: self.ffmpeg_available(),
            resvg_available: self.resvg_available(),
            magick_available: self.magick_available(),
            force_render_to_cache: request.force_render_to_cache,
            prepare_inline_payload: request.prepare_inline_payload,
        }
    }
}
