use anyhow::{Context, Result};
use ratatui::{buffer::Buffer, layout::Rect};
use std::{
    env,
    time::{Duration, Instant},
};

use crate::app::App;
use crate::preview::OverlayPresentState;
use crate::terminal_runtime::terminal_images::{
    self, ImageProtocol, TerminalIdentity, TerminalWindowSize,
};

/// How long terminal geometry must stay unchanged before image payloads are
/// (re)transmitted. Tiling window managers resize the terminal in an animated
/// burst of SIGWINCH steps; placing the image on every step queues a full
/// multi-megabyte transmission per step into the tmux/terminal pipe, which the
/// terminal then takes seconds to chew through while the UI looks frozen.
/// Inside tmux the delay must exceed tmux's pane-resize throttle of roughly
/// one per 250ms, or intermediate steps still place; outside tmux resize
/// events arrive unthrottled, so a much shorter settle suffices.
const TMUX_RESIZE_SETTLE_DELAY: Duration = Duration::from_millis(300);
const RESIZE_SETTLE_DELAY: Duration = Duration::from_millis(100);

impl App {
    pub(crate) fn enable_terminal_image_previews(&mut self) {
        let was_available = self.terminal_image_overlay_available();
        let identity = terminal_images::detect_terminal_identity();
        let image_previews_override = env::var_os("ELIO_IMAGE_PREVIEWS").is_some();
        let protocol = terminal_images::select_image_protocol(identity, image_previews_override);
        terminal_images::preview_log(format_args!(
            "enable_terminal_image_previews:\n  TERM={}\n  TERM_PROGRAM={}\n  KITTY_WINDOW_ID={}\n  WARP_SESSION_ID={}\n  WT_SESSION={}\n  KONSOLE_DBUS_SESSION={}\n  KONSOLE_DBUS_SERVICE={}\n  KONSOLE_DBUS_WINDOW={}\n  identity={identity:?}\n  override={image_previews_override}\n  protocol={protocol:?}",
            env::var("TERM").unwrap_or_default(),
            env::var("TERM_PROGRAM").unwrap_or_default(),
            env::var_os("KITTY_WINDOW_ID").is_some(),
            env::var_os("WARP_SESSION_ID").is_some(),
            env::var_os("WT_SESSION").is_some(),
            env::var_os("KONSOLE_DBUS_SESSION").is_some(),
            env::var_os("KONSOLE_DBUS_SERVICE").is_some(),
            env::var_os("KONSOLE_DBUS_WINDOW").is_some(),
        ));
        self.preview.terminal_images.identity = identity;
        self.preview.terminal_images.protocol = protocol;
        if matches!(
            protocol,
            ImageProtocol::KittyGraphics
                | ImageProtocol::KittyDirectGraphics
                | ImageProtocol::ItermInline
                | ImageProtocol::Sixel
        ) {
            terminal_images::enable_allow_passthrough();
        }
        self.preview.pdf.pdf_tools_available = terminal_images::pdf_preview_tools_available();
        self.refresh_terminal_image_window_size();
        terminal_images::preview_log(format_args!(
            "  window={:?}",
            self.preview.terminal_images.window
        ));
        self.sync_pdf_preview_selection();
        if !was_available && self.terminal_image_overlay_available() {
            self.refresh_current_media_preview_after_image_support_enabled();
        }
    }

    pub(crate) fn refresh_current_media_preview_after_image_support_enabled(&mut self) {
        if !self.terminal_image_overlay_available()
            || self.preview.state.content.preview_visual.is_some()
            || !matches!(
                self.preview.state.content.kind,
                crate::preview::PreviewKind::Audio | crate::preview::PreviewKind::Video
            )
        {
            return;
        }

        self.refresh_preview();
    }

    pub(crate) fn handle_terminal_image_resize(&mut self) {
        self.refresh_terminal_image_window_size();
        self.arm_terminal_image_resize_settle();
        self.queue_terminal_image_geometry_clear();
        self.handle_pdf_overlay_resize();
    }

