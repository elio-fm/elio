use super::geometry::fit_pdf_page;
use super::{
    FittedPdfPlacement, PDF_RENDER_CACHE_LIMIT, PdfOverlayRequest, PdfPageDimensions, PdfPageKey,
    PdfRenderKey,
};
use crate::app::overlays::inline_image::{
    RenderedImageDimensions, fit_image_area, read_png_dimensions,
};
use crate::app::{App, jobs};
use std::{
    fs,
    path::{Path, PathBuf},
};

impl App {
    pub(super) fn ensure_pdf_render(&mut self, key: &PdfRenderKey) -> Option<PathBuf> {
        if let Some(path) = self.cached_pdf_render_path(key) {
            return Some(path);
        }
        if self.preview.pdf.failed_renders.contains(key)
            || self.preview.pdf.pending_renders.contains(key)
        {
            return None;
        }
        if !self.jobs.scheduler.submit_pdf_render(
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
            self.preview.pdf.failed_renders.insert(key.clone());
            return None;
        }
        self.preview.pdf.pending_renders.insert(key.clone());
        None
    }

    pub(super) fn ensure_pdf_page_probe(
        &mut self,
        request: &PdfOverlayRequest,
    ) -> Option<PdfPageDimensions> {
        let key = PdfPageKey::from_request(request);
        if let Some(dimensions) = self.preview.pdf.page_dimensions.get(&key).copied() {
            return Some(dimensions);
        }
        if self.preview.pdf.failed_page_probes.contains(&key)
            || self.preview.pdf.pending_page_probes.contains(&key)
        {
            return None;
        }
        if !self.jobs.scheduler.submit_pdf_probe(
            jobs::PdfProbeRequest {
                path: request.path.clone(),
                size: request.size,
                modified: request.modified,
                page: request.page,
            },
            jobs::PdfJobPriority::Current,
        ) {
            self.preview.pdf.failed_page_probes.insert(key);
            return None;
        }
        self.preview.pdf.pending_page_probes.insert(key);
        None
    }

    pub(super) fn overlay_placement_for_request(
        &self,
        request: &PdfOverlayRequest,
    ) -> Option<FittedPdfPlacement> {
        let window_size = self.cached_terminal_window()?;
        let page_dimensions = self.cached_pdf_page_dimensions(request)?;
        Some(fit_pdf_page(request.area, window_size, page_dimensions))
    }

    fn cached_pdf_page_dimensions(&self, request: &PdfOverlayRequest) -> Option<PdfPageDimensions> {
        self.preview
            .pdf
            .page_dimensions
            .get(&PdfPageKey::from_request(request))
            .copied()
    }

    pub(super) fn cached_pdf_render_path(&mut self, key: &PdfRenderKey) -> Option<PathBuf> {
        if let Some(path) = self.preview.pdf.rendered_pages.get(key)
            && path.exists()
        {
            return Some(path.clone());
        }

        self.preview.pdf.rendered_pages.remove(key);
        self.preview.pdf.rendered_page_dimensions.remove(key);
        self.preview.pdf.render_order.retain(|queued| queued != key);
        None
    }

    pub(super) fn cached_render_exists(&self, key: &PdfRenderKey) -> bool {
        self.preview.pdf.rendered_pages.contains_key(key)
    }

    pub(super) fn remember_rendered_pdf(
        &mut self,
        key: PdfRenderKey,
        path: PathBuf,
        dimensions: Option<RenderedImageDimensions>,
    ) {
        self.preview.pdf.rendered_pages.insert(key.clone(), path);
        if let Some(dimensions) = dimensions {
            self.preview
                .pdf
                .rendered_page_dimensions
                .insert(key.clone(), dimensions);
        }
        self.preview
            .pdf
            .render_order
            .retain(|queued| queued != &key);
        self.preview.pdf.render_order.push_back(key);
        while self.preview.pdf.render_order.len() > PDF_RENDER_CACHE_LIMIT {
            if let Some(stale_key) = self.preview.pdf.render_order.pop_front()
                && let Some(stale_path) = self.preview.pdf.rendered_pages.remove(&stale_key)
            {
                self.preview.pdf.rendered_page_dimensions.remove(&stale_key);
                let _ = fs::remove_file(stale_path);
            }
        }
    }

    pub(super) fn invalidate_rendered_pdf(&mut self, key: &PdfRenderKey) {
        if let Some(path) = self.preview.pdf.rendered_pages.remove(key) {
            let _ = fs::remove_file(path);
        }
        self.preview.pdf.rendered_page_dimensions.remove(key);
        self.preview.pdf.render_order.retain(|queued| queued != key);
        self.preview.pdf.pending_renders.remove(key);
        self.preview.pdf.failed_renders.remove(key);
    }

    pub(super) fn active_pdf_render_key(&self) -> Option<PdfRenderKey> {
        let request = self.active_pdf_overlay_request()?;
        let placement = self.overlay_placement_for_request(&request)?;
        Some(self.pdf_render_key_from_request(&request, placement))
    }

    pub(super) fn resolved_pdf_display_placement(
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
        if let Some(dimensions) = self.preview.pdf.rendered_page_dimensions.get(key).copied() {
            return Some(dimensions);
        }

        let dimensions = read_png_dimensions(rendered)?;
        self.preview
            .pdf
            .rendered_page_dimensions
            .insert(key.clone(), dimensions);
        Some(dimensions)
    }

    pub(super) fn cached_display_placement_for_request(
        &self,
        request: &PdfOverlayRequest,
        fallback: FittedPdfPlacement,
    ) -> Option<FittedPdfPlacement> {
        let window_size = self.cached_terminal_window()?;
        let render_key = self.pdf_render_key_from_request(request, fallback);
        let image_dimensions = self
            .preview
            .pdf
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

    pub(super) fn pdf_page_key_from_request(&self, request: &PdfOverlayRequest) -> PdfPageKey {
        PdfPageKey::from_request(request)
    }

    pub(super) fn pdf_render_key_from_request(
        &self,
        request: &PdfOverlayRequest,
        placement: FittedPdfPlacement,
    ) -> PdfRenderKey {
        PdfRenderKey::from_request(request, placement)
    }
}
