mod geometry;
mod iterm;
mod kitty;
mod protocol;
mod sixel;
mod window;

use anyhow::{Context, Result};
use ratatui::layout::Rect;
use std::{env, io::Write as _, path::Path};

use crate::app::App;

pub(in crate::app) use self::geometry::{
    area_pixel_size, fit_image_area, fit_image_pixels, read_png_dimensions,
};
pub(in crate::app) use self::iterm::encode_iterm_inline_payload;
pub(in crate::app) use self::protocol::{command_exists, select_image_protocol};
use self::protocol::{detect_terminal_identity, pdf_preview_tools_available};
pub(in crate::app::overlays) use self::sixel::{encode_sixel_dcs, place_sixel_from_dcs};
use self::window::query_terminal_window_size;

/// Write a line to `<temp>/elio-preview.log` when `ELIO_DEBUG_PREVIEW` is set.
/// Does nothing (and compiles to nothing meaningful) when the env var is absent.
pub(in crate::app) fn preview_log(msg: impl std::fmt::Display) {
    if env::var_os("ELIO_DEBUG_PREVIEW").is_none() {
        return;
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join("elio-preview.log"))
        .and_then(|mut f| writeln!(f, "{msg}"));
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct TerminalImageState {
    pub(super) protocol: ImageProtocol,
    pub(super) window: Option<TerminalWindowSize>,
    pending_iterm_erase: Vec<Rect>,
    pending_kitty_resize_clear: bool,
    pending_iterm_popup_restore: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum OverlayPresentState {
    NotRequested,
    Waiting,
    Displayed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum TerminalIdentity {
    Kitty,
    Ghostty,
    Warp,
    WezTerm,
    ITerm2,
    Alacritty,
    Foot,
    WindowsTerminal,
    Other,
}

/// The wire protocol used to render images in the terminal preview pane.
/// Kept separate from `TerminalIdentity` so that multiple terminals can share
/// the same protocol without coupling detection logic to rendering logic.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) enum ImageProtocol {
    /// Kitty Graphics Protocol (APC `\x1b_G…\x1b\\`). Supported natively by
    /// Kitty, Ghostty, and Warp.
    KittyGraphics,
    /// iTerm2 inline image protocol (OSC 1337). Used by WezTerm and iTerm2.
    ItermInline,
    /// Sixel graphics protocol (DCS). Used by Windows Terminal (≥ 1.22).
    Sixel,
    #[default]
    None,
}

impl ImageProtocol {
    /// Returns `true` for pixel-buffer protocols that write directly into the
    /// terminal framebuffer and have no dedicated clear command (iTerm2 and
    /// Sixel).  These protocols require a pre-draw cell-erase pass before
    /// ratatui can safely overpaint stale image content.
    pub(in crate::app) fn is_raster(self) -> bool {
        matches!(self, ImageProtocol::ItermInline | ImageProtocol::Sixel)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(in crate::app) struct TerminalWindowSize {
    pub(super) cells_width: u16,
    pub(super) cells_height: u16,
    pub(super) pixels_width: u32,
    pub(super) pixels_height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct RenderedImageDimensions {
    pub(super) width_px: u32,
    pub(super) height_px: u32,
}

impl App {
    pub(crate) fn enable_terminal_image_previews(&mut self) {
        let identity = detect_terminal_identity();
        let image_previews_override = env::var_os("ELIO_IMAGE_PREVIEWS").is_some();
        let protocol = select_image_protocol(identity, image_previews_override);
        preview_log(format_args!(
            "enable_terminal_image_previews:\n  TERM={}\n  TERM_PROGRAM={}\n  KITTY_WINDOW_ID={}\n  WARP_SESSION_ID={}\n  WT_SESSION={}\n  identity={identity:?}\n  override={image_previews_override}\n  protocol={protocol:?}",
            env::var("TERM").unwrap_or_default(),
            env::var("TERM_PROGRAM").unwrap_or_default(),
            env::var_os("KITTY_WINDOW_ID").is_some(),
            env::var_os("WARP_SESSION_ID").is_some(),
            env::var_os("WT_SESSION").is_some(),
        ));
        self.preview.terminal_images.protocol = protocol;
        self.preview.pdf.pdf_tools_available = pdf_preview_tools_available();
        self.refresh_terminal_image_window_size();
        preview_log(format_args!(
            "  window={:?}",
            self.preview.terminal_images.window
        ));
        self.sync_pdf_preview_selection();
    }

    pub(crate) fn handle_terminal_image_resize(&mut self) {
        self.refresh_terminal_image_window_size();
        if self.preview.terminal_images.protocol == ImageProtocol::KittyGraphics
            && (self.static_image_overlay_displayed() || self.pdf_overlay_displayed())
        {
            // Kitty/Ghostty unicode placeholders are normal terminal cells. During
            // a live resize the terminal can reflow those cells before we redraw,
            // so erasing only the old image rect is not enough — some placeholder
            // text may survive outside the previous bounds. Force a full-screen
            // clear on the next draw so ratatui repaints the entire alt screen.
            self.preview.terminal_images.pending_kitty_resize_clear = true;
        }
        self.handle_pdf_overlay_resize();
    }

    pub(crate) fn take_pending_kitty_resize_clear(&mut self) -> bool {
        if !self.preview.terminal_images.pending_kitty_resize_clear {
            return false;
        }

        self.preview.terminal_images.pending_kitty_resize_clear = false;
        self.clear_displayed_static_image();
        self.clear_displayed_pdf_overlay();
        true
    }

    pub(in crate::app) fn terminal_image_overlay_available(&self) -> bool {
        self.preview.terminal_images.protocol != ImageProtocol::None
    }

    pub(in crate::app) fn cached_terminal_window(&self) -> Option<TerminalWindowSize> {
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
            .map(iterm::erase_cells)
            .unwrap_or_default()
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
            geometry::push_unique_rect(&mut areas, area);
        }
        if self.pdf_overlay_displayed()
            && !self.displayed_pdf_overlay_matches_active()
            && let Some(area) = self.displayed_pdf_overlay_area()
        {
            geometry::push_unique_rect(&mut areas, area);
        }
        if areas.is_empty() {
            return Vec::new();
        }
        let mut expanded_areas = Vec::with_capacity(areas.len());
        for area in areas {
            geometry::push_unique_rect(
                &mut expanded_areas,
                iterm::expand_iterm_erase_area(&self.input.frame_state, area),
            );
        }
        expanded_areas
            .into_iter()
            .flat_map(iterm::erase_cells)
            .collect()
    }

    pub(crate) fn present_preview_overlay(&mut self) -> Result<Vec<u8>> {
        if self.browser_wheel_burst_active() || self.preview.state.deferred_refresh_at.is_some() {
            return Ok(Vec::new());
        }

        let protocol = self.preview.terminal_images.protocol;
        if protocol == ImageProtocol::None {
            preview_log("present_preview_overlay: no protocol -> clear");
            return self.clear_preview_overlay();
        }

        let popup_open = self.any_modal_overlay_open();
        if protocol.is_raster()
            && popup_open
            && (self.static_image_overlay_displayed() || self.pdf_overlay_displayed())
        {
            self.preview.terminal_images.pending_iterm_popup_restore = true;
        }
        let force_iterm_popup_repaint = protocol.is_raster()
            && self.preview.terminal_images.pending_iterm_popup_restore
            && !popup_open;

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
            force_iterm_popup_repaint,
            &mut out,
        )?;
        preview_log(format_args!(
            "present_preview_overlay: protocol={protocol:?} static={static_state:?} out_len={}",
            out.len()
        ));
        match static_state {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => return Ok(out),
            OverlayPresentState::NotRequested => {}
        }

        let pdf_state =
            self.present_pdf_overlay(protocol, &excluded, force_iterm_popup_repaint, &mut out)?;
        preview_log(format_args!(
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
            force_iterm_popup_repaint,
            &mut out,
        )?;
        preview_log(format_args!(
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

    fn collect_popup_rects(&self) -> Vec<Rect> {
        let mut rects = Vec::new();
        if let Some(r) = self.input.frame_state.trash_panel {
            rects.push(r);
        }
        if let Some(r) = self.input.frame_state.restore_panel {
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
        self.overlays.trash.is_some()
            || self.overlays.restore.is_some()
            || self.overlays.create.is_some()
            || self.overlays.rename.is_some()
            || self.overlays.bulk_rename.is_some()
            || self.overlays.goto.is_some()
            || self.overlays.copy.is_some()
            || self.overlays.open_with.is_some()
            || self.overlays.search.is_some()
            || self.overlays.help
    }

    pub(in crate::app) fn clear_pending_iterm_popup_restore(&mut self) {
        self.preview.terminal_images.pending_iterm_popup_restore = false;
    }

    pub(crate) fn clear_preview_overlay(&mut self) -> Result<Vec<u8>> {
        if !self.static_image_overlay_displayed() && !self.pdf_overlay_displayed() {
            return Ok(Vec::new());
        }
        let bytes = clear_terminal_images(self.preview.terminal_images.protocol)
            .context("failed to clear preview overlay")?;
        // iTerm2 erase is emitted by iterm_pre_draw_erase() *before* terminal.draw(),
        // so ratatui naturally overpaints with the correct panel background. Nothing
        // extra needed here.
        self.clear_pending_iterm_popup_restore();
        self.clear_displayed_static_image();
        self.clear_displayed_pdf_overlay();
        Ok(bytes)
    }

    pub(crate) fn queue_forced_iterm_preview_erase(&mut self) {
        if !self.preview.terminal_images.protocol.is_raster() {
            return;
        }
        if let Some(area) = self.displayed_static_image_clear_area() {
            geometry::push_unique_rect(&mut self.preview.terminal_images.pending_iterm_erase, area);
        }
        if let Some(area) = self.displayed_pdf_overlay_area() {
            geometry::push_unique_rect(&mut self.preview.terminal_images.pending_iterm_erase, area);
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
            .then(query_terminal_window_size)
            .flatten();
    }
}

pub(in crate::app) fn place_terminal_image(
    protocol: ImageProtocol,
    path: &Path,
    area: Rect,
    excluded: &[Rect],
    inline_payload: Option<&str>,
    window_size: Option<TerminalWindowSize>,
) -> Result<Vec<u8>> {
    match protocol {
        ImageProtocol::KittyGraphics => {
            kitty::place_terminal_image_with_kitty_protocol(path, area, excluded)
        }
        ImageProtocol::ItermInline => {
            iterm::place_terminal_image_with_iterm_protocol(path, area, inline_payload)
        }
        ImageProtocol::Sixel => {
            let ws = window_size.ok_or_else(|| {
                anyhow::anyhow!("sixel protocol requires terminal window size, but none available")
            })?;
            sixel::place_terminal_image_with_sixel_protocol(path, area, ws)
        }
        ImageProtocol::None => Ok(Vec::new()),
    }
}

pub(in crate::app) fn clear_terminal_images(protocol: ImageProtocol) -> Result<Vec<u8>> {
    match protocol {
        ImageProtocol::KittyGraphics => kitty::clear_terminal_images_with_kitty_protocol(),
        // iTerm2 has no clear primitive — the overlay is erased by the next
        // ratatui draw call overwriting the cell region.
        ImageProtocol::ItermInline | ImageProtocol::None => Ok(Vec::new()),
        // Sixel also has no clear primitive (same as iTerm2).
        ImageProtocol::Sixel => sixel::clear_terminal_images_with_sixel_protocol(),
    }
}