    pub(crate) fn handle_terminal_image_focus_gained(&mut self) {
        let previous_window = self.preview.terminal_images.window;
        self.refresh_terminal_image_window_size();
        if self.preview.terminal_images.window != previous_window {
            self.arm_terminal_image_resize_settle();
            self.queue_terminal_image_geometry_clear();
            self.handle_pdf_overlay_resize();
        }
    }

    fn arm_terminal_image_resize_settle(&mut self) {
        if self.preview.terminal_images.protocol == ImageProtocol::None {
            return;
        }
        let delay = if terminal_images::inside_tmux() {
            TMUX_RESIZE_SETTLE_DELAY
        } else {
            RESIZE_SETTLE_DELAY
        };
        self.preview.terminal_images.resize_settled_at = Some(Instant::now() + delay);
    }

    /// True while a resize burst is still in flight; image/PDF overlay
    /// placement is held (text renders normally) until geometry settles so a
    /// storm of SIGWINCH steps doesn't queue one full payload per step.
    pub(crate) fn terminal_image_resize_settling(&self) -> bool {
        self.preview
            .terminal_images
            .resize_settled_at
            .is_some_and(|settled_at| Instant::now() < settled_at)
    }

    /// Clears the settle timer once it expires. Returns `true` exactly once
    /// per resize burst so the caller can schedule the deferred placement draw.
    pub(crate) fn process_terminal_image_resize_settle_timer(&mut self) -> bool {
        let Some(settled_at) = self.preview.terminal_images.resize_settled_at else {
            return false;
        };
        if Instant::now() < settled_at {
            return false;
        }
        self.preview.terminal_images.resize_settled_at = None;
        true
    }

    pub(crate) fn pending_terminal_image_resize_settle_timer(&self) -> Option<Duration> {
        self.preview
            .terminal_images
            .resize_settled_at
            .map(|settled_at| settled_at.saturating_duration_since(Instant::now()))
    }

    pub(crate) fn queue_terminal_image_geometry_clear(&mut self) {
        if matches!(
            self.preview.terminal_images.protocol,
            ImageProtocol::KittyGraphics | ImageProtocol::ItermInline | ImageProtocol::Sixel
        ) && (self.static_image_overlay_displayed() || self.pdf_overlay_displayed())
        {
            // Kitty unicode placeholders can reflow on resize, iTerm inline
            // images can stale against old geometry in WezTerm, and Sixel can
            // leave framebuffer pixels outside the new bounds in Foot.
            // Force a full-screen clear on the next draw so ratatui repaints
            // the entire alt screen before the image is re-rendered.
            self.preview.terminal_images.pending_resize_clear = true;
        }
    }

    pub(crate) fn invalidate_terminal_image_overlay_after_terminal_task(&mut self) {
        if self.preview.terminal_images.protocol == ImageProtocol::None
            || (!self.static_image_overlay_displayed() && !self.pdf_overlay_displayed())
        {
            return;
        }

        // zoxide, Shell Here, Open With, and editor-based actions temporarily
        // hand the real terminal to another program. That program may leave the
        // alternate screen, clear it, or simply overpaint the image cells. The
        // terminal-side image is no longer trustworthy even though elio's active
        // preview request still matches the last displayed overlay, so force the
        // next frame through the same full-repaint path used after image geometry
        // changes and let normal presentation re-place the image.
        self.preview.terminal_images.pending_resize_clear = true;
    }

    pub(crate) fn take_pending_resize_clear(&mut self) -> bool {
        if !self.preview.terminal_images.pending_resize_clear {
            return false;
        }

        self.preview.terminal_images.pending_resize_clear = false;
        self.clear_displayed_static_image();
        self.clear_displayed_pdf_overlay();
        true
    }

    pub(crate) fn terminal_image_overlay_available(&self) -> bool {
        self.preview.terminal_images.protocol != ImageProtocol::None
    }

    pub(crate) fn uses_sixel_image_protocol(&self) -> bool {
        self.preview.terminal_images.protocol == ImageProtocol::Sixel
    }

