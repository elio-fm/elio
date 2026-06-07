mod cache;
mod format;
mod preload;
mod prepare;
mod present;
mod render;
mod state;
mod types;

use self::format::read_raster_dimensions;
use super::super::*;
use super::inline_image::TerminalWindowSize;
use super::inline_image::read_png_dimensions;
use ratatui::layout::Rect;

pub(crate) use self::prepare::prepare_static_image_asset;
pub(in crate::app) use self::types::SixelDcsKey;
pub(in crate::app) use self::types::{
    ImagePreviewState, PreparedStaticImage, PreparedStaticImageAsset, StaticImageKey,
    StaticImageOverlayMode, StaticImageOverlayPreparation, StaticImageOverlayRequest,
};

const STATIC_IMAGE_RENDER_CACHE_LIMIT: usize = 64;
const STATIC_IMAGE_INLINE_PAYLOAD_CACHE_LIMIT: usize = 16;
const SIXEL_DCS_CACHE_LIMIT: usize = 128;
const STATIC_IMAGE_PRELOAD_LIMIT: usize = 12;
const STATIC_IMAGE_PRELOAD_LIMIT_SLOW_SIXEL: usize = 2;
// Keep base64 + OSC overhead below the 1 MiB iTerm/tmux single-sequence limit.
const STATIC_IMAGE_ITERM_SOURCE_PASSTHROUGH_MAX_BYTES: u64 = 700 * 1024;
const STATIC_IMAGE_INLINE_FALLBACK_PREPARE_MAX_BYTES: u64 = 512 * 1024;
const STATIC_IMAGE_INLINE_EXTERNAL_PREPARE_MAX_BYTES: u64 = 16 * 1024 * 1024;
const STATIC_IMAGE_RENDER_CACHE_VERSION: usize = 5;
const FAST_FORCE_RENDER_FFMPEG_RASTER_ARGS: [&str; 4] =
    ["-compression_level", "1", "-sws_flags", "fast_bilinear"];
const DEFAULT_FFMPEG_RASTER_ARGS: [&str; 0] = [];

pub(in crate::app) fn static_image_detail_label(entry: &Entry) -> Option<&'static str> {
    format::static_image_detail_label(entry)
}

pub(in crate::app) fn ffmpeg_raster_render_args(
    force_render_to_cache: bool,
) -> &'static [&'static str] {
    if force_render_to_cache {
        &FAST_FORCE_RENDER_FFMPEG_RASTER_ARGS
    } else {
        &DEFAULT_FFMPEG_RASTER_ARGS
    }
}

pub(in crate::app) fn image_target_width_px(
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> u32 {
    render::image_target_width_px(area, window_size)
}

pub(in crate::app) fn image_target_height_px(
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> u32 {
    render::image_target_height_px(area, window_size)
}

impl App {
    pub(in crate::app) fn prepared_static_image_for_overlay(
        &mut self,
        request: &StaticImageOverlayRequest,
    ) -> StaticImageOverlayPreparation {
        let key = StaticImageKey::from_request(request);
        if let Some(prepared) = self.cached_prepared_static_image_for_overlay(&key, request) {
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
