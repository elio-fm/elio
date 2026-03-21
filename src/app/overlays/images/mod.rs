mod state;
mod types;

use super::super::*;
use super::inline_image::{
    ImageProtocol, OverlayPresentState, RenderedImageDimensions, TerminalWindowSize,
    fit_image_area, place_terminal_image, preview_log, read_png_dimensions,
};
use anyhow::Result;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, imageops::FilterType};
use quick_xml::{Reader, events::Event};
use ratatui::layout::Rect;
use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    env, fs,
    fs::File,
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::SystemTime,
};

use self::types::{DisplayedStaticImagePreview, StaticImagePreloadViewport};
pub(in crate::app) use self::types::{
    ImagePreviewState, PreparedStaticImage, PreparedStaticImageAsset, StaticImageKey,
    StaticImageOverlayMode, StaticImageOverlayPreparation, StaticImageOverlayRequest,
};

const STATIC_IMAGE_RENDER_CACHE_LIMIT: usize = 24;
const STATIC_IMAGE_INLINE_PAYLOAD_CACHE_LIMIT: usize = 10;
const STATIC_IMAGE_PRELOAD_LIMIT: usize = 6;
const STATIC_IMAGE_INLINE_FALLBACK_PREPARE_MAX_BYTES: u64 = 512 * 1024;
const STATIC_IMAGE_INLINE_EXTERNAL_PREPARE_MAX_BYTES: u64 = 16 * 1024 * 1024;
const STATIC_IMAGE_RENDER_CACHE_VERSION: usize = 3;
const FAST_FORCE_RENDER_FFMPEG_RASTER_ARGS: [&str; 4] =
    ["-compression_level", "1", "-sws_flags", "fast_bilinear"];
const DEFAULT_FFMPEG_RASTER_ARGS: [&str; 0] = [];

impl App {
    pub(in crate::app) fn present_static_image_overlay(
        &mut self,
        protocol: ImageProtocol,
        excluded: &[Rect],
        out: &mut Vec<u8>,
    ) -> Result<OverlayPresentState> {
        let Some(request) = self.active_static_image_overlay_request() else {
            preview_log("present_static_image_overlay: no request");
            return Ok(OverlayPresentState::NotRequested);
        };
        preview_log(format_args!(
            "present_static_image_overlay: path={:?} area={:?}",
            request.path, request.area
        ));
        if !self.image_selection_activation_ready() {
            preview_log("present_static_image_overlay: activation not ready → Waiting");
            return Ok(OverlayPresentState::Waiting);
        }

        let prepared = match self.prepared_static_image_for_overlay(&request) {
            StaticImageOverlayPreparation::Ready(prepared) => prepared,
            StaticImageOverlayPreparation::Pending => {
                preview_log("present_static_image_overlay: preparation Pending → Waiting");
                return Ok(OverlayPresentState::Waiting);
            }
            StaticImageOverlayPreparation::Failed => {
                preview_log("present_static_image_overlay: preparation Failed");
                self.mark_static_image_failed(&request);
                self.refresh_preview();
                return Ok(OverlayPresentState::NotRequested);
            }
        };
        let Some(window_size) = self.cached_terminal_window() else {
            preview_log("present_static_image_overlay: no cached window size → failed");
            self.mark_static_image_failed(&request);
            self.refresh_preview();
            return Ok(OverlayPresentState::NotRequested);
        };
        let placement = self.static_image_display_area(&request, prepared.dimensions, window_size);
        preview_log(format_args!(
            "present_static_image_overlay: dims={}x{} placement={:?}",
            prepared.dimensions.width_px, prepared.dimensions.height_px, placement
        ));
        let displayed = DisplayedStaticImagePreview::from_request(
            &request,
            placement,
            self.static_image_clear_area(&request),
        );
        let image_changed = self.image_preview.displayed.as_ref() != Some(&displayed);
        let excluded_changed = excluded != self.image_preview.displayed_excluded.as_slice();
        if !image_changed && !excluded_changed {
            preview_log("present_static_image_overlay: already displayed → Displayed");
            return Ok(OverlayPresentState::Displayed);
        }
        // Only clear the old image when the image itself changed, not when only the
        // excluded rects changed, to avoid a visible flash as the image disappears
        // and then immediately reappears.
        if image_changed {
            out.extend(self.clear_preview_overlay()?);
        }
        match place_terminal_image(
            protocol,
            &prepared.display_path,
            placement,
            excluded,
            prepared.inline_payload.as_deref(),
        ) {
            Ok(bytes) => {
                preview_log(format_args!(
                    "present_static_image_overlay: placed {} bytes via {protocol:?}",
                    bytes.len()
                ));
                out.extend(bytes);
            }
            Err(e) => {
                preview_log(format_args!(
                    "present_static_image_overlay: place_terminal_image error: {e}"
                ));
                self.mark_static_image_failed(&request);
                self.refresh_preview();
                return Ok(OverlayPresentState::NotRequested);
            }
        }

        self.image_preview.displayed = Some(displayed);
        self.image_preview.displayed_excluded = excluded.to_vec();
        Ok(OverlayPresentState::Displayed)
    }

