use super::{
    PDF_PAGE_MIN, PDF_PAGE_STATUS_PREFIX, PDF_SELECTION_ACTIVATION_DELAY, PdfDocumentKey,
    PdfOverlayRequest, PdfPageDimensions, PdfPageKey, PdfRenderKey, PdfSession,
};
use crate::app::overlays::inline_image::read_png_dimensions;
use crate::app::{App, Entry, jobs};
use crate::file_info::{self, DocumentFormat};
use std::time::{Duration, Instant};

impl App {
    pub(in crate::app) fn handle_pdf_overlay_resize(&mut self) {
        if self.preview.pdf.session.is_some() {
            self.preview.pdf.activation_ready_at = None;
            self.refresh_pdf_prefetch_window();
        }
    }

    pub(crate) fn process_pdf_preview_timers(&mut self) -> bool {
        let Some(ready_at) = self.preview.pdf.activation_ready_at else {
            return false;
        };
        if Instant::now() < ready_at {
            return false;
        }

        self.preview.pdf.activation_ready_at = None;
        self.preview.pdf.session.is_some()
    }

    pub(crate) fn pending_pdf_preview_timer(&self) -> Option<Duration> {
        self.preview
            .pdf
            .activation_ready_at
            .map(|ready_at| ready_at.saturating_duration_since(Instant::now()))
    }

    pub(in crate::app) fn pdf_preview_header_detail(&self) -> Option<String> {
        let session = self.preview.pdf.session.as_ref()?;
        if !self.terminal_image_overlay_available() {
            return None;
        }

        let page_label = match session.total_pages {
            Some(total_pages) => format!("Page {}/{}", session.current_page, total_pages),
            None => format!("Page {}", session.current_page),
        };
        Some(page_label)
    }

    pub(in crate::app) fn step_pdf_page(&mut self, delta: isize) -> bool {
        let Some(session) = &mut self.preview.pdf.session else {
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
            self.preview.pdf.last_navigation_direction = delta.signum();
            self.preview.pdf.activation_ready_at = None;
            self.refresh_pdf_prefetch_window();
        }
        changed
    }

    pub(in crate::app) fn sync_pdf_preview_selection(&mut self) {
        self.clear_failed_static_image_state_if_needed();
        if !self.terminal_image_overlay_available() || !self.preview.pdf.pdf_tools_available {
            self.preview.pdf.session = None;
            self.preview.pdf.activation_ready_at = None;
            self.clear_pending_pdf_work();
            self.clear_pdf_page_status();
            return;
        }

        let Some(entry) = self.selected_entry() else {
            self.preview.pdf.session = None;
            self.preview.pdf.activation_ready_at = None;
            self.clear_pending_pdf_work();
            self.clear_pdf_page_status();
            return;
        };
        if !is_pdf_entry(entry) {
            self.preview.pdf.session = None;
            self.preview.pdf.activation_ready_at = None;
            self.clear_pending_pdf_work();
            self.clear_pdf_page_status();
            return;
        }

        let should_keep_session = self.preview.pdf.session.as_ref().is_some_and(|session| {
            session.path == entry.path
                && session.size == entry.size
                && session.modified == entry.modified
        });
        if should_keep_session {
            return;
        }

        self.preview.pdf.session = Some(PdfSession {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
            current_page: PDF_PAGE_MIN,
            total_pages: self.cached_pdf_total_pages(entry),
        });
        self.preview.pdf.last_navigation_direction = 0;
        self.preview.pdf.activation_ready_at =
            Some(Instant::now() + PDF_SELECTION_ACTIVATION_DELAY);
        self.refresh_pdf_prefetch_window();
        self.clear_pdf_page_status();
    }

