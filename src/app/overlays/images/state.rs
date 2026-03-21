use super::{
    StaticImageFormat, StaticImageKey, StaticImageOverlayMode, StaticImageOverlayRequest,
    static_image_can_prepare_inline, static_image_detail_label,
    static_image_format_for_overlay_request, static_image_format_for_path,
};
use crate::app::overlays::inline_image::{ImageProtocol, command_exists};
use crate::app::state::PreviewLoadState;
use crate::app::{App, Entry};
use crate::preview;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

impl App {
    fn current_page_preview_visual_active(&self) -> bool {
        self.preview_state
            .content
            .preview_visual
            .as_ref()
            .is_some_and(|visual| visual.kind == preview::PreviewVisualKind::PageImage)
    }

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
        if self.static_image_can_display_directly_now(&request) {
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
                let ready_at =
                    self.last_selection_change_at + self.image_preview.selection_activation_delay;
                (Instant::now() < ready_at).then_some(ready_at)
            });
    }

    pub(in crate::app) fn mark_static_image_failed(&mut self, request: &StaticImageOverlayRequest) {
        self.image_preview
            .failed_images
            .insert(StaticImageKey::from_request(request));
    }

    pub(super) fn static_image_can_display_directly_now(
        &self,
        request: &StaticImageOverlayRequest,
    ) -> bool {
        self.terminal_images.protocol == ImageProtocol::KittyGraphics
            && !request.force_render_to_cache
            && static_image_format_for_overlay_request(request) == Some(StaticImageFormat::Png)
    }

    pub(super) fn static_image_can_use_source_path(
        &self,
        request: &StaticImageOverlayRequest,
    ) -> bool {
        match self.terminal_images.protocol {
            ImageProtocol::KittyGraphics => self.static_image_can_display_directly_now(request),
            ImageProtocol::ItermInline => {
                super::static_image_supports_iterm_source_passthrough(request)
            }
            ImageProtocol::None => false,
        }
    }

    pub(super) fn static_image_requires_prepare(
        &self,
        request: &StaticImageOverlayRequest,
    ) -> bool {
        request.prepare_inline_payload || !self.static_image_can_display_directly_now(request)
    }

    pub(super) fn magick_available(&mut self) -> bool {
        *self
            .image_preview
            .magick_available
            .get_or_insert_with(|| command_exists("magick"))
    }

    pub(super) fn resvg_available(&mut self) -> bool {
        *self
            .image_preview
            .resvg_available
            .get_or_insert_with(|| command_exists("resvg"))
    }

    pub(super) fn ffmpeg_available(&mut self) -> bool {
        *self
            .image_preview
            .ffmpeg_available
            .get_or_insert_with(|| command_exists("ffmpeg"))
    }

    #[cfg(test)]
    pub(in crate::app) fn set_ffmpeg_available_for_tests(&mut self, available: bool) {
        self.image_preview.ffmpeg_available = Some(available);
    }

    pub(in crate::app) fn image_selection_activation_ready(&self) -> bool {
        self.image_preview.activation_ready_at.is_none()
    }

    pub(in crate::app) fn static_image_overlay_displayed(&self) -> bool {
        self.image_preview.displayed.is_some()
    }

    pub(in crate::app) fn displayed_static_image_clear_area(&self) -> Option<Rect> {
        self.image_preview
            .displayed
            .as_ref()
            .map(|displayed| displayed.clear_area)
    }

    pub(in crate::app) fn clear_displayed_static_image(&mut self) {
        self.image_preview.displayed = None;
    }

    pub(in crate::app) fn preview_visual_force_render_to_cache(
        &self,
        visual: &preview::PreviewVisual,
    ) -> bool {
        if visual.kind != preview::PreviewVisualKind::PageImage {
            return false;
        }

        let Some(format) = static_image_format_for_path(&visual.path) else {
            return true;
        };
        let ffmpeg_available = self
            .image_preview
            .ffmpeg_available
            .unwrap_or_else(|| command_exists("ffmpeg"));
        !static_image_can_prepare_inline(visual.size, format, ffmpeg_available)
    }

    pub(in crate::app) fn displayed_static_image_matches_active(&self) -> bool {
        self.active_static_image_display_target()
            .as_ref()
            .zip(self.image_preview.displayed.as_ref())
            .is_some_and(|(active, displayed)| active == displayed)
    }

    pub(in crate::app) fn keep_displayed_static_image_overlay_while_pending(&self) -> bool {
        let Some(displayed) = self.image_preview.displayed.as_ref() else {
            return false;
        };
        match displayed.mode {
            StaticImageOverlayMode::Inline => {
                let loading_current_page_preview = self.current_page_preview_loading_active();
                if !self.current_page_preview_visual_active() && !loading_current_page_preview {
                    return false;
                }

                if loading_current_page_preview {
                    return true;
                }

                let Some(request) = self.active_preview_visual_overlay_request_unchecked() else {
                    return false;
                };
                self.keep_displayed_static_image_request_while_pending(&request)
            }
            StaticImageOverlayMode::FullPane => self
                .active_static_image_overlay_request()
                .is_some_and(|request| {
                    self.keep_displayed_static_image_request_while_pending(&request)
                }),
        }
    }

    pub(in crate::app) fn displayed_static_image_replaces_preview(&self) -> bool {
        self.image_preview
            .displayed
            .as_ref()
            .is_some_and(|displayed| displayed.mode == StaticImageOverlayMode::FullPane)
            && self.displayed_static_image_matches_active()
    }

    pub(super) fn static_image_overlay_request_for_entry(
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
            target_width_px: super::image_target_width_px(area, self.cached_terminal_window()),
            target_height_px: super::image_target_height_px(area, self.cached_terminal_window()),
            mode: StaticImageOverlayMode::FullPane,
            force_render_to_cache: false,
            prepare_inline_payload: self.terminal_images.protocol == ImageProtocol::ItermInline,
        })
    }

    fn current_page_preview_loading_active(&self) -> bool {
        self.preview_state
            .load_state
            .as_ref()
            .is_some_and(|load_state| {
                let loading_path = match load_state {
                    PreviewLoadState::Placeholder(path) | PreviewLoadState::Refreshing(path) => {
                        path
                    }
                };
                self.selected_entry()
                    .is_some_and(|entry| entry.path == *loading_path)
                    && (self.comic_preview_wheel_capture_active()
                        || self.epub_preview_wheel_capture_active())
            })
    }

    fn keep_displayed_static_image_request_while_pending(
        &self,
        request: &StaticImageOverlayRequest,
    ) -> bool {
        let key = StaticImageKey::from_request(request);
        if self.image_preview.failed_images.contains(&key) {
            return false;
        }
        if !self.image_selection_activation_ready() {
            return true;
        }

        self.image_preview.pending_prepares.contains(&key)
            || (self.static_image_requires_prepare(request)
                && !self.image_preview.dimensions.contains_key(&key))
    }
}
