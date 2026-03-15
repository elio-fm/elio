use super::super::*;
use super::inline_image::{
    OverlayPresentState, RenderedImageDimensions, TerminalImageBackend, TerminalWindowSize,
    command_exists, fit_image_area, place_terminal_image,
};
use anyhow::Result;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, imageops::FilterType};
use quick_xml::{Reader, events::Event};
use ratatui::layout::Rect;
use std::{
    collections::{HashMap, HashSet, VecDeque, hash_map::DefaultHasher},
    env, fs,
    fs::File,
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Instant, SystemTime},
};

const STATIC_IMAGE_RENDER_CACHE_LIMIT: usize = 24;
const STATIC_IMAGE_PRELOAD_LIMIT: usize = 6;
const STATIC_IMAGE_INLINE_FALLBACK_PREPARE_MAX_BYTES: u64 = 512 * 1024;
const STATIC_IMAGE_INLINE_EXTERNAL_PREPARE_MAX_BYTES: u64 = 16 * 1024 * 1024;
const FAST_FORCE_RENDER_FFMPEG_RASTER_ARGS: [&str; 4] =
    ["-compression_level", "1", "-sws_flags", "fast_bilinear"];
const DEFAULT_FFMPEG_RASTER_ARGS: [&str; 0] = [];

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct ImagePreviewState {
    pub(super) dimensions: HashMap<StaticImageKey, RenderedImageDimensions>,
    pub(super) rendered_images: HashMap<StaticImageKey, PathBuf>,
    pub(super) render_order: VecDeque<StaticImageKey>,
    pub(super) failed_images: HashSet<StaticImageKey>,
    pub(super) pending_prepares: HashSet<StaticImageKey>,
    displayed: Option<DisplayedStaticImagePreview>,
    activation_ready_at: Option<Instant>,
    ffmpeg_available: Option<bool>,
    magick_available: Option<bool>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app) struct StaticImageKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    target_width_px: u32,
    target_height_px: u32,
    force_render_to_cache: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum StaticImageOverlayMode {
    FullPane,
    Inline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct StaticImageOverlayRequest {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) area: Rect,
    pub(super) target_width_px: u32,
    pub(super) target_height_px: u32,
    pub(super) mode: StaticImageOverlayMode,
    pub(super) force_render_to_cache: bool,
}

pub(in crate::app) struct PreparedStaticImage {
    pub(super) display_path: PathBuf,
    pub(super) dimensions: RenderedImageDimensions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DisplayedStaticImagePreview {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    area: Rect,
    mode: StaticImageOverlayMode,
}

#[derive(Debug)]
pub(in crate::app) struct PreparedStaticImageAsset {
    pub(super) display_path: PathBuf,
    pub(super) dimensions: RenderedImageDimensions,
}

pub(in crate::app) enum StaticImageOverlayPreparation {
    Ready(PreparedStaticImage),
    Pending,
    Failed,
}

impl App {
    pub(crate) fn process_image_preview_timers(&mut self) -> bool {
        let Some(ready_at) = self.image_preview.activation_ready_at else {
            return false;
        };
        if Instant::now() < ready_at {
            return false;
        }
        self.image_preview.activation_ready_at = None;
        self.active_static_image_overlay_request().is_some()
            || self.active_preview_visual_overlay_request().is_some()
    }

    pub(crate) fn pending_image_preview_timer(&self) -> Option<Duration> {
        self.image_preview
            .activation_ready_at
            .map(|ready_at| ready_at.saturating_duration_since(Instant::now()))
    }

    pub(in crate::app) fn preview_prefers_static_image_surface(&self) -> bool {
        let Some(request) = self.active_static_image_overlay_request() else {
            return false;
        };
        !self
            .image_preview
            .failed_images
            .contains(&StaticImageKey::from_request(&request))
    }

    pub(in crate::app) fn static_image_preview_header_detail(&self) -> Option<String> {
        let request = self.active_static_image_overlay_request()?;
        let dimensions = self
            .image_preview
            .dimensions
            .get(&StaticImageKey::from_request(&request))
            .copied()?;
        Some(format!("{}x{}", dimensions.width_px, dimensions.height_px))
    }

