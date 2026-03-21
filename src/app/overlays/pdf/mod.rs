mod geometry;
mod pipeline;
mod session;
mod types;

use self::geometry::fit_pdf_page;
pub(crate) use self::pipeline::{probe_pdf_page, render_pdf_page_to_cache};
pub(in crate::app::overlays::pdf) use self::types::{
    DisplayedPdfPreview, FittedPdfPlacement, PdfDocumentKey, PdfOverlayRequest, PdfPageDimensions,
    PdfPageKey, PdfRenderKey, PdfSession,
};
pub(in crate::app) use self::types::{PdfPreviewState, PdfProbeResult};
#[cfg(test)]
use self::{
    geometry::bucket_render_dimensions,
    pipeline::{parse_pdfinfo_page_count, parse_pdfinfo_page_dimensions},
};
use super::super::*;
use super::inline_image::{
    ImageProtocol, OverlayPresentState, RenderedImageDimensions, fit_image_area,
    place_terminal_image, preview_log, read_png_dimensions,
};
#[cfg(test)]
use super::inline_image::{
    TerminalIdentity, build_kitty_clear_sequence, build_kitty_placeholder_sequence,
    build_kitty_upload_sequence, fallback_window_size_pixels, parse_window_size,
    select_image_protocol,
};
use anyhow::{Context, Result};
#[cfg(test)]
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use ratatui::layout::Rect;
#[cfg(test)]
use std::time::Instant;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const PDF_RENDER_CACHE_LIMIT: usize = 12;
const PDF_RENDER_BUCKET_PX: u32 = 64;
const PDF_RENDER_MIN_DIMENSION_PX: u32 = 96;
const PDF_PAGE_MIN: usize = 1;
const PDF_PAGE_STATUS_PREFIX: &str = "PDF page ";
const PDF_PROBE_PREFETCH_AHEAD_DISTANCE: usize = 2;
const PDF_PROBE_PREFETCH_BEHIND_DISTANCE: usize = 1;
const PDF_RENDER_PREFETCH_AHEAD_DISTANCE: usize = 2;
const PDF_RENDER_PREFETCH_BEHIND_DISTANCE: usize = 1;
const PDF_SELECTION_ACTIVATION_DELAY: Duration = Duration::from_millis(16);

#[cfg(test)]
mod tests;

impl App {
    pub(in crate::app) fn present_pdf_overlay(
        &mut self,
        protocol: ImageProtocol,
        excluded: &[Rect],
        out: &mut Vec<u8>,
    ) -> Result<OverlayPresentState> {
        let Some(request) = self.active_pdf_overlay_request() else {
            preview_log("present_pdf_overlay: no request");
            return Ok(OverlayPresentState::NotRequested);
        };
        preview_log(format_args!(
            "present_pdf_overlay: path={:?} page={}",
            request.path, request.page
        ));

        if !self.pdf_selection_activation_ready() {
            preview_log("present_pdf_overlay: activation not ready → Waiting");
            return Ok(OverlayPresentState::Waiting);
        }

        let Some(requested_placement) = self.overlay_placement_for_request(&request) else {
            preview_log("present_pdf_overlay: no placement yet → probe + Waiting");
            let _ = self.ensure_pdf_page_probe(&request);
            return Ok(OverlayPresentState::Waiting);
        };
        let render_key = self.pdf_render_key_from_request(&request, requested_placement);
        let Some(rendered) = self.ensure_pdf_render(&render_key) else {
            preview_log("present_pdf_overlay: render not ready → Waiting");
            return Ok(OverlayPresentState::Waiting);
        };
        let placement = self.resolved_pdf_display_placement(
            &request,
            &render_key,
            requested_placement,
            &rendered,
        );
        preview_log(format_args!(
            "present_pdf_overlay: placement={:?}",
            placement.image_area
        ));
        let displayed = DisplayedPdfPreview::from_request(&request, placement);
        let image_changed = self.pdf_preview.displayed.as_ref() != Some(&displayed);
        let excluded_changed = excluded != self.pdf_preview.displayed_excluded.as_slice();
        if !image_changed && !excluded_changed {
            preview_log("present_pdf_overlay: already displayed → Displayed");
            return Ok(OverlayPresentState::Displayed);
        }
        if image_changed {
            out.extend(self.clear_preview_overlay()?);
        }
        let bytes = place_terminal_image(protocol, &rendered, placement.image_area, excluded, None)
            .context("failed to display PDF page")?;
        preview_log(format_args!(
            "present_pdf_overlay: placed {} bytes via {protocol:?}",
            bytes.len()
        ));
        out.extend(bytes);
        self.pdf_preview.displayed = Some(displayed);
        self.pdf_preview.displayed_excluded = excluded.to_vec();
        Ok(OverlayPresentState::Displayed)
    }

    pub(crate) fn preview_prefers_pdf_surface(&self) -> bool {
        if !self.terminal_image_overlay_available()
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

        let page_key = self.pdf_page_key_from_request(&request);
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
        let render_key = self.pdf_render_key_from_request(&request, placement);
        if self.pdf_preview.failed_renders.contains(&render_key) {
            return false;
        }
        self.pdf_preview.pending_renders.contains(&render_key)
            || self.cached_render_exists(&render_key)
    }