    pub(in crate::app) fn present_preview_visual_overlay(
        &mut self,
        protocol: ImageProtocol,
        excluded: &[Rect],
        out: &mut Vec<u8>,
    ) -> Result<OverlayPresentState> {
        let Some(request) = self.active_preview_visual_overlay_request() else {
            preview_log("present_preview_visual_overlay: no request");
            return Ok(OverlayPresentState::NotRequested);
        };
        preview_log(format_args!(
            "present_preview_visual_overlay: path={:?} area={:?}",
            request.path, request.area
        ));
        if !self.image_selection_activation_ready() {
            preview_log("present_preview_visual_overlay: activation not ready → Waiting");
            return Ok(OverlayPresentState::Waiting);
        }

        let prepared = match self.prepared_static_image_for_overlay(&request) {
            StaticImageOverlayPreparation::Ready(prepared) => prepared,
            StaticImageOverlayPreparation::Pending => {
                preview_log("present_preview_visual_overlay: preparation Pending → Waiting");
                return Ok(OverlayPresentState::Waiting);
            }
            StaticImageOverlayPreparation::Failed => {
                preview_log("present_preview_visual_overlay: preparation Failed");
                self.mark_static_image_failed(&request);
                return Ok(OverlayPresentState::NotRequested);
            }
        };
        let Some(window_size) = self.cached_terminal_window() else {
            preview_log("present_preview_visual_overlay: no cached window size → failed");
            self.mark_static_image_failed(&request);
            return Ok(OverlayPresentState::NotRequested);
        };
        let placement = self.static_image_display_area(&request, prepared.dimensions, window_size);
        preview_log(format_args!(
            "present_preview_visual_overlay: dims={}x{} placement={:?}",
            prepared.dimensions.width_px, prepared.dimensions.height_px, placement
        ));
        let displayed = DisplayedStaticImagePreview::from_request(
            &request,
            placement,
            self.static_image_clear_area(&request),
        );
        let image_changed = self.image_preview.displayed.as_ref() != Some(&displayed);
        let excluded_changed = excluded != self.image_preview.displayed_excluded.as_slice();
        if !image_changed && !excluded_changed {
            preview_log("present_preview_visual_overlay: already displayed → Displayed");
            return Ok(OverlayPresentState::Displayed);
        }
        if image_changed {
            out.extend(self.clear_preview_overlay()?);
        }
        match place_terminal_image(
            protocol,
            &prepared.display_path,
            placement,
            excluded,
            prepared.inline_payload.as_deref(),
        ) {
            Ok(bytes) => {
                preview_log(format_args!(
                    "present_preview_visual_overlay: placed {} bytes via {protocol:?}",
                    bytes.len()
                ));
                out.extend(bytes);
            }
            Err(e) => {
                preview_log(format_args!(
                    "present_preview_visual_overlay: place_terminal_image error: {e}"
                ));
                self.mark_static_image_failed(&request);
                return Ok(OverlayPresentState::NotRequested);
            }
        }

        self.image_preview.displayed = Some(displayed);
        self.image_preview.displayed_excluded = excluded.to_vec();
        Ok(OverlayPresentState::Displayed)
    }

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
        if self.image_preview.pending_prepares.contains(&key) {
            return StaticImageOverlayPreparation::Pending;
        }
        if self.image_preview.failed_images.contains(&key) {
            StaticImageOverlayPreparation::Failed
        } else {
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
        self.image_preview.failed_images.remove(&key);
        let dimensions = self
            .image_preview
            .dimensions
            .get(&key)
            .copied()
            .or_else(|| read_png_dimensions(&request.path))
            .or_else(|| read_raster_dimensions(&request.path))?;
        self.image_preview.dimensions.insert(key, dimensions);

        Some(PreparedStaticImage {
            display_path: request.path.clone(),
            dimensions,
            inline_payload: None,
        })
    }

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

    fn cached_static_image_display_path(&mut self, key: &StaticImageKey) -> Option<PathBuf> {
        if let Some(path) = self.image_preview.rendered_images.get(key)
            && path.exists()
        {
            return Some(path.clone());
        }

        self.image_preview.rendered_images.remove(key);
        self.image_preview
            .render_order
            .retain(|queued| queued != key);
        None
    }

    fn cached_static_image_inline_payload(&self, key: &StaticImageKey) -> Option<Arc<str>> {
        self.image_preview.inline_payloads.get(key).cloned()
    }

    fn remember_static_image_inline_payload(&mut self, key: StaticImageKey, payload: Arc<str>) {
        self.image_preview
            .inline_payloads
            .insert(key.clone(), payload);
        self.image_preview
            .payload_order
            .retain(|queued| queued != &key);
        self.image_preview.payload_order.push_back(key);
        while self.image_preview.payload_order.len() > STATIC_IMAGE_INLINE_PAYLOAD_CACHE_LIMIT {
            if let Some(stale_key) = self.image_preview.payload_order.pop_front() {
                self.image_preview.inline_payloads.remove(&stale_key);
            }
        }
    }

