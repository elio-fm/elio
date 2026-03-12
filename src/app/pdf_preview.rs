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
        clear_pdf_images, detect_terminal_pdf_preview_backend, pdf_preview_tools_available,
        place_pdf_image, query_terminal_window_size, read_png_dimensions,
    },
    geometry::{fit_image_area, fit_pdf_page},
};
use super::image_preview::{
    StaticImageKey, StaticImageOverlayPreparation, StaticImageOverlayRequest,
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
const PDF_SELECTION_ACTIVATION_DELAY: Duration = Duration::from_millis(35);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalImageBackend {
    KittyProtocol,
    Kitten,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PdfPreviewState {
    enabled: bool,
    backend: Option<TerminalImageBackend>,
    pdf_tools_available: bool,
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
    displayed: Option<DisplayedOverlay>,
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
pub(super) struct RenderedImageDimensions {
    pub(super) width_px: u32,
    pub(super) height_px: u32,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct DisplayedStaticImagePreview {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    area: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DisplayedOverlay {
    Pdf(DisplayedPdfPreview),
    StaticImage(DisplayedStaticImagePreview),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TerminalWindowSize {
    pub(super) cells_width: u16,
    pub(super) cells_height: u16,
    pub(super) pixels_width: u32,
    pub(super) pixels_height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PdfOverlayRequest {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    page: usize,
    area: Rect,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
    pub(super) fn terminal_image_overlay_available(&self) -> bool {
        self.pdf_preview.enabled && self.pdf_preview.backend.is_some()
    }

    pub(crate) fn enable_terminal_pdf_previews(&mut self) {
        self.pdf_preview.backend = detect_terminal_pdf_preview_backend();
        self.pdf_preview.enabled = self.pdf_preview.backend.is_some();
        self.pdf_preview.pdf_tools_available = pdf_preview_tools_available();
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
        if self.browser_wheel_burst_active() {
            return Ok(());
        }

        let Some(backend) = self.pdf_preview.backend else {
            self.clear_pdf_overlay()?;
            return Ok(());
        };

        if self
            .pdf_preview
            .displayed
            .as_ref()
            .is_some_and(|displayed| !self.should_keep_displayed_overlay(displayed))
        {
            self.clear_pdf_overlay()?;
        }

        if let Some(request) = self.active_static_image_overlay_request() {
            if !self.image_selection_activation_ready() {
                return Ok(());
            }
            let prepared = match self.prepare_static_image_for_overlay(&request) {
                StaticImageOverlayPreparation::Ready(prepared) => prepared,
                StaticImageOverlayPreparation::Pending => return Ok(()),
                StaticImageOverlayPreparation::Failed => {
                    self.mark_static_image_failed(&request);
                    self.refresh_preview();
                    return Ok(());
                }
            };
            let Some(window_size) = self.cached_pdf_terminal_window() else {
                self.mark_static_image_failed(&request);
                self.refresh_preview();
                return Ok(());
            };
            let placement = fit_image_area(
                request.area,
                window_size,
                prepared.dimensions.width_px as f32 / prepared.dimensions.height_px as f32,
            );
            let displayed = DisplayedOverlay::StaticImage(
                DisplayedStaticImagePreview::from_request(&request, placement),
            );
            if self.pdf_preview.displayed.as_ref() == Some(&displayed) {
                return Ok(());
            }

            if self.pdf_preview.displayed.is_some() {
                clear_pdf_images(backend).context("failed to clear previous preview image")?;
                self.pdf_preview.displayed = None;
            }
            if place_pdf_image(backend, &prepared.display_path, placement).is_err() {
                self.mark_static_image_failed(&request);
                self.refresh_preview();
                return Ok(());
            }
            self.pdf_preview.displayed = Some(displayed);
            return Ok(());
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
        let displayed =
            DisplayedOverlay::Pdf(DisplayedPdfPreview::from_request(&request, placement));
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
        self.active_display_target()
            .as_ref()
            .zip(self.pdf_preview.displayed.as_ref())
            .is_some_and(|(active, displayed)| active == displayed)
    }

    pub(crate) fn preview_prefers_image_surface(&self) -> bool {
        self.preview_prefers_static_image_surface() || self.preview_prefers_pdf_surface()
    }

    pub(crate) fn preview_prefers_pdf_surface(&self) -> bool {
        if !self.pdf_preview.enabled
            || self.pdf_preview.backend.is_none()
            || !self.pdf_preview.pdf_tools_available
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

    pub(crate) fn preview_overlay_placeholder_message(&self) -> Option<String> {
        if self.preview_prefers_static_image_surface() && !self.preview_uses_image_overlay() {
            return Some("Preparing image preview...".to_string());
        }

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
        self.clear_failed_static_image_state_if_needed();
        if !self.pdf_preview.enabled || !self.pdf_preview.pdf_tools_available {
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
        if !self.scheduler.submit_pdf_render(
            jobs::PdfRenderRequest {
                path: key.path.clone(),
                size: key.size,
                modified: key.modified,
                page: key.page,
                width_px: key.width_px,
                height_px: key.height_px,
            },
            jobs::PdfJobPriority::Current,
        ) {
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
        if !self.scheduler.submit_pdf_probe(
            jobs::PdfProbeRequest {
                path: request.path.clone(),
                size: request.size,
                modified: request.modified,
                page: request.page,
            },
            jobs::PdfJobPriority::Current,
        ) {
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
        let is_current_key = current_key.as_ref() == Some(&key);
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
                if is_current_key {
                    self.refresh_preview();
                    true
                } else {
                    false
                }
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
        let is_current_key = self
            .active_pdf_render_key()
            .as_ref()
            .is_some_and(|active| active == &key);

        match build.result {
            Ok(Some(path)) => {
                self.pdf_preview.failed_renders.remove(&key);
                let image_dimensions = read_png_dimensions(&path);
                self.remember_rendered_pdf(key.clone(), path, image_dimensions);
                let dirty = is_current_key;
                self.refresh_pdf_prefetch_window();
                dirty
            }
            Ok(None) | Err(_) => {
                self.pdf_preview.failed_renders.insert(key);
                if is_current_key {
                    self.refresh_preview();
                    true
                } else {
                    false
                }
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

    pub(super) fn should_defer_pdf_document_preview(&self, entry: &Entry) -> bool {
        is_pdf_entry(entry) && self.preview_prefers_pdf_surface()
    }

    fn overlay_placement_for_request(
        &self,
        request: &PdfOverlayRequest,
    ) -> Option<FittedPdfPlacement> {
        let window_size = self.cached_pdf_terminal_window()?;
        let page_dimensions = self.cached_pdf_page_dimensions(request)?;
        Some(fit_pdf_page(request.area, window_size, page_dimensions))
    }

    pub(super) fn cached_pdf_terminal_window(&self) -> Option<TerminalWindowSize> {
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

    fn active_display_target(&self) -> Option<DisplayedOverlay> {
        self.active_static_image_display_target()
            .or_else(|| self.active_pdf_display_target())
    }

    fn active_static_image_display_target(&self) -> Option<DisplayedOverlay> {
        let request = self.active_static_image_overlay_request()?;
        let window_size = self.cached_pdf_terminal_window()?;
        let image_dimensions = self
            .image_preview
            .dimensions
            .get(&StaticImageKey::from_request(&request))
            .copied()?;
        Some(DisplayedOverlay::StaticImage(
            DisplayedStaticImagePreview::from_request(
                &request,
                fit_image_area(
                    request.area,
                    window_size,
                    image_dimensions.width_px as f32 / image_dimensions.height_px as f32,
                ),
            ),
        ))
    }

    fn active_pdf_display_target(&self) -> Option<DisplayedOverlay> {
        let request = self.active_pdf_overlay_request()?;
        if !self.pdf_selection_activation_ready() {
            return None;
        }
        let requested_placement = self.overlay_placement_for_request(&request)?;
        let placement = self.cached_display_placement_for_request(&request, requested_placement)?;
        Some(DisplayedOverlay::Pdf(DisplayedPdfPreview::from_request(
            &request, placement,
        )))
    }

    fn active_pdf_render_key(&self) -> Option<PdfRenderKey> {
        let request = self.active_pdf_overlay_request()?;
        let placement = self.overlay_placement_for_request(&request)?;
        Some(PdfRenderKey::from_request(&request, placement))
    }

    fn should_keep_displayed_overlay(&self, displayed: &DisplayedOverlay) -> bool {
        self.active_display_target()
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
        if self.current_pdf_probe_ready() {
            self.queue_prefetch_pdf_probes();
        }
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

        self.submit_pdf_probe_key(
            PdfPageKey {
                path: session.path.clone(),
                size: session.size,
                modified: session.modified,
                page,
            },
            jobs::PdfJobPriority::Prefetch,
        );
    }

    fn try_queue_pdf_render_page(&mut self, page: usize) {
        let Some(key) = self.pdf_render_key_for_page(page) else {
            return;
        };
        self.submit_pdf_render_key(key, jobs::PdfJobPriority::Prefetch);
    }

    fn current_pdf_render_ready(&self) -> bool {
        self.active_pdf_render_key()
            .as_ref()
            .is_some_and(|key| self.cached_render_exists(key))
    }

    fn current_pdf_probe_ready(&self) -> bool {
        self.active_pdf_overlay_request()
            .as_ref()
            .is_some_and(|request| {
                self.pdf_preview
                    .page_dimensions
                    .contains_key(&PdfPageKey::from_request(request))
            })
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

        self.submit_pdf_probe_key(
            PdfPageKey {
                path: session.path.clone(),
                size: session.size,
                modified: session.modified,
                page: session.current_page,
            },
            jobs::PdfJobPriority::Current,
        );
    }

    fn queue_current_pdf_render(&mut self) {
        let Some(key) = self.active_pdf_render_key() else {
            return;
        };
        self.submit_pdf_render_key(key, jobs::PdfJobPriority::Current);
    }

    fn submit_pdf_probe_key(&mut self, key: PdfPageKey, priority: jobs::PdfJobPriority) {
        if self.pdf_preview.page_dimensions.contains_key(&key)
            || self.pdf_preview.pending_page_probes.contains(&key)
            || self.pdf_preview.failed_page_probes.contains(&key)
        {
            return;
        }

        if self.scheduler.submit_pdf_probe(
            jobs::PdfProbeRequest {
                path: key.path.clone(),
                size: key.size,
                modified: key.modified,
                page: key.page,
            },
            priority,
        ) {
            self.pdf_preview.pending_page_probes.insert(key);
        } else {
            self.pdf_preview.failed_page_probes.insert(key);
        }
    }

    fn submit_pdf_render_key(&mut self, key: PdfRenderKey, priority: jobs::PdfJobPriority) {
        if self.cached_render_exists(&key)
            || self.pdf_preview.pending_renders.contains(&key)
            || self.pdf_preview.failed_renders.contains(&key)
        {
            return;
        }

        if self.scheduler.submit_pdf_render(
            jobs::PdfRenderRequest {
                path: key.path.clone(),
                size: key.size,
                modified: key.modified,
                page: key.page,
                width_px: key.width_px,
                height_px: key.height_px,
            },
            priority,
        ) {
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
mod tests;