    pub(crate) fn preview_overlay_placeholder_message(&self) -> Option<String> {
        if self.preview_prefers_static_image_surface() && !self.preview_uses_image_overlay() {
            return self.static_image_overlay_placeholder_message();
        }

        if !self.preview_prefers_pdf_surface() || self.preview_uses_image_overlay() {
            return None;
        }

        let request = self.active_pdf_overlay_request()?;
        let page_key = self.pdf_page_key_from_request(&request);
        if self.pdf_preview.failed_page_probes.contains(&page_key) {
            return Some("PDF preview unavailable".to_string());
        }
        if !self.pdf_selection_activation_ready()
            || !self.pdf_preview.page_dimensions.contains_key(&page_key)
            || self.pdf_preview.pending_page_probes.contains(&page_key)
        {
            return None;
        }

        let placement = self.overlay_placement_for_request(&request)?;
        let render_key = self.pdf_render_key_from_request(&request, placement);
        if self.pdf_preview.failed_renders.contains(&render_key) {
            return Some("PDF preview unavailable".to_string());
        }
        if self.cached_render_exists(&render_key) {
            return None;
        }
        None
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

    fn overlay_placement_for_request(
        &self,
        request: &PdfOverlayRequest,
    ) -> Option<FittedPdfPlacement> {
        let window_size = self.cached_terminal_window()?;
        let page_dimensions = self.cached_pdf_page_dimensions(request)?;
        Some(fit_pdf_page(request.area, window_size, page_dimensions))
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

    pub(in crate::app) fn pdf_overlay_displayed(&self) -> bool {
        self.pdf_preview.displayed.is_some()
    }

    /// The content area of the currently displayed PDF overlay, if any.
    pub(in crate::app) fn displayed_pdf_overlay_area(&self) -> Option<Rect> {
        self.pdf_preview.displayed.as_ref().map(|d| d.area)
    }

    pub(in crate::app) fn clear_displayed_pdf_overlay(&mut self) {
        self.pdf_preview.displayed = None;
    }

    pub(in crate::app) fn displayed_pdf_overlay_matches_active(&self) -> bool {
        self.active_pdf_display_target()
            .as_ref()
            .zip(self.pdf_preview.displayed.as_ref())
            .is_some_and(|(active, displayed)| active == displayed)
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
        Some(self.pdf_render_key_from_request(&request, placement))
    }

    fn resolved_pdf_display_placement(
        &mut self,
        request: &PdfOverlayRequest,
        render_key: &PdfRenderKey,
        fallback: FittedPdfPlacement,
        rendered: &Path,
    ) -> FittedPdfPlacement {
        let Some(window_size) = self.cached_terminal_window() else {
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
        let window_size = self.cached_terminal_window()?;
        let render_key = self.pdf_render_key_from_request(request, fallback);
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
        if self.current_pdf_probe_ready() {
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

    fn pdf_probe_window_pages(&self) -> Vec<usize> {
        let Some(session) = self.pdf_preview.session.as_ref() else {
            return Vec::new();
        };

        let mut pages = vec![session.current_page];
        pages.extend(self.pdf_prefetch_pages(
            session.current_page,
            session.total_pages,
            PDF_PROBE_PREFETCH_AHEAD_DISTANCE,
            PDF_PROBE_PREFETCH_BEHIND_DISTANCE,
        ));
        pages
    }

    fn pdf_prefetch_probe_pages(&self) -> Vec<usize> {
        let Some(session) = self.pdf_preview.session.as_ref() else {
            return Vec::new();
        };
        self.pdf_prefetch_pages(
            session.current_page,
            session.total_pages,
            PDF_PROBE_PREFETCH_AHEAD_DISTANCE,
            PDF_PROBE_PREFETCH_BEHIND_DISTANCE,
        )
    }

    fn pdf_prefetch_render_pages(&self) -> Vec<usize> {
        let Some(session) = self.pdf_preview.session.as_ref() else {
            return Vec::new();
        };
        self.pdf_prefetch_pages(
            session.current_page,
            session.total_pages,
            PDF_RENDER_PREFETCH_AHEAD_DISTANCE,
            PDF_RENDER_PREFETCH_BEHIND_DISTANCE,
        )
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

    fn pdf_overlay_request_for_page(&self, page: usize) -> Option<PdfOverlayRequest> {
        if !self.terminal_image_overlay_available() {
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
        if self.current_pdf_probe_ready() {
            for page in self.pdf_prefetch_render_pages() {
                if let Some(key) = self.pdf_render_key_for_page(page) {
                    variants.push((key.page, key.width_px, key.height_px));
                }
            }
        }
        variants
    }

    fn pdf_prefetch_pages(
        &self,
        current_page: usize,
        total_pages: Option<usize>,
        ahead_distance: usize,
        behind_distance: usize,
    ) -> Vec<usize> {
        let Some(total_pages) = total_pages else {
            return Vec::new();
        };
        let prefer_backward = self.pdf_preview.last_navigation_direction < 0;
        let mut pages = Vec::new();

        if prefer_backward {
            for distance in 1..=ahead_distance {
                let previous_page = current_page.saturating_sub(distance);
                if previous_page >= PDF_PAGE_MIN {
                    pages.push(previous_page);
                }
            }
            for distance in 1..=behind_distance {
                let next_page = current_page.saturating_add(distance);
                if next_page <= total_pages {
                    pages.push(next_page);
                }
            }
        } else {
            for distance in 1..=ahead_distance {
                let next_page = current_page.saturating_add(distance);
                if next_page <= total_pages {
                    pages.push(next_page);
                }
            }
            for distance in 1..=behind_distance {
                let previous_page = current_page.saturating_sub(distance);
                if previous_page >= PDF_PAGE_MIN {
                    pages.push(previous_page);
                }
            }
        }

        pages
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

    fn pdf_page_key_from_request(&self, request: &PdfOverlayRequest) -> PdfPageKey {
        PdfPageKey::from_request(request)
    }

    fn pdf_render_key_from_request(
        &self,
        request: &PdfOverlayRequest,
        placement: FittedPdfPlacement,
    ) -> PdfRenderKey {
        PdfRenderKey::from_request(request, placement)
    }
}