    pub(in crate::app) fn should_defer_static_image_preview(&self, entry: &Entry) -> bool {
        static_image_detail_label(entry).is_some() && self.preview_prefers_static_image_surface()
    }

    pub(in crate::app) fn static_image_preview_detail(
        &self,
        entry: &Entry,
    ) -> Option<&'static str> {
        static_image_detail_label(entry)
    }

    pub(in crate::app) fn static_image_overlay_placeholder_message(&self) -> Option<String> {
        if !self.preview_prefers_static_image_surface() || self.preview_uses_image_overlay() {
            return None;
        }

        let request = self.active_static_image_overlay_request()?;
        let key = StaticImageKey::from_request(&request);
        if self.image_preview.failed_images.contains(&key) {
            return Some("Image preview unavailable".to_string());
        }
        if !self.image_selection_activation_ready() {
            return None;
        }
        if self.static_image_can_prepare_inline_now(&request) {
            return None;
        }
        if self.image_preview.dimensions.contains_key(&key) {
            return None;
        }
        if self.image_preview.pending_prepares.contains(&key) {
            return Some("Preparing image preview".to_string());
        }
        Some("Preparing image preview".to_string())
    }

    pub(in crate::app) fn active_static_image_overlay_request(
        &self,
    ) -> Option<StaticImageOverlayRequest> {
        let entry = self.selected_entry()?;
        self.static_image_overlay_request_for_entry(entry)
    }

    pub(in crate::app) fn present_static_image_overlay(
        &mut self,
        backend: TerminalImageBackend,
    ) -> Result<OverlayPresentState> {
        let Some(request) = self.active_static_image_overlay_request() else {
            return Ok(OverlayPresentState::NotRequested);
        };
        if !self.image_selection_activation_ready() {
            return Ok(OverlayPresentState::Waiting);
        }

        let prepared = match self.prepared_static_image_for_overlay(&request) {
            StaticImageOverlayPreparation::Ready(prepared) => prepared,
            StaticImageOverlayPreparation::Pending => return Ok(OverlayPresentState::Waiting),
            StaticImageOverlayPreparation::Failed => {
                self.mark_static_image_failed(&request);
                self.refresh_preview();
                return Ok(OverlayPresentState::NotRequested);
            }
        };
        let Some(window_size) = self.cached_terminal_window() else {
            self.mark_static_image_failed(&request);
            self.refresh_preview();
            return Ok(OverlayPresentState::NotRequested);
        };
        let placement = fit_image_area(
            request.area,
            window_size,
            prepared.dimensions.width_px as f32 / prepared.dimensions.height_px as f32,
        );
        let displayed = DisplayedStaticImagePreview::from_request(&request, placement);
        if self.image_preview.displayed.as_ref() == Some(&displayed) {
            return Ok(OverlayPresentState::Displayed);
        }
        if place_terminal_image(backend, &prepared.display_path, placement).is_err() {
            self.mark_static_image_failed(&request);
            self.refresh_preview();
            return Ok(OverlayPresentState::NotRequested);
        }

        self.image_preview.displayed = Some(displayed);
        Ok(OverlayPresentState::Displayed)
    }

