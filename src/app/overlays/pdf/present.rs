use super::DisplayedPdfPreview;
use crate::app::App;
use crate::app::overlays::images::SixelDcsKey;
use crate::app::overlays::inline_image::{
    ImageProtocol, OverlayPresentState, place_sixel_from_dcs, place_terminal_image, preview_log,
};
use anyhow::{Context, Result};
use ratatui::layout::Rect;

impl App {
    pub(in crate::app) fn present_pdf_overlay(
        &mut self,
        protocol: ImageProtocol,
        excluded: &[Rect],
        force_repaint: bool,
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
        let image_changed = self.preview.pdf.displayed.as_ref() != Some(&displayed);
        let excluded_changed = excluded != self.preview.pdf.displayed_excluded.as_slice();
        let needs_repaint = force_repaint && protocol.is_raster();
        if !image_changed && !excluded_changed && !needs_repaint {
            preview_log("present_pdf_overlay: already displayed → Displayed");
            return Ok(OverlayPresentState::Displayed);
        }
        if image_changed {
            out.extend(self.clear_preview_overlay()?);
        }
        let bytes = match protocol {
            ImageProtocol::Sixel => {
                let Some(window_size) = self.cached_terminal_window() else {
                    return Ok(OverlayPresentState::Waiting);
                };
                let dcs_key = SixelDcsKey::new(&rendered, placement.image_area, window_size);
                let Some(dcs) = self.cached_sixel_dcs(&dcs_key) else {
                    preview_log("present_pdf_overlay: sixel dcs not ready → Waiting");
                    let _ = self.ensure_pdf_render(&render_key);
                    return Ok(OverlayPresentState::Waiting);
                };
                place_sixel_from_dcs(&dcs, placement.image_area)
            }
            _ => match place_terminal_image(
                protocol,
                &rendered,
                placement.image_area,
                excluded,
                None,
                self.cached_terminal_window(),
            ) {
                Ok(bytes) => bytes,
                Err(error) if protocol == ImageProtocol::Sixel => {
                    preview_log(format_args!(
                        "present_pdf_overlay: invalidating cached sixel render after display error: {error}"
                    ));
                    self.invalidate_rendered_pdf(&render_key);
                    let _ = self.ensure_pdf_render(&render_key);
                    return Ok(OverlayPresentState::Waiting);
                }
                Err(error) => return Err(error).context("failed to display PDF page"),
            },
        };
        preview_log(format_args!(
            "present_pdf_overlay: placed {} bytes via {protocol:?}",
            bytes.len()
        ));
        out.extend(bytes);
        if protocol == ImageProtocol::Sixel && image_changed {
            self.queue_sixel_repaint();
            self.queue_windows_terminal_pdf_sixel_repaint();
        }
        self.preview.pdf.displayed = Some(displayed);
        self.preview.pdf.displayed_excluded = excluded.to_vec();
        self.clear_pending_iterm_popup_restore();
        Ok(OverlayPresentState::Displayed)
    }

    pub(crate) fn preview_prefers_pdf_surface(&self) -> bool {
        if !self.terminal_image_overlay_available()
            || !self.preview.pdf.pdf_tools_available
            || self.preview.pdf.session.is_none()
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
        if self.preview.pdf.failed_page_probes.contains(&page_key) {
            return false;
        }
        if self.preview.pdf.pending_page_probes.contains(&page_key)
            || !self.preview.pdf.page_dimensions.contains_key(&page_key)
        {
            return true;
        }

        let Some(placement) = self.overlay_placement_for_request(&request) else {
            return false;
        };
        let render_key = self.pdf_render_key_from_request(&request, placement);
        if self.preview.pdf.failed_renders.contains(&render_key) {
            return false;
        }
        self.preview.pdf.pending_renders.contains(&render_key)
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
        if self.preview.pdf.failed_page_probes.contains(&page_key) {
            return Some("PDF preview unavailable".to_string());
        }
        if !self.pdf_selection_activation_ready()
            || !self.preview.pdf.page_dimensions.contains_key(&page_key)
            || self.preview.pdf.pending_page_probes.contains(&page_key)
        {
            return None;
        }

        let placement = self.overlay_placement_for_request(&request)?;
        let render_key = self.pdf_render_key_from_request(&request, placement);
        if self.preview.pdf.failed_renders.contains(&render_key) {
            return Some("PDF preview unavailable".to_string());
        }
        if self.cached_render_exists(&render_key) {
            return None;
        }
        None
    }

    pub(in crate::app) fn pdf_overlay_displayed(&self) -> bool {
        self.preview.pdf.displayed.is_some()
    }

    pub(in crate::app) fn displayed_pdf_overlay_area(&self) -> Option<Rect> {
        self.preview
            .pdf
            .displayed
            .as_ref()
            .map(|displayed| displayed.area)
    }

    pub(in crate::app) fn clear_displayed_pdf_overlay(&mut self) {
        self.preview.pdf.displayed = None;
        self.preview.pdf.displayed_excluded.clear();
    }

    pub(in crate::app) fn displayed_pdf_overlay_matches_active(&self) -> bool {
        self.active_pdf_display_target()
            .as_ref()
            .zip(self.preview.pdf.displayed.as_ref())
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
}
