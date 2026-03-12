mod backend;
mod geometry;
mod pipeline;

pub(crate) use self::pipeline::{probe_pdf_page, render_pdf_page_to_cache};
#[cfg(test)]
use self::{
    backend::{
        build_kitty_clear_sequence, build_kitty_display_sequence, fallback_window_size_pixels,
        parse_window_size, select_terminal_image_backend,
    },
    geometry::bucket_render_dimensions,
    pipeline::{parse_pdfinfo_page_count, parse_pdfinfo_page_dimensions},
};
use self::{
    backend::{
        clear_pdf_images, detect_terminal_pdf_preview_backend, place_pdf_image,
        query_terminal_window_size, read_png_dimensions,
    },
    geometry::{fit_image_area, fit_pdf_page},
};
use super::*;
use crate::file_facts::{self, DocumentFormat};
use anyhow::{Context, Result};
#[cfg(test)]
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use ratatui::layout::Rect;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    hash::Hash,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

const PDF_RENDER_CACHE_LIMIT: usize = 12;
const PDF_RENDER_BUCKET_PX: u32 = 64;
const PDF_RENDER_MIN_DIMENSION_PX: u32 = 96;
const PDF_PAGE_MIN: usize = 1;
const PDF_PAGE_STATUS_PREFIX: &str = "PDF page ";
const PDF_PREFETCH_DISTANCE: usize = 1;
const PDF_SELECTION_ACTIVATION_DELAY: Duration = Duration::from_millis(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalImageBackend {
    KittyProtocol,
    Kitten,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PdfPreviewState {
    enabled: bool,
    backend: Option<TerminalImageBackend>,
    session: Option<PdfSession>,
    document_page_counts: HashMap<PdfDocumentKey, usize>,
    page_dimensions: HashMap<PdfPageKey, PdfPageDimensions>,
    pending_page_probes: HashSet<PdfPageKey>,
    failed_page_probes: HashSet<PdfPageKey>,
    rendered_pages: HashMap<PdfRenderKey, PathBuf>,
    rendered_page_dimensions: HashMap<PdfRenderKey, RenderedImageDimensions>,
    render_order: VecDeque<PdfRenderKey>,
    pending_renders: HashSet<PdfRenderKey>,
    failed_renders: HashSet<PdfRenderKey>,
    displayed: Option<DisplayedPdfPreview>,
    terminal_window: Option<TerminalWindowSize>,
    activation_ready_at: Option<Instant>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PdfSession {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    current_page: usize,
    total_pages: Option<usize>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PdfDocumentKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PdfPageKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    page: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PdfPageDimensions {
    width_pts: f32,
    height_pts: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RenderedImageDimensions {
    width_px: u32,
    height_px: u32,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PdfRenderKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    page: usize,
    width_px: u32,
    height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DisplayedPdfPreview {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    page: usize,
    area: Rect,
    render_width_px: u32,
    render_height_px: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerminalWindowSize {
    cells_width: u16,
    cells_height: u16,
    pixels_width: u32,
    pixels_height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PdfOverlayRequest {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    page: usize,
    area: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FittedPdfPlacement {
    image_area: Rect,
    render_width_px: u32,
    render_height_px: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct PdfProbeResult {
    pub total_pages: Option<usize>,
    pub width_pts: Option<f32>,
    pub height_pts: Option<f32>,
}

impl App {
    pub(crate) fn enable_terminal_pdf_previews(&mut self) {
        self.pdf_preview.backend = detect_terminal_pdf_preview_backend();
        self.pdf_preview.enabled = self.pdf_preview.backend.is_some();
        self.refresh_pdf_terminal_window_size();
        self.sync_pdf_preview_selection();
    }

    pub(crate) fn handle_pdf_terminal_resize(&mut self) {
        self.refresh_pdf_terminal_window_size();
        if self.pdf_preview.session.is_some() {
            self.pdf_preview.activation_ready_at = None;
            self.refresh_pdf_prefetch_window();
        }
    }

    pub(crate) fn process_pdf_preview_timers(&mut self) -> bool {
        let Some(ready_at) = self.pdf_preview.activation_ready_at else {
            return false;
        };
        if Instant::now() < ready_at {
            return false;
        }

        self.pdf_preview.activation_ready_at = None;
        self.pdf_preview.session.is_some()
    }

    pub(crate) fn pending_pdf_preview_timer(&self) -> Option<Duration> {
        self.pdf_preview
            .activation_ready_at
            .map(|ready_at| ready_at.saturating_duration_since(Instant::now()))
    }

    pub(crate) fn present_pdf_overlay(&mut self) -> Result<()> {
        let Some(backend) = self.pdf_preview.backend else {
            self.clear_pdf_overlay()?;
            return Ok(());
        };

        if self
            .pdf_preview
            .displayed
            .as_ref()
            .is_some_and(|displayed| !self.should_keep_displayed_pdf_overlay(displayed))
        {
            self.clear_pdf_overlay()?;
        }

        let Some(request) = self.active_pdf_overlay_request() else {
            self.clear_pdf_overlay()?;
            return Ok(());
        };

        if !self.pdf_selection_activation_ready() {
            return Ok(());
        }

        let Some(requested_placement) = self.overlay_placement_for_request(&request) else {
            let _ = self.ensure_pdf_page_probe(&request);
            return Ok(());
        };
        let render_key = PdfRenderKey::from_request(&request, requested_placement);
        let Some(rendered) = self.ensure_pdf_render(&render_key) else {
            return Ok(());
        };
        let placement = self.resolved_pdf_display_placement(
            &request,
            &render_key,
            requested_placement,
            &rendered,
        );
        let displayed = DisplayedPdfPreview::from_request(&request, placement);
        if self.pdf_preview.displayed.as_ref() == Some(&displayed) {
            return Ok(());
        }

        if self.pdf_preview.displayed.is_some() {
            clear_pdf_images(backend).context("failed to clear previous PDF page")?;
            self.pdf_preview.displayed = None;
        }
        place_pdf_image(backend, &rendered, placement.image_area)
            .context("failed to display PDF page")?;
        self.pdf_preview.displayed = Some(displayed);
        Ok(())
    }

    pub(crate) fn clear_pdf_overlay(&mut self) -> Result<()> {
        if self.pdf_preview.displayed.is_none() {
            return Ok(());
        }
        let Some(backend) = self.pdf_preview.backend else {
            self.pdf_preview.displayed = None;
            return Ok(());
        };
        clear_pdf_images(backend).context("failed to clear PDF preview overlay")?;
        self.pdf_preview.displayed = None;
        Ok(())
    }

    pub(crate) fn preview_uses_image_overlay(&self) -> bool {
        self.active_pdf_display_target()
            .as_ref()
            .zip(self.pdf_preview.displayed.as_ref())
            .is_some_and(|(active, displayed)| active == displayed)
    }

    pub(crate) fn preview_prefers_pdf_surface(&self) -> bool {
        if !self.pdf_preview.enabled
            || self.pdf_preview.backend.is_none()
            || self.pdf_preview.session.is_none()
        {
            return false;
        }
        if self.preview_uses_image_overlay() {
            return true;
        }

        let Some(request) = self.active_pdf_overlay_request() else {
            return false;
        };
        if !self.pdf_selection_activation_ready() {
            return true;
        }

        let page_key = PdfPageKey::from_request(&request);
        if self.pdf_preview.failed_page_probes.contains(&page_key) {
            return false;
        }
        if self.pdf_preview.pending_page_probes.contains(&page_key)
            || !self.pdf_preview.page_dimensions.contains_key(&page_key)
        {
            return true;
        }

        let Some(placement) = self.overlay_placement_for_request(&request) else {
            return false;
        };
        let render_key = PdfRenderKey::from_request(&request, placement);
        if self.pdf_preview.failed_renders.contains(&render_key) {
            return false;
        }
        self.pdf_preview.pending_renders.contains(&render_key)
            || self.cached_render_exists(&render_key)
    }

    pub(crate) fn pdf_preview_placeholder_message(&self) -> Option<String> {
        if !self.preview_prefers_pdf_surface() || self.preview_uses_image_overlay() {
            return None;
        }

        let request = self.active_pdf_overlay_request()?;
        if !self.pdf_selection_activation_ready() {
            return Some("Preparing PDF preview...".to_string());
        }

        let page_key = PdfPageKey::from_request(&request);
        if self.pdf_preview.failed_page_probes.contains(&page_key) {
            return Some("PDF preview unavailable".to_string());
        }
        if !self.pdf_preview.page_dimensions.contains_key(&page_key)
            || self.pdf_preview.pending_page_probes.contains(&page_key)
        {
            return Some("Loading PDF page...".to_string());
        }

        let placement = self.overlay_placement_for_request(&request)?;
        let render_key = PdfRenderKey::from_request(&request, placement);
        if self.pdf_preview.failed_renders.contains(&render_key) {
            return Some("PDF preview unavailable".to_string());
        }
        if self.cached_render_exists(&render_key) {
            return None;
        }
        Some("Rendering PDF page...".to_string())
    }

    pub(super) fn pdf_preview_header_detail(&self) -> Option<String> {
        let session = self.pdf_preview.session.as_ref()?;
        if !self.pdf_preview.enabled {
            return None;
        }

        let page_label = match session.total_pages {
            Some(total_pages) => format!("Page {}/{}", session.current_page, total_pages),
            None => format!("Page {}", session.current_page),
        };
        Some(page_label)
    }

    pub(super) fn step_pdf_page(&mut self, delta: isize) -> bool {
        let Some(session) = &mut self.pdf_preview.session else {
            return false;
        };

        let previous_page = session.current_page;
        let next_page = if delta.is_negative() {
            session.current_page.saturating_sub(delta.unsigned_abs())
        } else {
            session.current_page.saturating_add(delta as usize)
        };

        let max_page = session.total_pages.unwrap_or(next_page.max(PDF_PAGE_MIN));
        session.current_page = next_page.clamp(PDF_PAGE_MIN, max_page.max(PDF_PAGE_MIN));
        let changed = session.current_page != previous_page;
        if changed {
            self.pdf_preview.activation_ready_at = None;
            self.refresh_pdf_prefetch_window();
        }
        changed
    }

    pub(super) fn sync_pdf_preview_selection(&mut self) {
        if !self.pdf_preview.enabled {
            self.pdf_preview.session = None;
            self.pdf_preview.activation_ready_at = None;
            self.clear_pending_pdf_work();
            self.clear_pdf_page_status();
            return;
        }

        let Some(entry) = self.selected_entry() else {
            self.pdf_preview.session = None;
            self.pdf_preview.activation_ready_at = None;
            self.clear_pending_pdf_work();
            self.clear_pdf_page_status();
            return;
        };
        if !is_pdf_entry(entry) {
            self.pdf_preview.session = None;
            self.pdf_preview.activation_ready_at = None;
            self.clear_pending_pdf_work();
            self.clear_pdf_page_status();
            return;
        }

        let should_keep_session = self.pdf_preview.session.as_ref().is_some_and(|session| {
            session.path == entry.path
                && session.size == entry.size
                && session.modified == entry.modified
        });
        if should_keep_session {
            return;
        }

        self.pdf_preview.session = Some(PdfSession {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
            current_page: PDF_PAGE_MIN,
            total_pages: self.cached_pdf_total_pages(entry),
        });
        self.pdf_preview.activation_ready_at =
            Some(Instant::now() + PDF_SELECTION_ACTIVATION_DELAY);
        self.refresh_pdf_prefetch_window();
        self.clear_pdf_page_status();
    }

    fn active_pdf_overlay_request(&self) -> Option<PdfOverlayRequest> {
        if !self.pdf_preview.enabled {
            return None;
        }

        let session = self.pdf_preview.session.as_ref()?;
        let area = self.frame_state.preview_content_area?;
        if area.width == 0 || area.height == 0 {
            return None;
        }

        Some(PdfOverlayRequest {
            path: session.path.clone(),
            size: session.size,
            modified: session.modified,
            page: session.current_page,
            area,
        })
    }

    fn ensure_pdf_render(&mut self, key: &PdfRenderKey) -> Option<PathBuf> {
        if let Some(path) = self.cached_pdf_render_path(key) {
            return Some(path);
        }
        if self.pdf_preview.failed_renders.contains(key)
            || self.pdf_preview.pending_renders.contains(key)
        {
            return None;
        }
        if !self.scheduler.submit_pdf_render(jobs::PdfRenderRequest {
            path: key.path.clone(),
            size: key.size,
            modified: key.modified,
            page: key.page,
            width_px: key.width_px,
            height_px: key.height_px,
        }) {
            self.pdf_preview.failed_renders.insert(key.clone());
            return None;
        }
        self.pdf_preview.pending_renders.insert(key.clone());
        None
    }

    fn ensure_pdf_page_probe(&mut self, request: &PdfOverlayRequest) -> Option<PdfPageDimensions> {
        let key = PdfPageKey::from_request(request);
        if let Some(dimensions) = self.pdf_preview.page_dimensions.get(&key).copied() {
            return Some(dimensions);
        }
        if self.pdf_preview.failed_page_probes.contains(&key)
            || self.pdf_preview.pending_page_probes.contains(&key)
        {
            return None;
        }
        if !self.scheduler.submit_pdf_probe(jobs::PdfProbeRequest {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
        }) {
            self.pdf_preview.failed_page_probes.insert(key);
            return None;
        }
        self.pdf_preview.pending_page_probes.insert(key);
        None
    }

    pub(super) fn apply_pdf_probe_build(&mut self, build: jobs::PdfProbeBuild) -> bool {
        let key = PdfPageKey {
            path: build.path.clone(),
            size: build.size,
            modified: build.modified,
            page: build.page,
        };
        self.pdf_preview.pending_page_probes.remove(&key);

        let current_request = self.active_pdf_overlay_request();
        let current_key = current_request.as_ref().map(PdfPageKey::from_request);
        let current_document = self
            .pdf_preview
            .session
            .as_ref()
            .map(PdfDocumentKey::from_session);

        match build.result {
            Ok(result) => {
                self.pdf_preview.failed_page_probes.remove(&key);
                let mut dirty = current_key.as_ref() == Some(&key);
                if let Some(total_pages) = result.total_pages {
                    let document_key = PdfDocumentKey::from_page_key(&key);
                    self.pdf_preview
                        .document_page_counts
                        .insert(document_key.clone(), total_pages);
                    if current_document.as_ref() == Some(&document_key)
                        && let Some(session) = &mut self.pdf_preview.session
                    {
                        let previous_total = session.total_pages;
                        session.total_pages = Some(total_pages);
                        let clamped_page = session
                            .current_page
                            .clamp(PDF_PAGE_MIN, total_pages.max(PDF_PAGE_MIN));
                        if clamped_page != session.current_page {
                            session.current_page = clamped_page;
                            self.pdf_preview.activation_ready_at = Some(Instant::now());
                            dirty = true;
                        }
                        if previous_total != session.total_pages {
                            dirty = true;
                        }
                    }
                }
                if let (Some(width_pts), Some(height_pts)) = (result.width_pts, result.height_pts) {
                    self.pdf_preview.page_dimensions.insert(
                        key.clone(),
                        PdfPageDimensions {
                            width_pts,
                            height_pts,
                        },
                    );
                    dirty |= current_key.as_ref() == Some(&key);
                }
                self.refresh_pdf_prefetch_window();
                dirty
            }
            Err(_) => {
                self.pdf_preview.failed_page_probes.insert(key);
                false
            }
        }
    }

    pub(super) fn apply_pdf_render_build(&mut self, build: jobs::PdfRenderBuild) -> bool {
        let key = PdfRenderKey {
            path: build.path.clone(),
            size: build.size,
            modified: build.modified,
            page: build.page,
            width_px: build.width_px,
            height_px: build.height_px,
        };
        self.pdf_preview.pending_renders.remove(&key);

        match build.result {
            Ok(Some(path)) => {
                self.pdf_preview.failed_renders.remove(&key);
                let image_dimensions = read_png_dimensions(&path);
                self.remember_rendered_pdf(key.clone(), path, image_dimensions);
                let dirty = self
                    .active_pdf_render_key()
                    .as_ref()
                    .is_some_and(|active| active == &key);
                self.refresh_pdf_prefetch_window();
                dirty
            }
            Ok(None) | Err(_) => {
                self.pdf_preview.failed_renders.insert(key);
                false
            }
        }
    }

    fn clear_pdf_page_status(&mut self) {
        if self.status.starts_with(PDF_PAGE_STATUS_PREFIX) {
            self.status.clear();
        }
    }

    fn refresh_pdf_terminal_window_size(&mut self) {
        self.pdf_preview.terminal_window = if self.pdf_preview.enabled {
            query_terminal_window_size()
        } else {
            None
        };
    }

    fn cached_pdf_total_pages(&self, entry: &Entry) -> Option<usize> {
        self.pdf_preview
            .document_page_counts
            .get(&PdfDocumentKey::from_entry(entry))
            .copied()
    }

    fn pdf_selection_activation_ready(&self) -> bool {
        self.pdf_preview
            .activation_ready_at
            .is_none_or(|ready_at| Instant::now() >= ready_at)
    }

    fn overlay_placement_for_request(
        &self,
        request: &PdfOverlayRequest,
    ) -> Option<FittedPdfPlacement> {
        let window_size = self.cached_pdf_terminal_window()?;
        let page_dimensions = self.cached_pdf_page_dimensions(request)?;
        Some(fit_pdf_page(request.area, window_size, page_dimensions))
    }

    fn cached_pdf_terminal_window(&self) -> Option<TerminalWindowSize> {
        self.pdf_preview.terminal_window
    }

    fn cached_pdf_page_dimensions(&self, request: &PdfOverlayRequest) -> Option<PdfPageDimensions> {
        self.pdf_preview
            .page_dimensions
            .get(&PdfPageKey::from_request(request))
            .copied()
    }

    fn cached_pdf_render_path(&mut self, key: &PdfRenderKey) -> Option<PathBuf> {
        if let Some(path) = self.pdf_preview.rendered_pages.get(key)
            && path.exists()
        {
            return Some(path.clone());
        }

        self.pdf_preview.rendered_pages.remove(key);
        self.pdf_preview.rendered_page_dimensions.remove(key);
        self.pdf_preview.render_order.retain(|queued| queued != key);
        None
    }

    fn cached_render_exists(&self, key: &PdfRenderKey) -> bool {
        self.pdf_preview.rendered_pages.contains_key(key)
    }

    fn remember_rendered_pdf(
        &mut self,
        key: PdfRenderKey,
        path: PathBuf,
        dimensions: Option<RenderedImageDimensions>,
    ) {
        self.pdf_preview.rendered_pages.insert(key.clone(), path);
        if let Some(dimensions) = dimensions {
            self.pdf_preview
                .rendered_page_dimensions
                .insert(key.clone(), dimensions);
        }
        self.pdf_preview
            .render_order
            .retain(|queued| queued != &key);
        self.pdf_preview.render_order.push_back(key);
        while self.pdf_preview.render_order.len() > PDF_RENDER_CACHE_LIMIT {
            if let Some(stale_key) = self.pdf_preview.render_order.pop_front()
                && let Some(stale_path) = self.pdf_preview.rendered_pages.remove(&stale_key)
            {
                self.pdf_preview.rendered_page_dimensions.remove(&stale_key);
                let _ = fs::remove_file(stale_path);
            }
        }
    }

    fn active_pdf_display_target(&self) -> Option<DisplayedPdfPreview> {
        let request = self.active_pdf_overlay_request()?;
        if !self.pdf_selection_activation_ready() {
            return None;
        }
        let requested_placement = self.overlay_placement_for_request(&request)?;
        let placement = self.cached_display_placement_for_request(&request, requested_placement)?;
        Some(DisplayedPdfPreview::from_request(&request, placement))
    }

    fn active_pdf_render_key(&self) -> Option<PdfRenderKey> {
        let request = self.active_pdf_overlay_request()?;
        let placement = self.overlay_placement_for_request(&request)?;
        Some(PdfRenderKey::from_request(&request, placement))
    }

    fn should_keep_displayed_pdf_overlay(&self, displayed: &DisplayedPdfPreview) -> bool {
        self.active_pdf_display_target()
            .as_ref()
            .is_some_and(|target| target == displayed)
    }

    fn clear_pending_pdf_work(&mut self) {
        self.pdf_preview.pending_page_probes.clear();
        self.pdf_preview.pending_renders.clear();
        self.scheduler.clear_pending_pdf_jobs();
    }

    fn resolved_pdf_display_placement(
        &mut self,
        request: &PdfOverlayRequest,
        render_key: &PdfRenderKey,
        fallback: FittedPdfPlacement,
        rendered: &Path,
    ) -> FittedPdfPlacement {
        let Some(window_size) = self.cached_pdf_terminal_window() else {
            return fallback;
        };
        let Some(image_dimensions) = self.cached_rendered_image_dimensions(render_key, rendered)
        else {
            return fallback;
        };

        FittedPdfPlacement {
            image_area: fit_image_area(
                request.area,
                window_size,
                image_dimensions.width_px as f32 / image_dimensions.height_px as f32,
            ),
            ..fallback
        }
    }

    fn cached_rendered_image_dimensions(
        &mut self,
        key: &PdfRenderKey,
        rendered: &Path,
    ) -> Option<RenderedImageDimensions> {
        if let Some(dimensions) = self.pdf_preview.rendered_page_dimensions.get(key).copied() {
            return Some(dimensions);
        }

        let dimensions = read_png_dimensions(rendered)?;
        self.pdf_preview
            .rendered_page_dimensions
            .insert(key.clone(), dimensions);
        Some(dimensions)
    }

    fn cached_display_placement_for_request(
        &self,
        request: &PdfOverlayRequest,
        fallback: FittedPdfPlacement,
    ) -> Option<FittedPdfPlacement> {
        let window_size = self.cached_pdf_terminal_window()?;
        let render_key = PdfRenderKey::from_request(request, fallback);
        let image_dimensions = self
            .pdf_preview
            .rendered_page_dimensions
            .get(&render_key)
            .copied()?;
        Some(FittedPdfPlacement {
            image_area: fit_image_area(
                request.area,
                window_size,
                image_dimensions.width_px as f32 / image_dimensions.height_px as f32,
            ),
            ..fallback
        })
    }

    fn refresh_pdf_prefetch_window(&mut self) {
        let Some(session) = self.pdf_preview.session.as_ref() else {
            self.clear_pending_pdf_work();
            return;
        };
        let path = session.path.clone();
        let size = session.size;
        let modified = session.modified;

        let window_pages = self.pdf_probe_window_pages();
        self.scheduler
            .retain_pdf_probe_pages(&path, size, modified, &window_pages);
        self.pdf_preview.pending_page_probes.retain(|key| {
            key.path == path
                && key.size == size
                && key.modified == modified
                && window_pages.contains(&key.page)
        });

        let render_variants = self.desired_pdf_render_variants();
        self.scheduler
            .retain_pdf_render_variants(&path, size, modified, &render_variants);
        self.pdf_preview.pending_renders.retain(|key| {
            key.path == path
                && key.size == size
                && key.modified == modified
                && render_variants.contains(&(key.page, key.width_px, key.height_px))
        });

        self.queue_current_pdf_probe();
        self.queue_current_pdf_render();
        self.queue_prefetch_pdf_probes();
        if self.current_pdf_render_ready() {
            self.queue_prefetch_pdf_renders();
        }
    }

    fn queue_prefetch_pdf_probes(&mut self) {
        for page in self.pdf_prefetch_probe_pages() {
            self.try_queue_pdf_probe_page(page);
        }
    }

    fn queue_prefetch_pdf_renders(&mut self) {
        for page in self.pdf_prefetch_render_pages() {
            self.try_queue_pdf_render_page(page);
        }
    }

    fn try_queue_pdf_probe_page(&mut self, page: usize) {
        let Some(session) = &self.pdf_preview.session else {
            return;
        };

        self.submit_pdf_probe_key(PdfPageKey {
            path: session.path.clone(),
            size: session.size,
            modified: session.modified,
            page,
        });
    }

    fn try_queue_pdf_render_page(&mut self, page: usize) {
        let Some(key) = self.pdf_render_key_for_page(page) else {
            return;
        };
        self.submit_pdf_render_key(key);
    }

    fn current_pdf_render_ready(&self) -> bool {
        self.active_pdf_render_key()
            .as_ref()
            .is_some_and(|key| self.cached_render_exists(key))
    }

    fn pdf_probe_window_pages(&self) -> Vec<usize> {
        let Some(session) = self.pdf_preview.session.as_ref() else {
            return Vec::new();
        };

        let mut pages = vec![session.current_page];
        let Some(total_pages) = session.total_pages else {
            return pages;
        };

        for distance in 1..=PDF_PREFETCH_DISTANCE {
            let next_page = session.current_page.saturating_add(distance);
            if next_page <= total_pages {
                pages.push(next_page);
            }
            let previous_page = session.current_page.saturating_sub(distance);
            if previous_page >= PDF_PAGE_MIN {
                pages.push(previous_page);
            }
        }
        pages
    }

    fn pdf_prefetch_probe_pages(&self) -> Vec<usize> {
        self.pdf_probe_window_pages()
            .into_iter()
            .filter(|page| {
                self.pdf_preview
                    .session
                    .as_ref()
                    .is_some_and(|session| *page != session.current_page)
            })
            .collect()
    }

    fn pdf_prefetch_render_pages(&self) -> Vec<usize> {
        let Some(session) = self.pdf_preview.session.as_ref() else {
            return Vec::new();
        };
        let Some(total_pages) = session.total_pages else {
            return Vec::new();
        };

        let next_page = session.current_page.saturating_add(1);
        if next_page <= total_pages {
            return vec![next_page];
        }

        let previous_page = session.current_page.saturating_sub(1);
        if previous_page >= PDF_PAGE_MIN {
            vec![previous_page]
        } else {
            Vec::new()
        }
    }

    fn pdf_overlay_request_for_page(&self, page: usize) -> Option<PdfOverlayRequest> {
        if !self.pdf_preview.enabled {
            return None;
        }

        let session = self.pdf_preview.session.as_ref()?;
        let area = self.frame_state.preview_content_area?;
        if area.width == 0 || area.height == 0 {
            return None;
        }

        Some(PdfOverlayRequest {
            path: session.path.clone(),
            size: session.size,
            modified: session.modified,
            page,
            area,
        })
    }

    fn pdf_render_key_for_page(&self, page: usize) -> Option<PdfRenderKey> {
        let request = self.pdf_overlay_request_for_page(page)?;
        let placement = self.overlay_placement_for_request(&request)?;
        Some(PdfRenderKey::from_request(&request, placement))
    }

    fn desired_pdf_render_variants(&self) -> Vec<(usize, u32, u32)> {
        let Some(session) = self.pdf_preview.session.as_ref() else {
            return Vec::new();
        };

        let mut variants = Vec::new();
        if let Some(key) = self.pdf_render_key_for_page(session.current_page) {
            variants.push((key.page, key.width_px, key.height_px));
        }
        if self.current_pdf_render_ready() {
            for page in self.pdf_prefetch_render_pages() {
                if let Some(key) = self.pdf_render_key_for_page(page) {
                    variants.push((key.page, key.width_px, key.height_px));
                }
            }
        }
        variants
    }

    fn queue_current_pdf_probe(&mut self) {
        let Some(session) = &self.pdf_preview.session else {
            return;
        };

        self.submit_pdf_probe_key(PdfPageKey {
            path: session.path.clone(),
            size: session.size,
            modified: session.modified,
            page: session.current_page,
        });
    }

    fn queue_current_pdf_render(&mut self) {
        let Some(key) = self.active_pdf_render_key() else {
            return;
        };
        self.submit_pdf_render_key(key);
    }

    fn submit_pdf_probe_key(&mut self, key: PdfPageKey) {
        if self.pdf_preview.page_dimensions.contains_key(&key)
            || self.pdf_preview.pending_page_probes.contains(&key)
            || self.pdf_preview.failed_page_probes.contains(&key)
        {
            return;
        }

        if self.scheduler.submit_pdf_probe(jobs::PdfProbeRequest {
            path: key.path.clone(),
            size: key.size,
            modified: key.modified,
            page: key.page,
        }) {
            self.pdf_preview.pending_page_probes.insert(key);
        } else {
            self.pdf_preview.failed_page_probes.insert(key);
        }
    }

    fn submit_pdf_render_key(&mut self, key: PdfRenderKey) {
        if self.cached_render_exists(&key)
            || self.pdf_preview.pending_renders.contains(&key)
            || self.pdf_preview.failed_renders.contains(&key)
        {
            return;
        }

        if self.scheduler.submit_pdf_render(jobs::PdfRenderRequest {
            path: key.path.clone(),
            size: key.size,
            modified: key.modified,
            page: key.page,
            width_px: key.width_px,
            height_px: key.height_px,
        }) {
            self.pdf_preview.pending_renders.insert(key);
        } else {
            self.pdf_preview.failed_renders.insert(key);
        }
    }
}

impl PdfRenderKey {
    fn from_request(request: &PdfOverlayRequest, placement: FittedPdfPlacement) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
            width_px: placement.render_width_px,
            height_px: placement.render_height_px,
        }
    }
}

impl PdfPageKey {
    fn from_request(request: &PdfOverlayRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
        }
    }
}

impl DisplayedPdfPreview {
    fn from_request(request: &PdfOverlayRequest, placement: FittedPdfPlacement) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
            area: request.area,
            render_width_px: placement.render_width_px,
            render_height_px: placement.render_height_px,
        }
    }
}

impl PdfDocumentKey {
    fn from_entry(entry: &Entry) -> Self {
        Self {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
        }
    }

    fn from_page_key(key: &PdfPageKey) -> Self {
        Self {
            path: key.path.clone(),
            size: key.size,
            modified: key.modified,
        }
    }

    fn from_session(session: &PdfSession) -> Self {
        Self {
            path: session.path.clone(),
            size: session.size,
            modified: session.modified,
        }
    }
}

fn is_pdf_entry(entry: &Entry) -> bool {
    file_facts::inspect_path(&entry.path, entry.kind)
        .preview
        .document_format
        == Some(DocumentFormat::Pdf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-pdf-preview-{label}-{unique}"))
    }

    fn build_pdf_overlay_test_app(label: &str) -> (App, PathBuf) {
        let root = temp_root(label);
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("app should initialize");
        let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
        app.pdf_preview.enabled = true;
        app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);
        app.pdf_preview.session = Some(PdfSession {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            current_page: 1,
            total_pages: None,
        });
        app.frame_state.preview_content_area = Some(Rect {
            x: 2,
            y: 3,
            width: 48,
            height: 20,
        });
        app.pdf_preview.terminal_window = Some(TerminalWindowSize {
            cells_width,
            cells_height,
            pixels_width: 1920,
            pixels_height: 1080,
        });
        app.pdf_preview.activation_ready_at = Some(Instant::now());
        (app, root)
    }

    #[test]
    fn parse_pdfinfo_page_count_reads_page_field() {
        assert_eq!(
            parse_pdfinfo_page_count("Title: demo\nPages: 18\nProducer: test\n"),
            Some(18)
        );
    }

    #[test]
    fn parse_pdfinfo_page_dimensions_reads_global_and_per_page_sizes() {
        assert_eq!(
            parse_pdfinfo_page_dimensions("Page size: 595.276 x 841.89 pts (A4)\n"),
            Some(PdfPageDimensions {
                width_pts: 595.276,
                height_pts: 841.89,
            })
        );
        assert_eq!(
            parse_pdfinfo_page_dimensions("Page    2 size: 300 x 144 pts\n"),
            Some(PdfPageDimensions {
                width_pts: 300.0,
                height_pts: 144.0,
            })
        );
    }

    #[test]
    fn parse_window_size_reads_pixel_dimensions() {
        assert_eq!(parse_window_size("1575x919\n"), Some((1575, 919)));
    }

    #[test]
    fn read_png_dimensions_reads_ihdr_size() {
        let root = temp_root("png-dimensions");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("page.png");
        let bytes = [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, // signature
            0x00, 0x00, 0x00, 0x0d, // chunk length
            b'I', b'H', b'D', b'R', // chunk type
            0x00, 0x00, 0x02, 0x58, // width 600
            0x00, 0x00, 0x01, 0x2c, // height 300
        ];
        fs::write(&path, bytes).expect("failed to write png header");

        assert_eq!(
            read_png_dimensions(&path),
            Some(RenderedImageDimensions {
                width_px: 600,
                height_px: 300,
            })
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn bucket_render_dimensions_rounds_up_longest_edge_without_distortion() {
        assert_eq!(bucket_render_dimensions((512, 768)), (512, 768));
        assert_eq!(bucket_render_dimensions((530, 742)), (549, 768));
    }

    #[test]
    fn select_terminal_image_backend_prefers_known_kitty_protocol_terminals() {
        assert_eq!(
            select_terminal_image_backend("xterm-kitty", "", false, false, false),
            Some(TerminalImageBackend::KittyProtocol)
        );
        assert_eq!(
            select_terminal_image_backend("xterm-256color", "ghostty", false, false, false),
            Some(TerminalImageBackend::KittyProtocol)
        );
        assert_eq!(
            select_terminal_image_backend("xterm-256color", "WezTerm", false, false, false),
            Some(TerminalImageBackend::KittyProtocol)
        );
        assert_eq!(
            select_terminal_image_backend("screen-256color", "", true, false, false),
            Some(TerminalImageBackend::KittyProtocol)
        );
    }

    #[test]
    fn select_terminal_image_backend_falls_back_to_kitten_detection() {
        assert_eq!(
            select_terminal_image_backend("xterm-256color", "", false, true, true),
            Some(TerminalImageBackend::Kitten)
        );
        assert_eq!(
            select_terminal_image_backend("xterm-256color", "", false, true, false),
            None
        );
    }

    #[test]
    fn fallback_window_size_pixels_uses_reasonable_cell_defaults() {
        assert_eq!(fallback_window_size_pixels(100, 40), (800, 640));
        assert_eq!(fallback_window_size_pixels(0, 0), (8, 16));
    }

    #[test]
    fn build_kitty_display_sequence_positions_png_without_cursor_motion() {
        let path = Path::new("/tmp/demo.pdf-preview.png");
        let area = Rect {
            x: 7,
            y: 4,
            width: 30,
            height: 12,
        };

        let sequence = build_kitty_display_sequence(path, area);

        assert!(sequence.starts_with("\u{1b}[5;8H\u{1b}_G"));
        assert!(sequence.contains("a=T"));
        assert!(sequence.contains("q=2"));
        assert!(sequence.contains("f=100"));
        assert!(sequence.contains("t=f"));
        assert!(sequence.contains("c=30"));
        assert!(sequence.contains("r=12"));
        assert!(sequence.contains("C=1"));
        assert!(sequence.contains(&BASE64_STANDARD.encode(path.as_os_str().as_encoded_bytes())));
        assert!(sequence.ends_with("\u{1b}\\"));
    }

    #[test]
    fn build_kitty_clear_sequence_deletes_visible_images() {
        assert_eq!(build_kitty_clear_sequence(), "\u{1b}_Ga=d,d=A,q=2\u{1b}\\");
    }

    #[test]
    fn fit_pdf_page_preserves_aspect_ratio_for_wide_pages() {
        let placement = fit_pdf_page(
            Rect {
                x: 10,
                y: 4,
                width: 30,
                height: 20,
            },
            TerminalWindowSize {
                cells_width: 100,
                cells_height: 50,
                pixels_width: 1000,
                pixels_height: 1000,
            },
            PdfPageDimensions {
                width_pts: 300.0,
                height_pts: 144.0,
            },
        );

        assert!(placement.image_area.width <= 30);
        assert!(placement.image_area.height <= 20);
        assert_eq!(placement.image_area.height, 7);
        assert_eq!(placement.image_area.y, 10);
        assert!(placement.render_width_px > placement.render_height_px);
    }

    #[test]
    fn fit_image_area_preserves_actual_rendered_png_aspect_ratio() {
        let area = fit_image_area(
            Rect {
                x: 10,
                y: 4,
                width: 30,
                height: 20,
            },
            TerminalWindowSize {
                cells_width: 100,
                cells_height: 50,
                pixels_width: 1000,
                pixels_height: 1000,
            },
            0.25,
        );

        assert_eq!(area.width, 10);
        assert_eq!(area.height, 20);
        assert_eq!(area.x, 20);
        assert_eq!(area.y, 4);
    }

    #[test]
    fn pdf_preview_page_navigation_clamps_to_document_bounds() {
        let mut app = App::new_at(std::env::temp_dir()).expect("app should initialize");
        app.pdf_preview.enabled = true;
        app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);
        app.pdf_preview.session = Some(PdfSession {
            path: PathBuf::from("demo.pdf"),
            size: 1,
            modified: None,
            current_page: 2,
            total_pages: Some(3),
        });

        assert!(app.step_pdf_page(1));
        assert_eq!(
            app.pdf_preview
                .session
                .as_ref()
                .map(|session| session.current_page),
            Some(3)
        );
        assert!(!app.step_pdf_page(1));
        assert_eq!(
            app.pdf_preview
                .session
                .as_ref()
                .map(|session| session.current_page),
            Some(3)
        );
        assert!(app.step_pdf_page(-2));
        assert_eq!(
            app.pdf_preview
                .session
                .as_ref()
                .map(|session| session.current_page),
            Some(1)
        );
        assert!(app.status.is_empty());
    }

    #[test]
    fn present_pdf_overlay_waits_for_selection_activation_before_queueing_probe() {
        let (mut app, root) = build_pdf_overlay_test_app("activation-delay");
        app.pdf_preview.activation_ready_at = Some(Instant::now() + Duration::from_secs(5));

        app.present_pdf_overlay()
            .expect("presenting a delayed PDF overlay should not fail");

        assert!(app.pdf_preview.pending_page_probes.is_empty());
        assert!(!app.scheduler.has_pending_work());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn present_pdf_overlay_queues_current_probe_only_once() {
        let (mut app, root) = build_pdf_overlay_test_app("probe-queue");
        let request = app
            .active_pdf_overlay_request()
            .expect("PDF overlay request should be available");
        let key = PdfPageKey::from_request(&request);

        app.present_pdf_overlay()
            .expect("presenting a PDF overlay should not fail");
        app.present_pdf_overlay()
            .expect("retrying a PDF overlay should not fail");

        assert_eq!(app.pdf_preview.pending_page_probes.len(), 1);
        assert!(app.pdf_preview.pending_page_probes.contains(&key));
        assert!(app.scheduler.has_pending_work());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn process_pdf_preview_timers_releases_selection_activation_once() {
        let (mut app, root) = build_pdf_overlay_test_app("activation-timer");
        app.pdf_preview.activation_ready_at = Some(Instant::now() - Duration::from_millis(1));

        assert!(app.process_pdf_preview_timers());
        assert!(!app.process_pdf_preview_timers());
        assert!(app.pdf_preview.activation_ready_at.is_none());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn sync_pdf_preview_selection_reuses_cached_total_page_count() {
        let root = temp_root("cached-page-count");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let mut app = App::new_at(root.clone()).expect("app should initialize");
        let entry = Entry {
            path: root.join("cached.pdf"),
            name: "cached.pdf".to_string(),
            name_key: "cached.pdf".to_string(),
            kind: EntryKind::File,
            size: 64,
            modified: None,
            readonly: false,
        };
        app.entries = vec![entry.clone()];
        app.selected = 0;
        app.pdf_preview.enabled = true;
        app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);
        app.pdf_preview
            .document_page_counts
            .insert(PdfDocumentKey::from_entry(&entry), 12);

        app.sync_pdf_preview_selection();

        assert_eq!(
            app.pdf_preview
                .session
                .as_ref()
                .and_then(|session| session.total_pages),
            Some(12)
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn sync_pdf_preview_selection_queues_initial_probe_for_current_page() {
        let root = temp_root("selection-probe");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let mut app = App::new_at(root.clone()).expect("app should initialize");
        let entry = Entry {
            path: root.join("queued.pdf"),
            name: "queued.pdf".to_string(),
            name_key: "queued.pdf".to_string(),
            kind: EntryKind::File,
            size: 64,
            modified: None,
            readonly: false,
        };
        app.entries = vec![entry.clone()];
        app.selected = 0;
        app.pdf_preview.enabled = true;
        app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);

        app.sync_pdf_preview_selection();

        assert!(app.scheduler.has_pending_work());
        assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
            path: entry.path,
            size: entry.size,
            modified: entry.modified,
            page: PDF_PAGE_MIN,
        }));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn apply_pdf_probe_build_updates_current_session_and_cached_dimensions() {
        let (mut app, root) = build_pdf_overlay_test_app("probe-apply");
        let session = app
            .pdf_preview
            .session
            .as_mut()
            .expect("PDF session should exist");
        session.current_page = 5;
        let key = PdfPageKey {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page: 5,
        };
        app.pdf_preview.pending_page_probes.insert(key.clone());

        let dirty = app.apply_pdf_probe_build(jobs::PdfProbeBuild {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page: 5,
            result: Ok(PdfProbeResult {
                total_pages: Some(3),
                width_pts: Some(300.0),
                height_pts: Some(144.0),
            }),
        });

        assert!(dirty);
        assert_eq!(
            app.pdf_preview
                .session
                .as_ref()
                .map(|session| session.current_page),
            Some(3)
        );
        assert_eq!(
            app.pdf_preview
                .session
                .as_ref()
                .and_then(|session| session.total_pages),
            Some(3)
        );
        assert_eq!(
            app.pdf_preview.page_dimensions.get(&key),
            Some(&PdfPageDimensions {
                width_pts: 300.0,
                height_pts: 144.0,
            })
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn apply_pdf_probe_build_queues_render_for_current_page() {
        let (mut app, root) = build_pdf_overlay_test_app("probe-render-queue");
        let request = app
            .active_pdf_overlay_request()
            .expect("PDF overlay request should be available");
        let page_key = PdfPageKey::from_request(&request);
        app.pdf_preview.pending_page_probes.insert(page_key);

        let dirty = app.apply_pdf_probe_build(jobs::PdfProbeBuild {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
            result: Ok(PdfProbeResult {
                total_pages: Some(8),
                width_pts: Some(595.0),
                height_pts: Some(842.0),
            }),
        });

        let placement = app
            .overlay_placement_for_request(&request)
            .expect("overlay placement should be available after probe");
        let render_key = PdfRenderKey::from_request(&request, placement);

        assert!(dirty);
        assert!(app.pdf_preview.pending_renders.contains(&render_key));
        assert!(app.scheduler.has_pending_work());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn apply_pdf_probe_build_queues_render_even_before_selection_activation_is_ready() {
        let (mut app, root) = build_pdf_overlay_test_app("probe-render-before-activation");
        app.pdf_preview.activation_ready_at = Some(Instant::now() + Duration::from_secs(5));
        let request = app
            .active_pdf_overlay_request()
            .expect("PDF overlay request should be available");
        let page_key = PdfPageKey::from_request(&request);
        app.pdf_preview.pending_page_probes.insert(page_key);

        let dirty = app.apply_pdf_probe_build(jobs::PdfProbeBuild {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
            result: Ok(PdfProbeResult {
                total_pages: Some(8),
                width_pts: Some(595.0),
                height_pts: Some(842.0),
            }),
        });

        let placement = app
            .overlay_placement_for_request(&request)
            .expect("overlay placement should be available after probe");
        let render_key = PdfRenderKey::from_request(&request, placement);

        assert!(dirty);
        assert!(app.pdf_preview.pending_renders.contains(&render_key));
        assert!(app.scheduler.has_pending_work());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn apply_pdf_probe_build_prefetches_adjacent_page_probes_once_total_is_known() {
        let (mut app, root) = build_pdf_overlay_test_app("probe-prefetch-pages");
        let session = app
            .pdf_preview
            .session
            .as_mut()
            .expect("PDF session should exist");
        session.current_page = 2;

        let request = app
            .active_pdf_overlay_request()
            .expect("PDF overlay request should be available");
        let page_key = PdfPageKey::from_request(&request);
        app.pdf_preview.pending_page_probes.insert(page_key);

        let dirty = app.apply_pdf_probe_build(jobs::PdfProbeBuild {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
            result: Ok(PdfProbeResult {
                total_pages: Some(4),
                width_pts: Some(595.0),
                height_pts: Some(842.0),
            }),
        });

        assert!(dirty);
        assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: 1,
        }));
        assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
            path: request.path,
            size: request.size,
            modified: request.modified,
            page: 3,
        }));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn preview_uses_image_overlay_only_for_current_render_target() {
        let (mut app, root) = build_pdf_overlay_test_app("overlay-match");
        let request = app
            .active_pdf_overlay_request()
            .expect("PDF overlay request should be available");
        let key = PdfPageKey::from_request(&request);
        app.pdf_preview.page_dimensions.insert(
            key,
            PdfPageDimensions {
                width_pts: 595.0,
                height_pts: 842.0,
            },
        );
        let placement = app
            .overlay_placement_for_request(&request)
            .expect("overlay placement should be available");
        let render_key = PdfRenderKey::from_request(&request, placement);
        app.pdf_preview.rendered_page_dimensions.insert(
            render_key,
            RenderedImageDimensions {
                width_px: placement.render_width_px,
                height_px: placement.render_height_px,
            },
        );
        app.pdf_preview.displayed = Some(DisplayedPdfPreview::from_request(&request, placement));

        assert!(app.preview_uses_image_overlay());

        app.pdf_preview
            .session
            .as_mut()
            .expect("PDF session should exist")
            .current_page = 2;

        assert!(!app.preview_uses_image_overlay());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn step_pdf_page_queues_render_immediately_when_dimensions_are_cached() {
        let (mut app, root) = build_pdf_overlay_test_app("page-step-render");
        let next_request = PdfOverlayRequest {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page: 2,
            area: app
                .frame_state
                .preview_content_area
                .expect("preview content area should be set"),
        };
        app.pdf_preview.page_dimensions.insert(
            PdfPageKey::from_request(&next_request),
            PdfPageDimensions {
                width_pts: 612.0,
                height_pts: 792.0,
            },
        );
        app.pdf_preview
            .session
            .as_mut()
            .expect("PDF session should exist")
            .total_pages = Some(3);

        assert!(app.step_pdf_page(1));

        let active_request = app
            .active_pdf_overlay_request()
            .expect("updated PDF overlay request should be available");
        let placement = app
            .overlay_placement_for_request(&active_request)
            .expect("overlay placement should be available");
        let render_key = PdfRenderKey::from_request(&active_request, placement);
        assert!(app.pdf_preview.pending_renders.contains(&render_key));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn step_pdf_page_prunes_stale_prefetch_probe_window() {
        let (mut app, root) = build_pdf_overlay_test_app("page-step-prune");
        let session = app
            .pdf_preview
            .session
            .as_mut()
            .expect("PDF session should exist");
        session.current_page = 2;
        session.total_pages = Some(5);

        for page in [1, 2, 3] {
            app.pdf_preview.pending_page_probes.insert(PdfPageKey {
                path: root.join("demo.pdf"),
                size: 128,
                modified: None,
                page,
            });
        }

        assert!(app.step_pdf_page(1));

        assert!(!app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page: 1,
        }));
        assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page: 2,
        }));
        assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page: 3,
        }));
        assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page: 4,
        }));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn apply_pdf_render_build_prefetches_next_page_when_current_page_is_ready() {
        let (mut app, root) = build_pdf_overlay_test_app("render-prefetch-next");
        let session = app
            .pdf_preview
            .session
            .as_mut()
            .expect("PDF session should exist");
        session.current_page = 2;
        session.total_pages = Some(4);

        for page in [2, 3] {
            let request = PdfOverlayRequest {
                path: root.join("demo.pdf"),
                size: 128,
                modified: None,
                page,
                area: app
                    .frame_state
                    .preview_content_area
                    .expect("preview content area should be set"),
            };
            app.pdf_preview.page_dimensions.insert(
                PdfPageKey::from_request(&request),
                PdfPageDimensions {
                    width_pts: 612.0,
                    height_pts: 792.0,
                },
            );
        }

        let current_request = app
            .pdf_overlay_request_for_page(2)
            .expect("current PDF overlay request should be available");
        let current_key = app
            .pdf_render_key_for_page(2)
            .expect("current PDF render key should be available");
        app.pdf_preview.pending_renders.insert(current_key.clone());

        let rendered_path = root.join("current-page.png");
        fs::write(&rendered_path, b"png").expect("failed to write rendered page placeholder");

        let dirty = app.apply_pdf_render_build(jobs::PdfRenderBuild {
            path: current_request.path.clone(),
            size: current_request.size,
            modified: current_request.modified,
            page: current_request.page,
            width_px: current_key.width_px,
            height_px: current_key.height_px,
            result: Ok(Some(rendered_path)),
        });

        let next_key = app
            .pdf_render_key_for_page(3)
            .expect("next page render key should be available");

        assert!(dirty);
        assert!(app.pdf_preview.pending_renders.contains(&next_key));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn pdf_preview_placeholder_message_tracks_loading_state() {
        let (mut app, root) = build_pdf_overlay_test_app("placeholder");

        assert_eq!(
            app.pdf_preview_placeholder_message().as_deref(),
            Some("Loading PDF page...")
        );

        let request = app
            .active_pdf_overlay_request()
            .expect("PDF overlay request should be available");
        let page_key = PdfPageKey::from_request(&request);
        app.pdf_preview.page_dimensions.insert(
            page_key,
            PdfPageDimensions {
                width_pts: 595.0,
                height_pts: 842.0,
            },
        );
        let placement = app
            .overlay_placement_for_request(&request)
            .expect("overlay placement should be available");
        app.pdf_preview
            .pending_renders
            .insert(PdfRenderKey::from_request(&request, placement));

        assert_eq!(
            app.pdf_preview_placeholder_message().as_deref(),
            Some("Rendering PDF page...")
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn preview_prefers_pdf_surface_falls_back_after_overlay_failure() {
        let (mut app, root) = build_pdf_overlay_test_app("fallback");
        let request = app
            .active_pdf_overlay_request()
            .expect("PDF overlay request should be available");
        let page_key = PdfPageKey::from_request(&request);
        app.pdf_preview.failed_page_probes.insert(page_key);

        assert!(!app.preview_prefers_pdf_surface());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn sync_pdf_preview_selection_clears_stale_pdf_page_status() {
        let mut app = App::new_at(std::env::temp_dir()).expect("app should initialize");
        app.status = "PDF page 3/10".to_string();
        app.pdf_preview.enabled = true;
        app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);

        app.sync_pdf_preview_selection();

        assert!(app.status.is_empty());
    }
}
