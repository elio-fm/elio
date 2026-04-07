use super::types::DisplayedStaticImagePreview;
use super::{
    StaticImageKey, StaticImageOverlayMode, StaticImageOverlayPreparation,
    StaticImageOverlayRequest,
};
use crate::app::App;
use crate::app::overlays::inline_image::{
    ImageProtocol, OverlayPresentState, RenderedImageDimensions, TerminalWindowSize,
    fit_image_area, place_terminal_image, preview_log,
};
use anyhow::Result;
use ratatui::layout::Rect;

impl App {
    pub(in crate::app) fn present_static_image_overlay(
        &mut self,
        protocol: ImageProtocol,
        excluded: &[Rect],
        force_repaint: bool,
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
        let image_changed = self.preview.image.displayed.as_ref() != Some(&displayed);
        let excluded_changed = excluded != self.preview.image.displayed_excluded.as_slice();
        let needs_repaint = force_repaint && protocol.is_raster();
        if !image_changed && !excluded_changed && !needs_repaint {
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
            Some(window_size),
        ) {
            Ok(bytes) => {
                preview_log(format_args!(
                    "present_static_image_overlay: placed {} bytes via {protocol:?}",
                    bytes.len()
                ));
                out.extend(bytes);
            }
            Err(error) => {
                preview_log(format_args!(
                    "present_static_image_overlay: place_terminal_image error: {error}"
                ));
                self.mark_static_image_failed(&request);
                self.refresh_preview();
                return Ok(OverlayPresentState::NotRequested);
            }
        }

        self.preview.image.displayed = Some(displayed);
        self.preview.image.displayed_excluded = excluded.to_vec();
        self.clear_pending_iterm_popup_restore();
        Ok(OverlayPresentState::Displayed)
    }

    pub(in crate::app) fn present_preview_visual_overlay(
        &mut self,
        protocol: ImageProtocol,
        excluded: &[Rect],
        force_repaint: bool,
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
        let image_changed = self.preview.image.displayed.as_ref() != Some(&displayed);
        let excluded_changed = excluded != self.preview.image.displayed_excluded.as_slice();
        let needs_repaint = force_repaint && protocol.is_raster();
        if !image_changed && !excluded_changed && !needs_repaint {
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
            Some(window_size),
        ) {
            Ok(bytes) => {
                preview_log(format_args!(
                    "present_preview_visual_overlay: placed {} bytes via {protocol:?}",
                    bytes.len()
                ));
                out.extend(bytes);
            }
            Err(error) => {
                preview_log(format_args!(
                    "present_preview_visual_overlay: place_terminal_image error: {error}"
                ));
                self.mark_static_image_failed(&request);
                return Ok(OverlayPresentState::NotRequested);
            }
        }

        self.preview.image.displayed = Some(displayed);
        self.record_comic_page_image_displayed();
        self.preview.image.displayed_excluded = excluded.to_vec();
        self.clear_pending_iterm_popup_restore();
        Ok(OverlayPresentState::Displayed)
    }

    pub(super) fn active_static_image_display_target(&self) -> Option<DisplayedStaticImagePreview> {
        let request = self
            .active_static_image_overlay_request()
            .or_else(|| self.active_preview_visual_overlay_request_unchecked())?;
        let window_size = self.cached_terminal_window()?;
        let image_dimensions = self
            .preview
            .image
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
        if self.preview.terminal_images.protocol.is_raster() {
            if request.mode == StaticImageOverlayMode::Inline {
                return self
                    .input
                    .frame_state
                    .preview_body_area
                    .or_else(|| self.preview_body_area())
                    .unwrap_or(request.area);
            }
            return self
                .input
                .frame_state
                .preview_content_area
                .unwrap_or(request.area);
        }
        request.area
    }

    fn preview_body_area(&self) -> Option<Rect> {
        match (
            self.input.frame_state.preview_media_area,
            self.input.frame_state.preview_content_area,
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
        // Kitty fills the entire cell area and lets the terminal handle scaling
        // via unicode placeholders, so no fitting is needed.  iTerm2 and Sixel
        // both paint pixel buffers and need a cell area that matches the image's
        // aspect ratio so the encoder can produce the correct pixel dimensions.
        if self.preview.terminal_images.protocol == ImageProtocol::KittyGraphics {
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