    pub(crate) fn uses_iterm_inline_protocol_inside_tmux(&self) -> bool {
        self.preview.terminal_images.protocol == ImageProtocol::ItermInline
            && terminal_images::inside_tmux()
    }

    pub(crate) fn is_windows_terminal(&self) -> bool {
        self.preview.terminal_images.identity == TerminalIdentity::WindowsTerminal
    }

    pub(crate) fn needs_sixel_repaint_workaround(&self) -> bool {
        self.preview.terminal_images.protocol == ImageProtocol::Sixel
            && matches!(
                self.preview.terminal_images.identity,
                TerminalIdentity::Foot | TerminalIdentity::WindowsTerminal
            )
    }

    pub(crate) fn needs_slow_sixel_navigation_workaround(&self) -> bool {
        self.preview.terminal_images.protocol == ImageProtocol::Sixel
            && matches!(
                self.preview.terminal_images.identity,
                TerminalIdentity::Foot | TerminalIdentity::WindowsTerminal
            )
    }

    #[cfg(test)]
    pub(crate) fn set_terminal_image_protocol_for_tests(
        &mut self,
        protocol: ImageProtocol,
        identity: TerminalIdentity,
    ) {
        self.preview.terminal_images.protocol = protocol;
        self.preview.terminal_images.identity = identity;
    }

    /// Pushes the resize-settle deadline far enough into the future that tests
    /// can assert pending-settle behavior without depending on scheduler timing.
    #[cfg(test)]
    pub(crate) fn defer_terminal_image_resize_settle_for_tests(&mut self) {
        self.preview.terminal_images.resize_settled_at =
            Some(Instant::now() + Duration::from_secs(60));
    }

    /// Jumps past the resize-settle window so tests can exercise the placement
    /// that follows a resize without waiting out the real delay.
    #[cfg(test)]
    pub(crate) fn expire_terminal_image_resize_settle_for_tests(&mut self) {
        self.preview.terminal_images.resize_settled_at = None;
    }

    pub(crate) fn cached_terminal_window(&self) -> Option<TerminalWindowSize> {
        self.preview.terminal_images.window
    }

    /// Returns Kitty erase bytes that must be written to the terminal **before**
    /// `terminal.draw()` when a unicode-placeholder image is about to be replaced
    /// or cleared.
    ///
    /// Unlike standard Kitty placement, unicode placeholder cells are regular
    /// terminal characters. ratatui's differential renderer skips cells it
    /// considers "unchanged", leaving stale placeholder chars visible even after
    /// the image is no longer active. Emitting spaces to those cells before the
    /// draw forces the terminal to show blank content, which ratatui then
    /// overpaints correctly.
    pub(crate) fn kitty_pre_draw_erase(&self) -> Vec<u8> {
        if self.preview.terminal_images.protocol != ImageProtocol::KittyGraphics {
            return Vec::new();
        }
        let keep_stale = self.keep_displayed_static_image_overlay_while_pending();
        let needs_clear = (self.static_image_overlay_displayed()
            && !self.displayed_static_image_matches_active()
            && !keep_stale)
            || (self.pdf_overlay_displayed() && !self.displayed_pdf_overlay_matches_active());
        if !needs_clear {
            return Vec::new();
        }
        self.displayed_static_image_clear_area()
            .or_else(|| self.displayed_pdf_overlay_area())
            .map(terminal_images::erase_cells)
            .unwrap_or_default()
    }