    fn cached_prepared_static_image_for_overlay(
        &mut self,
        key: &StaticImageKey,
        request: &StaticImageOverlayRequest,
    ) -> Option<PreparedStaticImage> {
        let dimensions = self.image_preview.dimensions.get(key).copied()?;
        let inline_payload = if request.prepare_inline_payload {
            Some(self.cached_static_image_inline_payload(key)?)
        } else {
            None
        };
        if self.static_image_can_use_source_path(request) {
            return Some(PreparedStaticImage {
                display_path: request.path.clone(),
                dimensions,
                inline_payload,
            });
        }

        let display_path = self.cached_static_image_display_path(key)?;
        Some(PreparedStaticImage {
            display_path,
            dimensions,
            inline_payload,
        })
    }

    fn remember_rendered_static_image(&mut self, key: StaticImageKey, path: PathBuf) {
        self.image_preview.rendered_images.insert(key.clone(), path);
        self.image_preview
            .render_order
            .retain(|queued| queued != &key);
        self.image_preview.render_order.push_back(key);
        while self.image_preview.render_order.len() > STATIC_IMAGE_RENDER_CACHE_LIMIT {
            if let Some(stale_key) = self.image_preview.render_order.pop_front()
                && let Some(stale_path) = self.image_preview.rendered_images.remove(&stale_key)
            {
                let _ = fs::remove_file(stale_path);
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
            .take(STATIC_IMAGE_PRELOAD_LIMIT)
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

    fn active_static_image_display_target(&self) -> Option<DisplayedStaticImagePreview> {
        let request = self
            .active_static_image_overlay_request()
            .or_else(|| self.active_preview_visual_overlay_request_unchecked())?;
        let window_size = self.cached_terminal_window()?;
        let image_dimensions = self
            .image_preview
            .dimensions
            .get(&StaticImageKey::from_request(&request))
            .copied()?;
        Some(DisplayedStaticImagePreview::from_request(
            &request,
            self.static_image_display_area(&request, image_dimensions, window_size),
            self.static_image_clear_area(&request),
        ))
    }

    fn static_image_clear_area(&self, request: &StaticImageOverlayRequest) -> Rect {
        if self.terminal_images.protocol == ImageProtocol::ItermInline {
            if request.mode == StaticImageOverlayMode::Inline {
                return self
                    .frame_state
                    .preview_body_area
                    .or_else(|| self.preview_body_area())
                    .unwrap_or(request.area);
            }
            return self
                .frame_state
                .preview_content_area
                .unwrap_or(request.area);
        }
        request.area
    }

    fn preview_body_area(&self) -> Option<Rect> {
        match (
            self.frame_state.preview_media_area,
            self.frame_state.preview_content_area,
        ) {
            (Some(media), Some(content)) => Some(union_rect(media, content)),
            (Some(media), None) => Some(media),
            (None, Some(content)) => Some(content),
            (None, None) => None,
        }
    }

    fn static_image_display_area(
        &self,
        request: &StaticImageOverlayRequest,
        dimensions: RenderedImageDimensions,
        window_size: TerminalWindowSize,
    ) -> Rect {
        if self.terminal_images.protocol == ImageProtocol::KittyGraphics {
            request.area
        } else {
            fit_image_area(
                request.area,
                window_size,
                dimensions.width_px as f32 / dimensions.height_px as f32,
            )
        }
    }
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let left = a.x.min(b.x);
    let top = a.y.min(b.y);
    let right = a.x.saturating_add(a.width).max(b.x.saturating_add(b.width));
    let bottom =
        a.y.saturating_add(a.height)
            .max(b.y.saturating_add(b.height));
    Rect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StaticImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
    Svg,
}

impl StaticImageFormat {
    fn detail_label(self) -> &'static str {
        match self {
            Self::Png => "PNG image",
            Self::Jpeg => "JPEG image",
            Self::Gif => "GIF image",
            Self::Webp => "WebP image",
            Self::Svg => "SVG image",
        }
    }

    fn from_label(label: &'static str) -> Option<Self> {
        match label {
            "PNG image" => Some(Self::Png),
            "JPEG image" => Some(Self::Jpeg),
            "GIF image" => Some(Self::Gif),
            "WebP image" => Some(Self::Webp),
            "SVG image" => Some(Self::Svg),
            _ => None,
        }
    }
}

pub(in crate::app) fn static_image_detail_label(entry: &Entry) -> Option<&'static str> {
    static_image_format_for_entry(entry).map(StaticImageFormat::detail_label)
}

pub(crate) fn prepare_static_image_asset<F>(
    request: &jobs::ImagePrepareRequest,
    canceled: F,
) -> Option<PreparedStaticImageAsset>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }
    let format = static_image_format_for_prepare_request(request)?;
    let source_dimensions = if format == StaticImageFormat::Svg {
        read_svg_dimensions(&request.path)?
    } else {
        read_raster_dimensions(&request.path)?
    };
    if canceled() {
        return None;
    }
    let target_width_px = request.target_width_px.max(1);
    let target_height_px = request.target_height_px.max(1);
    let key = StaticImageKey::from_parts(
        request.path.clone(),
        request.size,
        request.modified,
        target_width_px,
        target_height_px,
        request.force_render_to_cache,
        request.prepare_inline_payload,
    );
    let inline_payload = |path: &Path| -> Option<Option<Arc<str>>> {
        if !request.prepare_inline_payload {
            return Some(None);
        }
        Some(Some(super::inline_image::encode_iterm_inline_payload(
            path,
        )?))
    };

    if static_image_supports_iterm_source_passthrough_for_prepare(request, format) {
        return Some(PreparedStaticImageAsset {
            dimensions: source_dimensions,
            display_path: request.path.clone(),
            inline_payload: inline_payload(&request.path)?,
        });
    }

    if format == StaticImageFormat::Svg {
        let cache_path = static_image_render_cache_path(&key)?;
        if cache_path.exists() {
            let payload = inline_payload(&cache_path)?;
            return Some(PreparedStaticImageAsset {
                dimensions: prepared_display_dimensions(&cache_path, source_dimensions),
                display_path: cache_path,
                inline_payload: payload,
            });
        }
        let temp_path = static_image_render_temp_path(&cache_path)?;
        let rendered = (request.resvg_available
            && render_svg_to_png_with_resvg(
                &request.path,
                &temp_path,
                source_dimensions,
                target_width_px,
                target_height_px,
                &canceled,
            ))
            || (request.magick_available
                && render_svg_to_png_with_magick(
                    &request.path,
                    &temp_path,
                    target_width_px,
                    target_height_px,
                    &canceled,
                ));
        if rendered {
            finalize_static_image_render(&temp_path, &cache_path)?;
            let payload = inline_payload(&cache_path)?;
            return Some(PreparedStaticImageAsset {
                dimensions: prepared_display_dimensions(&cache_path, source_dimensions),
                display_path: cache_path,
                inline_payload: payload,
            });
        }
        let _ = fs::remove_file(temp_path);
        return None;
    }

    let cache_path = static_image_render_cache_path(&key)?;
    if cache_path.exists() {
        let payload = inline_payload(&cache_path)?;
        return Some(PreparedStaticImageAsset {
            dimensions: prepared_display_dimensions(&cache_path, source_dimensions),
            display_path: cache_path,
            inline_payload: payload,
        });
    }
    if canceled() {
        return None;
    }
    let temp_path = static_image_render_temp_path(&cache_path)?;

    if request.ffmpeg_available
        && should_render_raster_with_ffmpeg(format)
        && render_raster_to_png_with_ffmpeg(
            &request.path,
            &temp_path,
            target_width_px,
            target_height_px,
            request.force_render_to_cache,
            &canceled,
        )
    {
        finalize_static_image_render(&temp_path, &cache_path)?;
        let payload = inline_payload(&cache_path)?;
        return Some(PreparedStaticImageAsset {
            dimensions: prepared_display_dimensions(&cache_path, source_dimensions),
            display_path: cache_path,
            inline_payload: payload,
        });
    }

    let image = ImageReader::open(&request.path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    if canceled() {
        return None;
    }
    let image = apply_raster_orientation(image, read_exif_orientation(&request.path).unwrap_or(1));
    if canceled() {
        return None;
    }
    let image = shrink_image_to_fit(image, target_width_px, target_height_px);
    if canceled() {
        return None;
    }
    image.save_with_format(&temp_path, ImageFormat::Png).ok()?;
    finalize_static_image_render(&temp_path, &cache_path)?;
    let payload = inline_payload(&cache_path)?;

    Some(PreparedStaticImageAsset {
        dimensions: prepared_display_dimensions(&cache_path, source_dimensions),
        display_path: cache_path,
        inline_payload: payload,
    })
}

