use super::terminal_images::{
    OverlayPresentState, RenderedImageDimensions, TerminalImageBackend, TerminalWindowSize,
    command_exists, fit_image_area, place_terminal_image,
};
use super::*;
use anyhow::Result;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, imageops::FilterType};
use quick_xml::{Reader, events::Event};
use ratatui::layout::Rect;
use std::{
    collections::{HashMap, HashSet, VecDeque, hash_map::DefaultHasher},
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Instant, SystemTime},
};

const STATIC_IMAGE_RENDER_CACHE_LIMIT: usize = 24;
const STATIC_IMAGE_PRELOAD_LIMIT: usize = 6;

#[derive(Clone, Debug, Default)]
pub(super) struct ImagePreviewState {
    pub(super) dimensions: HashMap<StaticImageKey, RenderedImageDimensions>,
    pub(super) rendered_images: HashMap<StaticImageKey, PathBuf>,
    pub(super) render_order: VecDeque<StaticImageKey>,
    pub(super) failed_images: HashSet<StaticImageKey>,
    pub(super) pending_prepares: HashSet<StaticImageKey>,
    displayed: Option<DisplayedStaticImagePreview>,
    activation_ready_at: Option<Instant>,
    magick_available: Option<bool>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct StaticImageKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    target_width_px: u32,
    target_height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StaticImageOverlayRequest {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) area: Rect,
    pub(super) target_width_px: u32,
    pub(super) target_height_px: u32,
}

pub(super) struct PreparedStaticImage {
    pub(super) display_path: PathBuf,
    pub(super) dimensions: RenderedImageDimensions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DisplayedStaticImagePreview {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    area: Rect,
}

#[derive(Debug)]
pub(super) struct PreparedStaticImageAsset {
    pub(super) display_path: PathBuf,
    pub(super) dimensions: RenderedImageDimensions,
}

pub(super) enum StaticImageOverlayPreparation {
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
    }

    pub(crate) fn pending_image_preview_timer(&self) -> Option<Duration> {
        self.image_preview
            .activation_ready_at
            .map(|ready_at| ready_at.saturating_duration_since(Instant::now()))
    }

    pub(super) fn preview_prefers_static_image_surface(&self) -> bool {
        let Some(request) = self.active_static_image_overlay_request() else {
            return false;
        };
        !self
            .image_preview
            .failed_images
            .contains(&StaticImageKey::from_request(&request))
    }

    pub(super) fn static_image_preview_header_detail(&self) -> Option<String> {
        let request = self.active_static_image_overlay_request()?;
        let dimensions = self
            .image_preview
            .dimensions
            .get(&StaticImageKey::from_request(&request))
            .copied()?;
        Some(format!("{}x{}", dimensions.width_px, dimensions.height_px))
    }

    pub(super) fn should_defer_static_image_preview(&self, entry: &Entry) -> bool {
        static_image_detail_label(entry).is_some() && self.preview_prefers_static_image_surface()
    }