    /// Erases blank modal cells that would otherwise show terminal image content
    /// through transparent popup surfaces. The tracked image remains logically
    /// displayed; raster protocols repaint it after the modal closes, while Kitty
    /// placeholders are redrawn with popup exclusions after the frame render.
    pub(crate) fn modal_image_post_draw_erase(
        &mut self,
        modal_rects: &[Rect],
        frame_buffer: &Buffer,
    ) -> Vec<u8> {
        let protocol = self.preview.terminal_images.protocol;
        if modal_rects.is_empty()
            || !matches!(
                protocol,
                ImageProtocol::KittyGraphics | ImageProtocol::ItermInline
            )
        {
            return Vec::new();
        }

        let image_rects = [
            self.displayed_static_image_clear_area(),
            self.displayed_pdf_overlay_area(),
        ];
        if image_rects.iter().all(Option::is_none) {
            return Vec::new();
        }

        let mut to_erase = Vec::new();
        for popup in modal_rects {
            for image in image_rects.iter().flatten() {
                if let Some(mask) = terminal_images::intersect_rect(*popup, *image) {
                    push_blank_cell_runs(&mut to_erase, mask, frame_buffer);
                }
            }
        }

        if to_erase.is_empty() {
            return Vec::new();
        }

        if protocol.is_raster() {
            self.preview.terminal_images.pending_iterm_popup_restore = true;
        }

        to_erase
            .into_iter()
            .flat_map(terminal_images::erase_cells)
            .collect()
    }

    /// Returns iTerm2 erase bytes that must be written to the terminal **before**
    /// `terminal.draw()` when an image is about to be replaced or cleared.
    ///
    /// Emitting the erase before the draw lets ratatui naturally overpaint the
    /// erased cells with the correct panel background in the same render pass,
    /// avoiding the black-background artifact that occurs when erasing after draw.
    pub(crate) fn iterm_pre_draw_erase(&mut self) -> Vec<u8> {
        if !self.preview.terminal_images.protocol.is_raster() {
            return Vec::new();
        }
        let mut areas = std::mem::take(&mut self.preview.terminal_images.pending_iterm_erase);
        let keep_stale = self.keep_displayed_static_image_overlay_while_pending();
        if self.static_image_overlay_displayed()
            && !self.displayed_static_image_matches_active()
            && !keep_stale
            && let Some(area) = self.displayed_static_image_clear_area()
        {
            terminal_images::push_unique_rect(&mut areas, area);
        }
        if self.pdf_overlay_displayed()
            && !self.displayed_pdf_overlay_matches_active()
            && let Some(area) = self.displayed_pdf_overlay_area()
        {
            terminal_images::push_unique_rect(&mut areas, area);
        }
        if areas.is_empty() {
            return Vec::new();
        }
        let mut expanded_areas = Vec::with_capacity(areas.len());
        for area in areas {
            let (expand_right, expand_bottom) = if self.displayed_static_image_mode()
                == Some(crate::app::preview::static_images::StaticImageOverlayMode::Inline)
            {
                (0, 0)
            } else if self.preview.terminal_images.identity == TerminalIdentity::WindowsTerminal
                && self.preview.terminal_images.protocol == ImageProtocol::Sixel
            {
                (1, 1)
            } else {
                (0, 2)
            };
            terminal_images::push_unique_rect(
                &mut expanded_areas,
                expand_raster_erase_area(
                    &self.input.frame_state,
                    area,
                    expand_right,
                    expand_bottom,
                ),
            );
        }
        expanded_areas
            .into_iter()
            .flat_map(terminal_images::erase_cells)
            .collect()
    }

    pub(crate) fn should_repaint_iterm_inline_under_modal(&self, modal_rects: &[Rect]) -> bool {
        let active_static_image = self.active_static_image_overlay_request().is_some()
            || self
                .active_preview_visual_overlay_request_unchecked()
                .is_some();
        let active_pdf_overlay = self.active_pdf_overlay_requested();
        self.preview.terminal_images.protocol == ImageProtocol::ItermInline
            && !modal_rects.is_empty()
            && self.any_modal_overlay_open()
            && ((active_static_image
                && ((!self.displayed_static_image_matches_active()
                    && !self.keep_displayed_static_image_overlay_while_pending())
                    || self.preview.terminal_images.pending_iterm_popup_restore))
                || (active_pdf_overlay && !self.displayed_pdf_overlay_matches_active()))
    }

