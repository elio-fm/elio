use super::{
    PDF_PAGE_MIN, PDF_PROBE_PREFETCH_AHEAD_DISTANCE, PDF_PROBE_PREFETCH_BEHIND_DISTANCE,
    PDF_RENDER_PREFETCH_AHEAD_DISTANCE, PDF_RENDER_PREFETCH_BEHIND_DISTANCE, PdfOverlayRequest,
    PdfPageKey, PdfRenderKey,
};
use crate::app::{App, jobs};

impl App {
    pub(super) fn refresh_pdf_prefetch_window(&mut self) {
        let Some(session) = self.preview.pdf.session.as_ref() else {
            self.clear_pending_pdf_work();
            return;
        };
        let path = session.path.clone();
        let size = session.size;
        let modified = session.modified;

        let window_pages = self.pdf_probe_window_pages();
        self.jobs
            .scheduler
            .retain_pdf_probe_pages(&path, size, modified, &window_pages);
        self.preview.pdf.pending_page_probes.retain(|key| {
            key.path == path
                && key.size == size
                && key.modified == modified
                && window_pages.contains(&key.page)
        });

        let render_variants = self.desired_pdf_render_variants();
        self.jobs
            .scheduler
            .retain_pdf_render_variants(&path, size, modified, &render_variants);
        self.preview.pdf.pending_renders.retain(|key| {
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
        let Some(session) = &self.preview.pdf.session else {
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
        let Some(session) = self.preview.pdf.session.as_ref() else {
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

    pub(super) fn pdf_prefetch_probe_pages(&self) -> Vec<usize> {
        let Some(session) = self.preview.pdf.session.as_ref() else {
            return Vec::new();
        };
        self.pdf_prefetch_pages(
            session.current_page,
            session.total_pages,
            PDF_PROBE_PREFETCH_AHEAD_DISTANCE,
            PDF_PROBE_PREFETCH_BEHIND_DISTANCE,
        )
    }

    pub(super) fn pdf_prefetch_render_pages(&self) -> Vec<usize> {
        let Some(session) = self.preview.pdf.session.as_ref() else {
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
                self.preview
                    .pdf
                    .page_dimensions
                    .contains_key(&PdfPageKey::from_request(request))
            })
    }

    pub(super) fn pdf_overlay_request_for_page(&self, page: usize) -> Option<PdfOverlayRequest> {
        if !self.terminal_image_overlay_available() {
            return None;
        }

        let session = self.preview.pdf.session.as_ref()?;
        let area = self.input.frame_state.preview_content_area?;
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

    pub(super) fn pdf_render_key_for_page(&self, page: usize) -> Option<PdfRenderKey> {
        let request = self.pdf_overlay_request_for_page(page)?;
        let placement = self.overlay_placement_for_request(&request)?;
        Some(PdfRenderKey::from_request(&request, placement))
    }

    fn desired_pdf_render_variants(&self) -> Vec<(usize, u32, u32)> {
        let Some(session) = self.preview.pdf.session.as_ref() else {
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
        let prefer_backward = self.preview.pdf.last_navigation_direction < 0;
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
        let Some(session) = &self.preview.pdf.session else {
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
        if self.preview.pdf.page_dimensions.contains_key(&key)
            || self.preview.pdf.pending_page_probes.contains(&key)
            || self.preview.pdf.failed_page_probes.contains(&key)
        {
            return;
        }

        if self.jobs.scheduler.submit_pdf_probe(
            jobs::PdfProbeRequest {
                path: key.path.clone(),
                size: key.size,
                modified: key.modified,
                page: key.page,
            },
            priority,
        ) {
            self.preview.pdf.pending_page_probes.insert(key);
        } else {
            self.preview.pdf.failed_page_probes.insert(key);
        }
    }

    fn submit_pdf_render_key(&mut self, key: PdfRenderKey, priority: jobs::PdfJobPriority) {
        if self.cached_render_exists(&key)
            || self.preview.pdf.pending_renders.contains(&key)
            || self.preview.pdf.failed_renders.contains(&key)
        {
            return;
        }

        if self.jobs.scheduler.submit_pdf_render(
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
            self.preview.pdf.pending_renders.insert(key);
        } else {
            self.preview.pdf.failed_renders.insert(key);
        }
    }
}