    pub(super) fn static_image_preview_detail(&self, entry: &Entry) -> Option<&'static str> {
        static_image_detail_label(entry)
    }

    pub(super) fn active_static_image_overlay_request(&self) -> Option<StaticImageOverlayRequest> {
        let entry = self.selected_entry()?;
        self.static_image_overlay_request_for_entry(entry)
    }

    pub(super) fn present_static_image_overlay(
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

    pub(super) fn prepared_static_image_for_overlay(
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
        if self.image_preview.failed_images.contains(&key) {
            StaticImageOverlayPreparation::Failed
        } else {
            StaticImageOverlayPreparation::Pending
        }
    }

    pub(super) fn clear_failed_static_image_state_if_needed(&mut self) {
        if let Some(entry) = self.selected_entry()
            && static_image_detail_label(entry).is_none()
        {
            self.image_preview.failed_images.clear();
        }
    }

    pub(super) fn sync_image_preview_selection_activation(&mut self) {
        self.image_preview.activation_ready_at =
            self.active_static_image_overlay_request().and_then(|_| {
                let ready_at = self.last_selection_change_at + IMAGE_SELECTION_ACTIVATION_DELAY;
                (Instant::now() < ready_at).then_some(ready_at)
            });
    }

    pub(super) fn mark_static_image_failed(&mut self, request: &StaticImageOverlayRequest) {
        self.image_preview
            .failed_images
            .insert(StaticImageKey::from_request(request));
    }

    fn magick_available(&mut self) -> bool {
        *self
            .image_preview
            .magick_available
            .get_or_insert_with(|| command_exists("magick"))
    }

    pub(super) fn image_selection_activation_ready(&self) -> bool {
        self.image_preview.activation_ready_at.is_none()
    }

    pub(super) fn static_image_overlay_displayed(&self) -> bool {
        self.image_preview.displayed.is_some()
    }

    pub(super) fn clear_displayed_static_image(&mut self) {
        self.image_preview.displayed = None;
    }

    pub(super) fn displayed_static_image_matches_active(&self) -> bool {
        self.active_static_image_display_target()
            .as_ref()
            .zip(self.image_preview.displayed.as_ref())
            .is_some_and(|(active, displayed)| active == displayed)
    }

    pub(super) fn refresh_static_image_preloads(&mut self) {
        let current = self.active_static_image_overlay_request();
        let nearby = self.nearby_static_image_overlay_requests(current.as_ref());
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

    pub(super) fn apply_image_prepare_build(&mut self, build: jobs::ImagePrepareBuild) -> bool {
        let key = StaticImageKey::from_parts(
            build.path.clone(),
            build.size,
            build.modified,
            build.target_width_px,
            build.target_height_px,
        );
        self.image_preview.pending_prepares.remove(&key);
        let is_current = self
            .active_static_image_overlay_request()
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
        if is_png_path(&key.path) && key.path.exists() {
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
            magick_available: self.magick_available(),
        }
    }

    fn active_static_image_display_target(&self) -> Option<DisplayedStaticImagePreview> {
        let request = self.active_static_image_overlay_request()?;
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
    pub(super) fn from_request(request: &StaticImageOverlayRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            target_width_px: request.target_width_px,
            target_height_px: request.target_height_px,
        }
    }

    pub(super) fn from_parts(
        path: PathBuf,
        size: u64,
        modified: Option<SystemTime>,
        target_width_px: u32,
        target_height_px: u32,
    ) -> Self {
        Self {
            path,
            size,
            modified,
            target_width_px,
            target_height_px,
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
        }
    }
}

pub(super) fn static_image_detail_label(entry: &Entry) -> Option<&'static str> {
    let extension = entry.path.extension()?.to_str()?;
    match extension.to_ascii_lowercase().as_str() {
        "png" => Some("PNG image"),
        "jpg" | "jpeg" => Some("JPEG image"),
        "gif" => Some("GIF image"),
        "webp" => Some("WebP image"),
        "svg" => Some("SVG image"),
        _ => None,
    }
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
    let source_dimensions = if is_svg_path(&request.path) {
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
    );

    if is_svg_path(&request.path) {
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

    if is_png_path(&request.path)
        && source_dimensions.width_px <= target_width_px
        && source_dimensions.height_px <= target_height_px
    {
        return Some(PreparedStaticImageAsset {
            display_path: request.path.clone(),
            dimensions: source_dimensions,
        });
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

    let image = ImageReader::open(&request.path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
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

fn is_svg_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

fn is_png_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
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

fn read_raster_dimensions(path: &Path) -> Option<RenderedImageDimensions> {
    let (width_px, height_px) = image::image_dimensions(path).ok()?;
    (width_px > 0 && height_px > 0).then_some(RenderedImageDimensions {
        width_px,
        height_px,
    })
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

fn image_target_width_px(area: Rect, window_size: Option<TerminalWindowSize>) -> u32 {
    let (cell_width_px, _) = image_cell_pixels(window_size);
    (f32::from(area.width.max(1)) * cell_width_px)
        .round()
        .max(1.0) as u32
}

fn image_target_height_px(area: Rect, window_size: Option<TerminalWindowSize>) -> u32 {
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