    pub(crate) fn should_repaint_sixel_under_modal(&self, modal_rects: &[Rect]) -> bool {
        let active_static_image = self.active_static_image_overlay_request().is_some()
            || self
                .active_preview_visual_overlay_request_unchecked()
                .is_some();
        let active_pdf_overlay = self.active_pdf_overlay_requested();
        self.needs_sixel_repaint_workaround()
            && !modal_rects.is_empty()
            && self.any_modal_overlay_open()
            && ((active_static_image
                && !self.displayed_static_image_matches_active()
                && !self.keep_displayed_static_image_overlay_while_pending())
                || (active_pdf_overlay && !self.displayed_pdf_overlay_matches_active()))
    }

    pub(crate) fn present_preview_overlay(&mut self) -> Result<Vec<u8>> {
        self.present_preview_overlay_inner(false)
    }

    pub(crate) fn present_preview_overlay_behind_modal(&mut self) -> Result<Vec<u8>> {
        let out = self.present_preview_overlay_inner(true)?;
        if !out.is_empty()
            && self.preview.terminal_images.protocol == ImageProtocol::ItermInline
            && self.any_modal_overlay_open()
        {
            self.preview.terminal_images.pending_iterm_popup_restore = true;
        }
        Ok(out)
    }

    fn present_preview_overlay_inner(
        &mut self,
        allow_iterm_modal_repaint: bool,
    ) -> Result<Vec<u8>> {
        if self.browser_wheel_burst_active() || self.preview.state.deferred_refresh_at.is_some() {
            return Ok(Vec::new());
        }

        let protocol = self.preview.terminal_images.protocol;
        if protocol == ImageProtocol::None {
            terminal_images::preview_log("present_preview_overlay: no protocol -> clear");
            return self.clear_preview_overlay();
        }

        let popup_open = self.any_modal_overlay_open();
        if protocol == ImageProtocol::KittyDirectGraphics && popup_open {
            if self.static_image_overlay_displayed() || self.pdf_overlay_displayed() {
                return self.clear_preview_overlay();
            }
            return Ok(Vec::new());
        }

        if protocol == ImageProtocol::ItermInline && popup_open && !allow_iterm_modal_repaint {
            if self.static_image_overlay_displayed() || self.pdf_overlay_displayed() {
                self.preview.terminal_images.pending_iterm_popup_restore = true;
            }
            return Ok(Vec::new());
        }
        if self.needs_sixel_repaint_workaround() && popup_open && !allow_iterm_modal_repaint {
            return Ok(Vec::new());
        }
        if protocol == ImageProtocol::Sixel
            && popup_open
            && (self.static_image_overlay_displayed() || self.pdf_overlay_displayed())
        {
            self.preview.terminal_images.pending_iterm_popup_restore = true;
        }
        let force_sixel_repaint = protocol == ImageProtocol::Sixel
            && std::mem::take(&mut self.preview.terminal_images.pending_sixel_repaint);
        let force_iterm_popup_repaint = protocol.is_raster()
            && self.preview.terminal_images.pending_iterm_popup_restore
            && (!popup_open || allow_iterm_modal_repaint);
        let force_protocol_repaint = force_iterm_popup_repaint || force_sixel_repaint;

        // For Kitty, collect rects occupied by open popups so the image can be
        // rendered only in cells not covered by any popup.
        let excluded: Vec<Rect> = if protocol == ImageProtocol::KittyGraphics {
            self.collect_popup_rects()
        } else {
            Vec::new()
        };

        let keep_stale_page_preview_overlay =
            self.keep_displayed_static_image_overlay_while_pending();
        let mut out = Vec::new();
        if (self.static_image_overlay_displayed()
            && !self.displayed_static_image_matches_active()
            && !keep_stale_page_preview_overlay)
            || self.pdf_overlay_displayed() && !self.displayed_pdf_overlay_matches_active()
        {
            out.extend(self.clear_preview_overlay()?);
        }

        let static_state = self.present_static_image_overlay(
            protocol,
            &excluded,
            force_protocol_repaint,
            &mut out,
        )?;
        terminal_images::preview_log(format_args!(
            "present_preview_overlay: protocol={protocol:?} static={static_state:?} out_len={}",
            out.len()
        ));
        match static_state {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => return Ok(out),
            OverlayPresentState::NotRequested => {}
        }

        let pdf_state =
            self.present_pdf_overlay(protocol, &excluded, force_protocol_repaint, &mut out)?;
        terminal_images::preview_log(format_args!(
            "present_preview_overlay: pdf={pdf_state:?} out_len={}",
            out.len()
        ));
        match pdf_state {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => return Ok(out),
            OverlayPresentState::NotRequested => {}
        }

        let visual_state = self.present_preview_visual_overlay(
            protocol,
            &excluded,
            force_protocol_repaint,
            &mut out,
        )?;
        terminal_images::preview_log(format_args!(
            "present_preview_overlay: visual={visual_state:?} out_len={}",
            out.len()
        ));
        match visual_state {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => Ok(out),
            OverlayPresentState::NotRequested if keep_stale_page_preview_overlay => Ok(out),
            OverlayPresentState::NotRequested => {
                self.preview.terminal_images.pending_iterm_popup_restore = false;
                out.extend(self.clear_preview_overlay()?);
                Ok(out)
            }
        }
    }

