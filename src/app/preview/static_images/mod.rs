mod preload;
mod present;
mod state;

use super::super::*;
use crate::{
    preview::images::read_raster_dimensions, terminal_runtime::terminal_images::read_png_dimensions,
};

pub(crate) use crate::preview::images::{
    SixelDcsKey, StaticImageKey, image_target_height_px, image_target_width_px,
    static_image_detail_label,
};
pub(in crate::app::preview::static_images) use crate::preview::{
    DisplayedStaticImagePreview, StaticImagePreloadViewport,
};
pub(crate) use crate::preview::{
    PreparedStaticImage, StaticImageOverlayMode, StaticImageOverlayPreparation,
    StaticImageOverlayRequest,
};

const STATIC_IMAGE_PRELOAD_LIMIT: usize = 12;
const STATIC_IMAGE_PRELOAD_LIMIT_SLOW_SIXEL: usize = 2;

impl App {
    pub(crate) fn prepared_static_image_for_overlay(
        &mut self,
        request: &StaticImageOverlayRequest,
    ) -> StaticImageOverlayPreparation {
        let key = StaticImageKey::from_request(request);
        let protocol = self.preview.terminal_images.protocol;
        if let Some(prepared) = self
            .preview
            .image
            .cached_prepared_image(&key, request, protocol)
        {
            return StaticImageOverlayPreparation::Ready(prepared);
        }
        if let Some(prepared) = self.direct_static_image_for_overlay(request) {
            return StaticImageOverlayPreparation::Ready(prepared);
        }
        if self.preview.image.pending_prepares.contains(&key) {
            return StaticImageOverlayPreparation::Pending;
        }
        if self.preview.image.failed_images.contains(&key) {
            StaticImageOverlayPreparation::Failed
        } else {
            // No prepare job is running and the key has not failed.  This can happen when a job
            // was cancelled by a stale refresh_static_image_preloads() call without a replacement
            // being queued (e.g. because preview_state.content had no preview_visual at the time).
            // Re-submit via a fresh preload cycle so the overlay can be presented next cycle.
            self.refresh_static_image_preloads();
            StaticImageOverlayPreparation::Pending
        }
    }

    fn direct_static_image_for_overlay(
        &mut self,
        request: &StaticImageOverlayRequest,
    ) -> Option<PreparedStaticImage> {
        if !self.static_image_can_display_directly_now(request) {
            return None;
        }

        let key = StaticImageKey::from_request(request);
        self.preview.image.failed_images.remove(&key);
        let dimensions = self
            .preview
            .image
            .dimensions
            .get(&key)
            .copied()
            .or_else(|| read_png_dimensions(&request.path))
            .or_else(|| read_raster_dimensions(&request.path))?;
        self.preview.image.dimensions.insert(key, dimensions);

        Some(PreparedStaticImage {
            display_path: request.path.clone(),
            dimensions,
            inline_payload: None,
        })
    }
}

#[cfg(test)]
mod tests;