    pub(super) fn active_pdf_overlay_request(&self) -> Option<PdfOverlayRequest> {
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
            page: session.current_page,
            area,
        })
    }

    pub(in crate::app) fn apply_pdf_probe_build(&mut self, build: jobs::PdfProbeBuild) -> bool {
        let key = PdfPageKey {
            path: build.path.clone(),
            size: build.size,
            modified: build.modified,
            page: build.page,
        };
        self.preview.pdf.pending_page_probes.remove(&key);

        let current_request = self.active_pdf_overlay_request();
        let current_key = current_request.as_ref().map(PdfPageKey::from_request);
        let is_current_key = current_key.as_ref() == Some(&key);
        let current_document = self
            .preview
            .pdf
            .session
            .as_ref()
            .map(PdfDocumentKey::from_session);

        match build.result {
            Ok(result) => {
                self.preview.pdf.failed_page_probes.remove(&key);
                let mut dirty = current_key.as_ref() == Some(&key);
                if let Some(total_pages) = result.total_pages {
                    let document_key = PdfDocumentKey::from_page_key(&key);
                    self.preview
                        .pdf
                        .document_page_counts
                        .insert(document_key.clone(), total_pages);
                    if current_document.as_ref() == Some(&document_key)
                        && let Some(session) = &mut self.preview.pdf.session
                    {
                        let previous_total = session.total_pages;
                        session.total_pages = Some(total_pages);
                        let clamped_page = session
                            .current_page
                            .clamp(PDF_PAGE_MIN, total_pages.max(PDF_PAGE_MIN));
                        if clamped_page != session.current_page {
                            session.current_page = clamped_page;
                            self.preview.pdf.activation_ready_at = Some(Instant::now());
                            dirty = true;
                        }
                        if previous_total != session.total_pages {
                            dirty = true;
                        }
                    }
                }
                if let (Some(width_pts), Some(height_pts)) = (result.width_pts, result.height_pts) {
                    self.preview.pdf.page_dimensions.insert(
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
                self.preview.pdf.failed_page_probes.insert(key);
                if is_current_key {
                    self.refresh_preview();
                    true
                } else {
                    false
                }
            }
        }
    }

    pub(in crate::app) fn apply_pdf_render_build(&mut self, build: jobs::PdfRenderBuild) -> bool {
        let key = PdfRenderKey {
            path: build.path.clone(),
            size: build.size,
            modified: build.modified,
            page: build.page,
            width_px: build.width_px,
            height_px: build.height_px,
        };
        self.preview.pdf.pending_renders.remove(&key);
        let is_current_key = self
            .active_pdf_render_key()
            .as_ref()
            .is_some_and(|active| active == &key);

        match build.result {
            Ok(Some(path)) => {
                self.preview.pdf.failed_renders.remove(&key);
                let image_dimensions = read_png_dimensions(&path);
                self.remember_rendered_pdf(key.clone(), path, image_dimensions);
                if let (Some(sixel_dcs), Some(sixel_dcs_key)) =
                    (build.sixel_dcs, build.sixel_dcs_key)
                {
                    self.remember_sixel_dcs(sixel_dcs_key, sixel_dcs);
                }
                let dirty = is_current_key;
                self.refresh_pdf_prefetch_window();
                dirty
            }
            Ok(None) | Err(_) => {
                self.preview.pdf.failed_renders.insert(key);
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

    fn cached_pdf_total_pages(&self, entry: &Entry) -> Option<usize> {
        self.preview
            .pdf
            .document_page_counts
            .get(&PdfDocumentKey::from_entry(entry))
            .copied()
    }

    pub(super) fn pdf_selection_activation_ready(&self) -> bool {
        self.preview
            .pdf
            .activation_ready_at
            .is_none_or(|ready_at| Instant::now() >= ready_at)
    }

    pub(in crate::app) fn should_defer_pdf_document_preview(&self, entry: &Entry) -> bool {
        is_pdf_entry(entry) && self.preview_prefers_pdf_surface()
    }

    pub(super) fn clear_pending_pdf_work(&mut self) {
        self.preview.pdf.pending_page_probes.clear();
        self.preview.pdf.pending_renders.clear();
        self.jobs.scheduler.clear_pending_pdf_jobs();
    }
}

fn is_pdf_entry(entry: &Entry) -> bool {
    file_info::inspect_path_cached(&entry.path, entry.kind, entry.size, entry.modified)
        .preview
        .document_format
        == Some(DocumentFormat::Pdf)
}