fn prepared_display_dimensions(
    display_path: &Path,
    fallback: RenderedImageDimensions,
) -> RenderedImageDimensions {
    read_png_dimensions(display_path)
        .or_else(|| read_raster_dimensions(display_path))
        .unwrap_or(fallback)
}

fn static_image_render_cache_path(key: &StaticImageKey) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    STATIC_IMAGE_RENDER_CACHE_VERSION.hash(&mut hasher);
    key.path.hash(&mut hasher);
    key.size.hash(&mut hasher);
    key.modified.hash(&mut hasher);
    key.target_width_px.hash(&mut hasher);
    key.target_height_px.hash(&mut hasher);
    key.force_render_to_cache.hash(&mut hasher);
    let cache_dir = env::temp_dir().join(format!(
        "elio-image-preview-v{STATIC_IMAGE_RENDER_CACHE_VERSION}"
    ));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!("image-{:016x}.png", hasher.finish())))
}

fn static_image_render_temp_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let stem = path.file_stem()?.to_string_lossy();
    let extension = path.extension().and_then(|extension| extension.to_str());
    let file_name = match extension {
        Some(extension) if !extension.is_empty() => {
            format!(".{stem}.tmp-{}-{unique}.{extension}", std::process::id())
        }
        _ => format!(".{stem}.tmp-{}-{unique}", std::process::id()),
    };
    Some(parent.join(file_name))
}