    pub(in crate::app) fn present_preview_visual_overlay(
        &mut self,
        backend: TerminalImageBackend,
    ) -> Result<OverlayPresentState> {
        let Some(request) = self.active_preview_visual_overlay_request() else {
            return Ok(OverlayPresentState::NotRequested);
        };
        if !self.image_selection_activation_ready() {
            return Ok(OverlayPresentState::Waiting);
        }

        let prepared = match self.prepared_static_image_for_overlay(&request) {
            StaticImageOverlayPreparation::Ready(prepared) => prepared,
            StaticImageOverlayPreparation::Pending => return Ok(OverlayPresentState::Waiting),
            StaticImageOverlayPreparation::Failed => {
                self.mark_static_image_failed(&request);
                return Ok(OverlayPresentState::NotRequested);
            }
        };
        let Some(window_size) = self.cached_terminal_window() else {
            self.mark_static_image_failed(&request);
            return Ok(OverlayPresentState::NotRequested);
        };
        let placement = fit_image_area(
            request.area,
            window_size,
            prepared.dimensions.width_px as f32 / prepared.dimensions.height_px as f32,
        );
        let displayed = DisplayedStaticImagePreview::from_request(&request, placement);
        if self.image_preview.displayed.as_ref() == Some(&displayed) {
            return Ok(OverlayPresentState::Displayed);
        }
        if place_terminal_image(backend, &prepared.display_path, placement).is_err() {
            self.mark_static_image_failed(&request);
            return Ok(OverlayPresentState::NotRequested);
        }

        self.image_preview.displayed = Some(displayed);
        Ok(OverlayPresentState::Displayed)
    }

    pub(in crate::app) fn prepared_static_image_for_overlay(
        &mut self,
        request: &StaticImageOverlayRequest,
    ) -> StaticImageOverlayPreparation {
        let key = StaticImageKey::from_request(request);
        if let Some(dimensions) = self.image_preview.dimensions.get(&key).copied()
            && let Some(display_path) = self.cached_static_image_display_path(&key)
        {
            return StaticImageOverlayPreparation::Ready(PreparedStaticImage {
                display_path,
                dimensions,
            });
        }
        if let Some(prepared) = self.try_prepare_current_static_image_inline(request) {
            return StaticImageOverlayPreparation::Ready(prepared);
        }
        if self.image_preview.failed_images.contains(&key) {
            StaticImageOverlayPreparation::Failed
        } else {
            StaticImageOverlayPreparation::Pending
        }
    }

    pub(in crate::app) fn clear_failed_static_image_state_if_needed(&mut self) {
        if let Some(entry) = self.selected_entry()
            && static_image_detail_label(entry).is_none()
        {
            self.image_preview.failed_images.clear();
        }
    }

    pub(in crate::app) fn sync_image_preview_selection_activation(&mut self) {
        self.image_preview.activation_ready_at = self
            .active_static_image_overlay_request()
            .or_else(|| self.active_preview_visual_overlay_request())
            .and_then(|_| {
                let ready_at = self.last_selection_change_at + IMAGE_SELECTION_ACTIVATION_DELAY;
                (Instant::now() < ready_at).then_some(ready_at)
            });
    }

    pub(in crate::app) fn mark_static_image_failed(&mut self, request: &StaticImageOverlayRequest) {
        self.image_preview
            .failed_images
            .insert(StaticImageKey::from_request(request));
    }

    fn try_prepare_current_static_image_inline(
        &mut self,
        request: &StaticImageOverlayRequest,
    ) -> Option<PreparedStaticImage> {
        let format = static_image_format_for_path(&request.path)?;
        if !should_prepare_static_image_inline(request, format, self.ffmpeg_available()) {
            return None;
        }

        let key = StaticImageKey::from_request(request);
        let prepared =
            prepare_static_image_asset(&self.image_prepare_request_for_overlay(request), || false)?;
        self.image_preview.failed_images.remove(&key);
        self.image_preview
            .dimensions
            .insert(key.clone(), prepared.dimensions);
        if prepared.display_path != request.path {
            self.remember_rendered_static_image(key, prepared.display_path.clone());
        }

        Some(PreparedStaticImage {
            display_path: prepared.display_path,
            dimensions: prepared.dimensions,
        })
    }

    fn static_image_can_prepare_inline_now(&self, request: &StaticImageOverlayRequest) -> bool {
        let Some(format) = static_image_format_for_path(&request.path) else {
            return false;
        };
        let ffmpeg_available = self
            .image_preview
            .ffmpeg_available
            .unwrap_or_else(|| command_exists("ffmpeg"));
        should_prepare_static_image_inline(request, format, ffmpeg_available)
    }