    pub(crate) fn collect_popup_rects(&self) -> Vec<Rect> {
        let mut rects = Vec::new();
        if let Some(r) = self.input.frame_state.trash_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.restore_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.archive_password_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.archive_create_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.create_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.rename_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.goto_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.copy_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.open_with_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.search_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.help_panel {
            rects.push(r);
        }
        rects
    }

    fn any_modal_overlay_open(&self) -> bool {
        self.file_operations.trash.is_some()
            || self.file_operations.restore.is_some()
            || self.file_operations.archive_password.is_some()
            || self.file_operations.archive_create.is_some()
            || self.file_operations.create.is_some()
            || self.file_operations.rename.is_some()
            || self.file_operations.bulk_rename.is_some()
            || self.overlays.goto.is_some()
            || self.file_operations.copy.is_some()
            || self.overlays.open_with.is_some()
            || self.fuzzy_finder.search.is_some()
            || self.overlays.help
    }

    pub(crate) fn clear_pending_iterm_popup_restore(&mut self) {
        self.preview.terminal_images.pending_iterm_popup_restore = false;
    }

    pub(crate) fn queue_sixel_repaint(&mut self) {
        if self.needs_sixel_repaint_workaround() {
            self.preview.terminal_images.pending_sixel_repaint = true;
        }
    }

    pub(crate) fn queue_windows_terminal_pdf_sixel_repaint(&mut self) {
        if self.preview.terminal_images.protocol == ImageProtocol::Sixel
            && self.preview.terminal_images.identity == TerminalIdentity::WindowsTerminal
        {
            self.preview.terminal_images.pending_sixel_repaint = true;
        }
    }

    pub(crate) fn clear_preview_overlay(&mut self) -> Result<Vec<u8>> {
        if !self.static_image_overlay_displayed() && !self.pdf_overlay_displayed() {
            return Ok(Vec::new());
        }
        let bytes = terminal_images::clear_terminal_images(self.preview.terminal_images.protocol)
            .context("failed to clear preview overlay")?;
        // iTerm2 erase is emitted by iterm_pre_draw_erase() *before* terminal.draw(),
        // so ratatui naturally overpaints with the correct panel background. Nothing
        // extra needed here.
        self.clear_pending_iterm_popup_restore();
        self.clear_displayed_static_image();
        self.clear_displayed_pdf_overlay();
        Ok(bytes)
    }

    pub(crate) fn sixel_modal_collision_erase(
        &mut self,
        modal_rects: &[Rect],
    ) -> (Vec<Rect>, Vec<u8>) {
        if !self.needs_sixel_repaint_workaround() || modal_rects.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let image_rects = [
            self.displayed_static_image_clear_area(),
            self.displayed_pdf_overlay_area(),
        ];
        if image_rects.iter().all(Option::is_none) {
            return (Vec::new(), Vec::new());
        }

        let mut collisions = Vec::new();
        for popup in modal_rects {
            for image in image_rects.iter().flatten() {
                if let Some(collision) = terminal_images::intersect_rect(*popup, *image) {
                    terminal_images::push_unique_rect(&mut collisions, collision);
                }
            }
        }
        if collisions.is_empty() {
            return (Vec::new(), Vec::new());
        }

        self.preview.terminal_images.pending_sixel_repaint = true;
        let erase = collisions
            .iter()
            .copied()
            .flat_map(terminal_images::erase_cells)
            .collect();
        (collisions, erase)
    }