fn finalize_static_image_render(temp_path: &Path, cache_path: &Path) -> Option<()> {
    match fs::rename(temp_path, cache_path) {
        Ok(()) => Some(()),
        Err(_) if cache_path.exists() => {
            let _ = fs::remove_file(temp_path);
            Some(())
        }
        Err(_) => {
            let _ = fs::remove_file(temp_path);
            None
        }
    }
}

fn static_image_can_prepare_inline(
    size: u64,
    format: StaticImageFormat,
    ffmpeg_available: bool,
) -> bool {
    match format {
        StaticImageFormat::Png => true,
        StaticImageFormat::Jpeg | StaticImageFormat::Gif | StaticImageFormat::Webp => {
            if ffmpeg_available {
                size <= STATIC_IMAGE_INLINE_EXTERNAL_PREPARE_MAX_BYTES
            } else {
                size <= STATIC_IMAGE_INLINE_FALLBACK_PREPARE_MAX_BYTES
            }
        }
        StaticImageFormat::Svg => false,
    }
}

fn static_image_supports_iterm_source_passthrough(request: &StaticImageOverlayRequest) -> bool {
    static_image_format_for_overlay_request(request)
        .is_some_and(|format| static_image_supports_iterm_source_format(&request.path, format))
        && !request.force_render_to_cache
}

fn static_image_supports_iterm_source_passthrough_for_prepare(
    request: &jobs::ImagePrepareRequest,
    format: StaticImageFormat,
) -> bool {
    request.prepare_inline_payload
        && !request.force_render_to_cache
        && static_image_supports_iterm_source_format(&request.path, format)
}

fn static_image_supports_iterm_source_format(path: &Path, format: StaticImageFormat) -> bool {
    match format {
        StaticImageFormat::Png => true,
        StaticImageFormat::Jpeg => read_exif_orientation(path).unwrap_or(1) == 1,
        StaticImageFormat::Gif | StaticImageFormat::Webp | StaticImageFormat::Svg => false,
    }
}

fn should_render_raster_with_ffmpeg(format: StaticImageFormat) -> bool {
    matches!(
        format,
        StaticImageFormat::Jpeg | StaticImageFormat::Gif | StaticImageFormat::Webp
    )
}

fn static_image_format_for_entry(entry: &Entry) -> Option<StaticImageFormat> {
    crate::file_info::inspect_path_cached(&entry.path, entry.kind, entry.size, entry.modified)
        .specific_type_label
        .and_then(StaticImageFormat::from_label)
}

fn static_image_format_for_overlay_request(
    request: &StaticImageOverlayRequest,
) -> Option<StaticImageFormat> {
    crate::file_info::inspect_path_cached(
        &request.path,
        EntryKind::File,
        request.size,
        request.modified,
    )
    .specific_type_label
    .and_then(StaticImageFormat::from_label)
}

fn static_image_format_for_prepare_request(
    request: &jobs::ImagePrepareRequest,
) -> Option<StaticImageFormat> {
    crate::file_info::inspect_path_cached(
        &request.path,
        EntryKind::File,
        request.size,
        request.modified,
    )
    .specific_type_label
    .and_then(StaticImageFormat::from_label)
}

fn static_image_format_for_path(path: &Path) -> Option<StaticImageFormat> {
    crate::file_info::inspect_path(path, EntryKind::File)
        .specific_type_label
        .and_then(StaticImageFormat::from_label)
}

fn render_svg_to_png_with_resvg(
    input_path: &Path,
    output_path: &Path,
    source_dimensions: RenderedImageDimensions,
    target_width_px: u32,
    target_height_px: u32,
    canceled: &impl Fn() -> bool,
) -> bool {
    if let Some(parent) = output_path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }

    let (width_arg, height_arg) =
        fit_svg_render_dimensions(source_dimensions, target_width_px, target_height_px);
    let mut command = Command::new("resvg");
    if let Some(width_px) = width_arg {
        command.arg("--width").arg(width_px.to_string());
    }
    if let Some(height_px) = height_arg {
        command.arg("--height").arg(height_px.to_string());
    }
    command
        .arg(input_path)
        .arg(output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    run_cancelable_command(&mut command, canceled)
        .is_some_and(|status| status.success() && output_path.exists())
}

fn render_svg_to_png_with_magick(
    input_path: &Path,
    output_path: &Path,
    target_width_px: u32,
    target_height_px: u32,
    canceled: &impl Fn() -> bool,
) -> bool {
    if let Some(parent) = output_path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }

    run_cancelable_command(
        Command::new("magick")
            .arg(input_path)
            .arg("-resize")
            .arg(format!(
                "{}x{}>",
                target_width_px.max(1),
                target_height_px.max(1)
            ))
            .arg(output_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
        canceled,
    )
    .is_some_and(|status| status.success() && output_path.exists())
}