    fn magick_available(&mut self) -> bool {
        *self
            .image_preview
            .magick_available
            .get_or_insert_with(|| command_exists("magick"))
    }

    fn ffmpeg_available(&mut self) -> bool {
        *self
            .image_preview
            .ffmpeg_available
            .get_or_insert_with(|| command_exists("ffmpeg"))
    }

    pub(in crate::app) fn image_selection_activation_ready(&self) -> bool {
        self.image_preview.activation_ready_at.is_none()
    }

    pub(in crate::app) fn static_image_overlay_displayed(&self) -> bool {
        self.image_preview.displayed.is_some()
    }

    pub(in crate::app) fn clear_displayed_static_image(&mut self) {
        self.image_preview.displayed = None;
    }

    pub(in crate::app) fn displayed_static_image_matches_active(&self) -> bool {
        self.active_static_image_display_target()
            .as_ref()
            .zip(self.image_preview.displayed.as_ref())
            .is_some_and(|(active, displayed)| active == displayed)
    }

    pub(in crate::app) fn keep_displayed_comic_preview_overlay_while_pending(&self) -> bool {
        let Some(displayed) = self.image_preview.displayed.as_ref() else {
            return false;
        };
        if displayed.mode != StaticImageOverlayMode::Inline
            || self.preview_state.content.kind != preview::PreviewKind::Comic
        {
            return false;
        }

        let loading_current_comic_page =
            self.preview_state
                .load_state
                .as_ref()
                .is_some_and(|load_state| {
                    let loading_path = match load_state {
                        PreviewLoadState::Placeholder(path)
                        | PreviewLoadState::Refreshing(path) => path,
                    };
                    self.selected_entry()
                        .is_some_and(|entry| entry.path == *loading_path)
                });
        if loading_current_comic_page {
            return true;
        }

        let Some(request) = self.active_preview_visual_overlay_request_unchecked() else {
            return false;
        };
        let key = StaticImageKey::from_request(&request);
        if !request.force_render_to_cache || self.image_preview.failed_images.contains(&key) {
            return false;
        }
        if !self.image_selection_activation_ready() {
            return true;
        }

        self.image_preview.pending_prepares.contains(&key)
            || (self.static_image_can_prepare_inline_now(&request)
                && !self.image_preview.dimensions.contains_key(&key))
    }

    pub(in crate::app) fn displayed_static_image_replaces_preview(&self) -> bool {
        self.image_preview
            .displayed
            .as_ref()
            .is_some_and(|displayed| displayed.mode == StaticImageOverlayMode::FullPane)
            && self.displayed_static_image_matches_active()
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
            self.nearby_comic_preview_visual_overlay_requests()
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

        if let Some(request) = current.as_ref() {
            self.ensure_static_image_preload(request, jobs::ImageJobPriority::Current);
        }
        for request in &nearby {
            self.ensure_static_image_preload(request, jobs::ImageJobPriority::Nearby);
        }
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
        if key.path.exists()
            && static_image_format_for_path(&key.path) == Some(StaticImageFormat::Png)
        {
            return Some(key.path.clone());
        }

        self.image_preview.rendered_images.remove(key);
        self.image_preview
            .render_order
            .retain(|queued| queued != key);
        None
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

    fn static_image_overlay_request_for_entry(
        &self,
        entry: &Entry,
    ) -> Option<StaticImageOverlayRequest> {
        if !self.terminal_image_overlay_available() {
            return None;
        }
        static_image_detail_label(entry)?;

        let area = self.frame_state.preview_content_area?;
        if area.width == 0 || area.height == 0 {
            return None;
        }

        Some(StaticImageOverlayRequest {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
            area,
            target_width_px: image_target_width_px(area, self.cached_terminal_window()),
            target_height_px: image_target_height_px(area, self.cached_terminal_window()),
            mode: StaticImageOverlayMode::FullPane,
            force_render_to_cache: false,
        })
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
            || self.image_preview.dimensions.contains_key(&key)
                && self.cached_static_image_display_path(&key).is_some()
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
            magick_available: self.magick_available(),
            force_render_to_cache: request.force_render_to_cache,
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
            fit_image_area(
                request.area,
                window_size,
                image_dimensions.width_px as f32 / image_dimensions.height_px as f32,
            ),
        ))
    }
}