    pub(crate) fn queue_forced_iterm_preview_erase(&mut self) {
        if !self.preview.terminal_images.protocol.is_raster() {
            return;
        }
        if let Some(area) = self.displayed_static_image_clear_area() {
            terminal_images::push_unique_rect(
                &mut self.preview.terminal_images.pending_iterm_erase,
                area,
            );
        }
        if let Some(area) = self.displayed_pdf_overlay_area() {
            terminal_images::push_unique_rect(
                &mut self.preview.terminal_images.pending_iterm_erase,
                area,
            );
        }
    }

    pub(crate) fn preview_uses_image_overlay(&self) -> bool {
        self.displayed_static_image_replaces_preview()
            || self.displayed_pdf_overlay_matches_active()
    }

    pub(crate) fn preview_prefers_image_surface(&self) -> bool {
        self.preview_prefers_static_image_surface() || self.preview_prefers_pdf_surface()
    }

    fn refresh_terminal_image_window_size(&mut self) {
        self.preview.terminal_images.window = (self.preview.terminal_images.protocol
            != ImageProtocol::None)
            .then(terminal_images::query_terminal_window_size)
            .flatten();
    }
}

pub(super) fn push_blank_cell_runs(rects: &mut Vec<Rect>, area: Rect, frame_buffer: &Buffer) {
    let Some(area) = terminal_images::intersect_rect(area, *frame_buffer.area()) else {
        return;
    };

    for y in area.y..area.y.saturating_add(area.height) {
        let mut run_start = None;
        for x in area.x..area.x.saturating_add(area.width) {
            let transparent_blank = frame_buffer.cell((x, y)).is_some_and(|cell| {
                cell.symbol() == " " && cell.bg == ratatui::style::Color::Reset
            });
            match (transparent_blank, run_start) {
                (true, None) => run_start = Some(x),
                (false, Some(start)) => {
                    terminal_images::push_unique_rect(
                        rects,
                        Rect {
                            x: start,
                            y,
                            width: x.saturating_sub(start),
                            height: 1,
                        },
                    );
                    run_start = None;
                }
                _ => {}
            }
        }
        if let Some(start) = run_start {
            terminal_images::push_unique_rect(
                rects,
                Rect {
                    x: start,
                    y,
                    width: area.x.saturating_add(area.width).saturating_sub(start),
                    height: 1,
                },
            );
        }
    }
}

pub(super) fn expand_raster_erase_area(
    frame_state: &crate::app::FrameState,
    area: Rect,
    expand_right: u16,
    expand_bottom: u16,
) -> Rect {
    let bounds = frame_state
        .preview_body_area
        .or(frame_state.preview_content_area)
        .unwrap_or(area);
    // Only erase inside the preview body/content, never into the pane border.
    // Ratatui may skip unchanged border cells on the following draw, so erasing
    // the bottom border here can leave it blank in raster terminals like WezTerm.
    let clamped = terminal_images::intersect_rect(area, bounds).unwrap_or(area);
    let right = clamped.x.saturating_add(clamped.width);
    let bottom = clamped.y.saturating_add(clamped.height);
    let bounds_right = bounds.x.saturating_add(bounds.width);
    let bounds_bottom = bounds.y.saturating_add(bounds.height);
    let extra_cols = bounds_right.saturating_sub(right).min(expand_right);
    let extra_rows = bounds_bottom.saturating_sub(bottom).min(expand_bottom);
    Rect {
        x: clamped.x,
        y: clamped.y,
        width: clamped.width.saturating_add(extra_cols),
        height: clamped.height.saturating_add(extra_rows),
    }
}