fn fit_svg_render_dimensions(
    source_dimensions: RenderedImageDimensions,
    target_width_px: u32,
    target_height_px: u32,
) -> (Option<u32>, Option<u32>) {
    let source_width = source_dimensions.width_px.max(1) as f32;
    let source_height = source_dimensions.height_px.max(1) as f32;
    let scale = (target_width_px.max(1) as f32 / source_width)
        .min(target_height_px.max(1) as f32 / source_height)
        .min(1.0);
    if scale >= 1.0 {
        return (None, None);
    }

    let fitted_width = (source_width * scale).round().max(1.0) as u32;
    let fitted_height = (source_height * scale).round().max(1.0) as u32;
    let width_ratio = target_width_px.max(1) as f32 / source_width;
    let height_ratio = target_height_px.max(1) as f32 / source_height;
    if width_ratio <= height_ratio {
        (Some(fitted_width), None)
    } else {
        (None, Some(fitted_height))
    }
}

fn render_raster_to_png_with_ffmpeg(
    input_path: &Path,
    output_path: &Path,
    target_width_px: u32,
    target_height_px: u32,
    force_render_to_cache: bool,
    canceled: &impl Fn() -> bool,
) -> bool {
    if let Some(parent) = output_path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }

    let mut command = Command::new("ffmpeg");
    command
        .arg("-v")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!(
            "scale=w={}:h={}:force_original_aspect_ratio=decrease",
            target_width_px.max(1),
            target_height_px.max(1)
        ));
    command.args(ffmpeg_raster_render_args(force_render_to_cache));
    command
        .arg(output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    run_cancelable_command(&mut command, canceled)
        .is_some_and(|status| status.success() && output_path.exists())
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

fn read_raster_dimensions(path: &Path) -> Option<RenderedImageDimensions> {
    let (mut width_px, mut height_px) = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    if exif_orientation_swaps_dimensions(read_exif_orientation(path).unwrap_or(1)) {
        std::mem::swap(&mut width_px, &mut height_px);
    }
    (width_px > 0 && height_px > 0).then_some(RenderedImageDimensions {
        width_px,
        height_px,
    })
}

fn read_exif_orientation(path: &Path) -> Option<u16> {
    let mut file = File::open(path).ok()?;
    let mut soi = [0_u8; 2];
    file.read_exact(&mut soi).ok()?;
    if soi != [0xff, 0xd8] {
        return None;
    }

    loop {
        let mut prefix = [0_u8; 1];
        file.read_exact(&mut prefix).ok()?;
        while prefix[0] != 0xff {
            file.read_exact(&mut prefix).ok()?;
        }

        let mut marker = [0_u8; 1];
        file.read_exact(&mut marker).ok()?;
        while marker[0] == 0xff {
            file.read_exact(&mut marker).ok()?;
        }

        match marker[0] {
            0xd8 | 0x01 => continue,
            0xd9 | 0xda => return None,
            _ => {
                let mut length = [0_u8; 2];
                file.read_exact(&mut length).ok()?;
                let payload_len = usize::from(u16::from_be_bytes(length)).checked_sub(2)?;
                let mut payload = vec![0_u8; payload_len];
                file.read_exact(&mut payload).ok()?;
                if marker[0] == 0xe1 && payload.starts_with(b"Exif\0\0") {
                    return parse_exif_orientation(&payload[6..]);
                }
            }
        }
    }
}

fn parse_exif_orientation(tiff: &[u8]) -> Option<u16> {
    if tiff.len() < 8 {
        return None;
    }
    let little_endian = match &tiff[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let read_u16 = |offset: usize| -> Option<u16> {
        let bytes: [u8; 2] = tiff.get(offset..offset + 2)?.try_into().ok()?;
        Some(if little_endian {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    };
    let read_u32 = |offset: usize| -> Option<u32> {
        let bytes: [u8; 4] = tiff.get(offset..offset + 4)?.try_into().ok()?;
        Some(if little_endian {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    };

    if read_u16(2)? != 42 {
        return None;
    }
    let ifd_offset = read_u32(4)? as usize;
    let entry_count = usize::from(read_u16(ifd_offset)?);
    let mut entry_offset = ifd_offset + 2;
    for _ in 0..entry_count {
        let tag = read_u16(entry_offset)?;
        let field_type = read_u16(entry_offset + 2)?;
        let count = read_u32(entry_offset + 4)?;
        if tag == 0x0112 && field_type == 3 && count >= 1 {
            return read_u16(entry_offset + 8);
        }
        entry_offset += 12;
    }
    None
}

fn exif_orientation_swaps_dimensions(orientation: u16) -> bool {
    matches!(orientation, 5..=8)
}

fn apply_raster_orientation(image: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate90().flipv(),
        8 => image.rotate270(),
        _ => image,
    }
}

fn shrink_image_to_fit(
    image: DynamicImage,
    target_width_px: u32,
    target_height_px: u32,
) -> DynamicImage {
    let (width_px, height_px) = image.dimensions();
    if width_px <= target_width_px.max(1) && height_px <= target_height_px.max(1) {
        image
    } else {
        image.resize(
            target_width_px.max(1),
            target_height_px.max(1),
            FilterType::Triangle,
        )
    }
}

pub(in crate::app) fn image_target_width_px(
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> u32 {
    let (cell_width_px, _) = image_cell_pixels(window_size);
    (f32::from(area.width.max(1)) * cell_width_px)
        .round()
        .max(1.0) as u32
}

pub(in crate::app) fn image_target_height_px(
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> u32 {
    let (_, cell_height_px) = image_cell_pixels(window_size);
    (f32::from(area.height.max(1)) * cell_height_px)
        .round()
        .max(1.0) as u32
}

fn image_cell_pixels(window_size: Option<TerminalWindowSize>) -> (f32, f32) {
    match window_size {
        Some(window_size) => (
            window_size.pixels_width as f32 / f32::from(window_size.cells_width.max(1)),
            window_size.pixels_height as f32 / f32::from(window_size.cells_height.max(1)),
        ),
        None => (8.0, 16.0),
    }
}

fn run_cancelable_command<F>(
    command: &mut Command,
    canceled: &F,
) -> Option<std::process::ExitStatus>
where
    F: Fn() -> bool,
{
    let mut child = command.spawn().ok()?;
    loop {
        if canceled() {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        if let Some(status) = child.try_wait().ok()? {
            return Some(status);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn read_svg_dimensions(path: &std::path::Path) -> Option<RenderedImageDimensions> {
    let bytes = fs::read(path).ok()?;
    let mut reader = Reader::from_reader(bytes.as_slice());
    reader.config_mut().trim_text(true);

    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == b"svg" => {
                let mut width = None;
                let mut height = None;
                let mut view_box = None;
                for attribute in tag.attributes().flatten() {
                    let key = attribute.key.as_ref();
                    let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
                    match key {
                        b"width" => width = parse_svg_length_px(&value),
                        b"height" => height = parse_svg_length_px(&value),
                        b"viewBox" => view_box = parse_svg_view_box(&value),
                        _ => {}
                    }
                }

                return match (width, height, view_box) {
                    (Some(width_px), Some(height_px), _) if width_px > 0 && height_px > 0 => {
                        Some(RenderedImageDimensions {
                            width_px,
                            height_px,
                        })
                    }
                    (_, _, Some((width_px, height_px))) if width_px > 0.0 && height_px > 0.0 => {
                        Some(RenderedImageDimensions {
                            width_px: width_px.round() as u32,
                            height_px: height_px.round() as u32,
                        })
                    }
                    _ => None,
                };
            }
            Event::Eof => return None,
            _ => {}
        }
        buffer.clear();
    }
}

fn parse_svg_length_px(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    let numeric = trimmed
        .strip_suffix("px")
        .unwrap_or(trimmed)
        .trim()
        .parse::<f32>()
        .ok()?;
    (numeric > 0.0).then_some(numeric.round() as u32)
}

fn parse_svg_view_box(value: &str) -> Option<(f32, f32)> {
    let mut parts = value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty());
    let _min_x = parts.next()?.parse::<f32>().ok()?;
    let _min_y = parts.next()?.parse::<f32>().ok()?;
    let width = parts.next()?.parse::<f32>().ok()?;
    let height = parts.next()?.parse::<f32>().ok()?;
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::overlays::inline_image::{
        OverlayPresentState, TerminalWindowSize, build_kitty_clear_sequence,
    };
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::time::{Duration, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-static-image-{label}-{unique}"))
    }

    fn configure_terminal_image_support(app: &mut App) {
        let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
        app.terminal_images.protocol = ImageProtocol::KittyGraphics;
        app.terminal_images.window = Some(TerminalWindowSize {
            cells_width,
            cells_height,
            pixels_width: 1920,
            pixels_height: 1080,
        });
    }

    fn write_test_raster_image(path: &Path, format: ImageFormat, width_px: u32, height_px: u32) {
        let mut image = RgbaImage::new(width_px, height_px);
        for pixel in image.pixels_mut() {
            *pixel = Rgba([32, 128, 224, 255]);
        }

        DynamicImage::ImageRgba8(image)
            .save_with_format(path, format)
            .expect("failed to write raster test image");
    }

    fn set_single_test_entry(app: &mut App, path: &Path) {
        let metadata = fs::metadata(path).expect("file metadata should exist");
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("file name should be valid utf-8");
        app.entries = vec![Entry {
            path: path.to_path_buf(),
            name: name.to_string(),
            name_key: name.to_ascii_lowercase(),
            kind: EntryKind::File,
            size: metadata.len(),
            modified: metadata.modified().ok(),
            readonly: false,
        }];
        app.selected = 0;
        app.frame_state.preview_content_area = Some(Rect {
            x: 2,
            y: 3,
            width: 48,
            height: 20,
        });
        app.frame_state.metrics.cols = 1;
        app.frame_state.metrics.rows_visible = 6;
    }

    fn build_selected_static_image_app(label: &str, file_name: &str) -> (App, PathBuf, PathBuf) {
        let root = temp_root(label);
        fs::create_dir_all(&root).expect("failed to create temp root");
        let image_path = root.join(file_name);
        write_test_raster_image(&image_path, ImageFormat::Png, 600, 300);

        let mut app = App::new_at(root.clone()).expect("app should initialize");
        configure_terminal_image_support(&mut app);
        app.pdf_preview.pdf_tools_available = true;
        set_single_test_entry(&mut app, &image_path);
        app.refresh_preview();

        (app, root, image_path)
    }

    fn ready_static_image_overlay(app: &mut App) -> StaticImageOverlayRequest {
        app.image_preview.selection_activation_delay = Duration::ZERO;
        app.sync_image_preview_selection_activation();
        app.active_static_image_overlay_request()
            .expect("static image overlay request should exist")
    }

    #[test]
    fn kitty_png_overlay_uses_source_path_for_direct_display() {
        let (mut app, root, image_path) =
            build_selected_static_image_app("direct-source", "demo.png");
        let request = ready_static_image_overlay(&mut app);
        let key = StaticImageKey::from_request(&request);

        match app.prepared_static_image_for_overlay(&request) {
            StaticImageOverlayPreparation::Ready(prepared) => {
                assert_eq!(prepared.display_path, image_path);
                assert_eq!(
                    prepared.dimensions,
                    RenderedImageDimensions {
                        width_px: 600,
                        height_px: 300,
                    }
                );
                assert!(prepared.inline_payload.is_none());
            }
            StaticImageOverlayPreparation::Pending => {
                panic!("png source path should display directly in kitty")
            }
            StaticImageOverlayPreparation::Failed => {
                panic!("png source path should not fail direct display")
            }
        }

        assert!(app.image_preview.dimensions.contains_key(&key));
        assert!(!app.image_preview.pending_prepares.contains(&key));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn cached_rendered_overlay_reuses_cached_path_and_inline_payload() {
        let (mut app, root, image_path) =
            build_selected_static_image_app("cache-reuse", "demo.png");
        let mut request = ready_static_image_overlay(&mut app);
        request.force_render_to_cache = true;
        request.prepare_inline_payload = true;
        let key = StaticImageKey::from_request(&request);
        let rendered_path = root.join("demo-rendered.png");
        write_test_raster_image(&rendered_path, ImageFormat::Png, 320, 180);
        let payload: Arc<str> = Arc::from("YWJj");

        app.image_preview.dimensions.insert(
            key.clone(),
            RenderedImageDimensions {
                width_px: 320,
                height_px: 180,
            },
        );
        app.remember_rendered_static_image(key.clone(), rendered_path.clone());
        app.remember_static_image_inline_payload(key.clone(), Arc::clone(&payload));

        let prepared = app
            .cached_prepared_static_image_for_overlay(&key, &request)
            .expect("cached rendered overlay should be reused");

        assert_eq!(prepared.display_path, rendered_path);
        assert_eq!(
            prepared.dimensions,
            RenderedImageDimensions {
                width_px: 320,
                height_px: 180,
            }
        );
        assert_eq!(prepared.inline_payload.as_deref(), Some(payload.as_ref()));
        assert_ne!(prepared.display_path, image_path);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn repeated_present_static_image_overlay_is_a_noop_when_nothing_changed() {
        let (mut app, root, _image_path) =
            build_selected_static_image_app("no-op-render", "demo.png");
        app.image_preview.selection_activation_delay = Duration::ZERO;
        app.sync_image_preview_selection_activation();

        let mut first = Vec::new();
        let first_state = app
            .present_static_image_overlay(ImageProtocol::KittyGraphics, &[], &mut first)
            .expect("first static image presentation should succeed");
        assert_eq!(first_state, OverlayPresentState::Displayed);
        assert!(!first.is_empty());
        assert!(app.static_image_overlay_displayed());

        let mut second = Vec::new();
        let second_state = app
            .present_static_image_overlay(ImageProtocol::KittyGraphics, &[], &mut second)
            .expect("repeat static image presentation should succeed");
        assert_eq!(second_state, OverlayPresentState::Displayed);
        assert!(second.is_empty(), "unchanged image should not redraw");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn exclusion_only_updates_redraw_without_clearing_the_existing_image() {
        let (mut app, root, _image_path) =
            build_selected_static_image_app("excluded-redraw", "demo.png");
        app.image_preview.selection_activation_delay = Duration::ZERO;
        app.sync_image_preview_selection_activation();

        let mut initial = Vec::new();
        app.present_static_image_overlay(ImageProtocol::KittyGraphics, &[], &mut initial)
            .expect("initial static image presentation should succeed");

        let excluded = [Rect {
            x: 4,
            y: 5,
            width: 6,
            height: 3,
        }];
        let mut updated = Vec::new();
        let state = app
            .present_static_image_overlay(ImageProtocol::KittyGraphics, &excluded, &mut updated)
            .expect("excluded-only redraw should succeed");
        let output = String::from_utf8(updated).expect("kitty redraw should be utf8");

        assert_eq!(state, OverlayPresentState::Displayed);
        assert!(
            !output.is_empty(),
            "changed exclusions should trigger a redraw"
        );
        assert!(
            !output.contains(build_kitty_clear_sequence()),
            "exclusion-only redraw should not clear the previous image first"
        );
        assert_eq!(app.image_preview.displayed_excluded, excluded);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