impl StaticImageKey {
    pub(in crate::app) fn from_request(request: &StaticImageOverlayRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            target_width_px: request.target_width_px,
            target_height_px: request.target_height_px,
            force_render_to_cache: request.force_render_to_cache,
        }
    }

    pub(in crate::app) fn from_parts(
        path: PathBuf,
        size: u64,
        modified: Option<SystemTime>,
        target_width_px: u32,
        target_height_px: u32,
        force_render_to_cache: bool,
    ) -> Self {
        Self {
            path,
            size,
            modified,
            target_width_px,
            target_height_px,
            force_render_to_cache,
        }
    }
}

impl DisplayedStaticImagePreview {
    fn from_request(request: &StaticImageOverlayRequest, area: Rect) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            area,
            mode: request.mode,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaticImageFormat {
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
    let format = static_image_format_for_path(&request.path)?;
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
    );

    if format == StaticImageFormat::Svg {
        let cache_path = static_image_render_cache_path(&key)?;
        if cache_path.exists() {
            return Some(PreparedStaticImageAsset {
                display_path: cache_path,
                dimensions: source_dimensions,
            });
        }
        if request.magick_available
            && render_svg_to_png(
                &request.path,
                &cache_path,
                target_width_px,
                target_height_px,
                &canceled,
            )
        {
            return Some(PreparedStaticImageAsset {
                display_path: cache_path,
                dimensions: source_dimensions,
            });
        }
        return None;
    }

    if format == StaticImageFormat::Png {
        if !request.force_render_to_cache {
            return Some(PreparedStaticImageAsset {
                display_path: request.path.clone(),
                dimensions: source_dimensions,
            });
        }
    }

    let cache_path = static_image_render_cache_path(&key)?;
    if cache_path.exists() {
        return Some(PreparedStaticImageAsset {
            display_path: cache_path,
            dimensions: source_dimensions,
        });
    }
    if canceled() {
        return None;
    }

    if request.ffmpeg_available
        && should_render_raster_with_ffmpeg(format)
        && render_raster_to_png_with_ffmpeg(
            &request.path,
            &cache_path,
            target_width_px,
            target_height_px,
            request.force_render_to_cache,
            &canceled,
        )
    {
        return Some(PreparedStaticImageAsset {
            display_path: cache_path,
            dimensions: source_dimensions,
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
    image.save_with_format(&cache_path, ImageFormat::Png).ok()?;

    Some(PreparedStaticImageAsset {
        display_path: cache_path,
        dimensions: source_dimensions,
    })
}

fn static_image_render_cache_path(key: &StaticImageKey) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let cache_dir = env::temp_dir().join("elio-image-preview");
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!("image-{:016x}.png", hasher.finish())))
}

fn should_prepare_static_image_inline(
    request: &StaticImageOverlayRequest,
    format: StaticImageFormat,
    ffmpeg_available: bool,
) -> bool {
    if request.force_render_to_cache {
        return false;
    }

    match format {
        StaticImageFormat::Png => true,
        StaticImageFormat::Jpeg | StaticImageFormat::Gif | StaticImageFormat::Webp => {
            if ffmpeg_available {
                request.size <= STATIC_IMAGE_INLINE_EXTERNAL_PREPARE_MAX_BYTES
            } else {
                request.size <= STATIC_IMAGE_INLINE_FALLBACK_PREPARE_MAX_BYTES
            }
        }
        StaticImageFormat::Svg => false,
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

fn static_image_format_for_path(path: &Path) -> Option<StaticImageFormat> {
    crate::file_info::inspect_path(path, EntryKind::File)
        .specific_type_label
        .and_then(StaticImageFormat::from_label)
}

fn render_svg_to_png(
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
