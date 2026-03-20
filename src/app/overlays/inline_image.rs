use super::super::*;
use anyhow::{Context, Result};
use base64::Engine as _;
use crossterm::terminal;
use ratatui::{layout::Rect, style::Color};
use std::{
    env,
    fs::{self, File},
    io::{Read, Write as _},
    path::Path,
    sync::Arc,
};

/// Write a line to `/tmp/elio-preview.log` when `ELIO_DEBUG_PREVIEW` is set.
/// Does nothing (and compiles to nothing meaningful) when the env var is absent.
pub(in crate::app) fn preview_log(msg: impl std::fmt::Display) {
    if env::var_os("ELIO_DEBUG_PREVIEW").is_none() {
        return;
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/elio-preview.log")
        .and_then(|mut f| writeln!(f, "{msg}"));
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct TerminalImageState {
    pub(super) protocol: ImageProtocol,
    pub(super) window: Option<TerminalWindowSize>,
    pending_iterm_erase: Vec<Rect>,
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
    Alacritty,
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
    /// iTerm2 inline image protocol (OSC 1337). WezTerm's preferred path.
    ItermInline,
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
            "enable_terminal_image_previews:\n  TERM={}\n  TERM_PROGRAM={}\n  KITTY_WINDOW_ID={}\n  WARP_SESSION_ID={}\n  identity={identity:?}\n  override={image_previews_override}\n  protocol={protocol:?}",
            env::var("TERM").unwrap_or_default(),
            env::var("TERM_PROGRAM").unwrap_or_default(),
            env::var_os("KITTY_WINDOW_ID").is_some(),
            env::var_os("WARP_SESSION_ID").is_some(),
        ));
        self.terminal_images.protocol = protocol;
        self.pdf_preview.pdf_tools_available = pdf_preview_tools_available();
        self.refresh_terminal_image_window_size();
        preview_log(format_args!("  window={:?}", self.terminal_images.window));
        self.sync_pdf_preview_selection();
    }

    pub(crate) fn handle_terminal_image_resize(&mut self) {
        self.refresh_terminal_image_window_size();
        self.handle_pdf_overlay_resize();
    }

    pub(in crate::app) fn terminal_image_overlay_available(&self) -> bool {
        self.terminal_images.protocol != ImageProtocol::None
    }

    pub(in crate::app) fn cached_terminal_window(&self) -> Option<TerminalWindowSize> {
        self.terminal_images.window
    }

    /// Returns iTerm2 erase bytes that must be written to the terminal **before**
    /// `terminal.draw()` when an image is about to be replaced or cleared.
    ///
    /// Emitting the erase before the draw lets ratatui naturally overpaint the
    /// erased cells with the correct panel background in the same render pass,
    /// avoiding the black-background artifact that occurs when erasing after draw.
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
        if self.terminal_images.protocol != ImageProtocol::KittyGraphics {
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
            .map(erase_cells)
            .unwrap_or_default()
    }

    pub(crate) fn iterm_pre_draw_erase(&mut self) -> Vec<u8> {
        if self.terminal_images.protocol != ImageProtocol::ItermInline {
            return Vec::new();
        }
        let mut areas = std::mem::take(&mut self.terminal_images.pending_iterm_erase);
        let keep_stale = self.keep_displayed_static_image_overlay_while_pending();
        if self.static_image_overlay_displayed()
            && !self.displayed_static_image_matches_active()
            && !keep_stale
            && let Some(area) = self.displayed_static_image_clear_area()
        {
            push_unique_rect(&mut areas, area);
        }
        if self.pdf_overlay_displayed()
            && !self.displayed_pdf_overlay_matches_active()
            && let Some(area) = self.displayed_pdf_overlay_area()
        {
            push_unique_rect(&mut areas, area);
        }
        if areas.is_empty() {
            return Vec::new();
        }
        let mut expanded_areas = Vec::with_capacity(areas.len());
        for area in areas {
            push_unique_rect(&mut expanded_areas, self.expand_iterm_erase_area(area));
        }
        expanded_areas.into_iter().flat_map(erase_cells).collect()
    }

    pub(crate) fn present_preview_overlay(&mut self) -> Result<Vec<u8>> {
        if self.browser_wheel_burst_active() {
            return Ok(Vec::new());
        }

        let protocol = self.terminal_images.protocol;
        if protocol == ImageProtocol::None {
            preview_log("present_preview_overlay: no protocol → clear");
            return self.clear_preview_overlay();
        }

        let any_overlay_open = self.trash.is_some()
            || self.restore.is_some()
            || self.create.is_some()
            || self.rename.is_some()
            || self.bulk_rename.is_some()
            || self.search.is_some()
            || self.help_open;

        // Non-Kitty protocols (e.g. iTerm2) have no unicode placeholder support —
        // clear the image when any popup is open.
        if any_overlay_open && protocol != ImageProtocol::KittyGraphics {
            self.queue_forced_iterm_preview_erase();
            return self.clear_preview_overlay();
        }

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

        let static_state = self.present_static_image_overlay(protocol, &excluded, &mut out)?;
        preview_log(format_args!(
            "present_preview_overlay: protocol={protocol:?} static={static_state:?} out_len={}",
            out.len()
        ));
        match static_state {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => return Ok(out),
            OverlayPresentState::NotRequested => {}
        }

        let pdf_state = self.present_pdf_overlay(protocol, &excluded, &mut out)?;
        preview_log(format_args!(
            "present_preview_overlay: pdf={pdf_state:?} out_len={}",
            out.len()
        ));
        match pdf_state {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => return Ok(out),
            OverlayPresentState::NotRequested => {}
        }

        let visual_state = self.present_preview_visual_overlay(protocol, &excluded, &mut out)?;
        preview_log(format_args!(
            "present_preview_overlay: visual={visual_state:?} out_len={}",
            out.len()
        ));
        match visual_state {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => Ok(out),
            OverlayPresentState::NotRequested if keep_stale_page_preview_overlay => Ok(out),
            OverlayPresentState::NotRequested => {
                out.extend(self.clear_preview_overlay()?);
                Ok(out)
            }
        }
    }

    fn collect_popup_rects(&self) -> Vec<Rect> {
        let mut rects = Vec::new();
        if let Some(r) = self.frame_state.trash_panel {
            rects.push(r);
        }
        if let Some(r) = self.frame_state.restore_panel {
            rects.push(r);
        }
        if let Some(r) = self.frame_state.create_panel {
            rects.push(r);
        }
        if let Some(r) = self.frame_state.rename_panel {
            rects.push(r);
        }
        if let Some(r) = self.frame_state.search_panel {
            rects.push(r);
        }
        if let Some(r) = self.frame_state.help_panel {
            rects.push(r);
        }
        rects
    }

    pub(crate) fn clear_preview_overlay(&mut self) -> Result<Vec<u8>> {
        if !self.static_image_overlay_displayed() && !self.pdf_overlay_displayed() {
            return Ok(Vec::new());
        }
        let bytes = clear_terminal_images(self.terminal_images.protocol)
            .context("failed to clear preview overlay")?;
        // iTerm2 erase is emitted by iterm_pre_draw_erase() *before* terminal.draw(),
        // so ratatui naturally overpaints with the correct panel background. Nothing
        // extra needed here.
        self.clear_displayed_static_image();
        self.clear_displayed_pdf_overlay();
        Ok(bytes)
    }

    pub(crate) fn queue_forced_iterm_preview_erase(&mut self) {
        if self.terminal_images.protocol != ImageProtocol::ItermInline {
            return;
        }
        if let Some(area) = self.displayed_static_image_clear_area() {
            push_unique_rect(&mut self.terminal_images.pending_iterm_erase, area);
        }
        if let Some(area) = self.displayed_pdf_overlay_area() {
            push_unique_rect(&mut self.terminal_images.pending_iterm_erase, area);
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
        self.terminal_images.window = (self.terminal_images.protocol != ImageProtocol::None)
            .then(query_terminal_window_size)
            .flatten();
    }

    fn expand_iterm_erase_area(&self, area: Rect) -> Rect {
        let safe_bounds = self
            .frame_state
            .preview_body_area
            .or(self.frame_state.preview_content_area)
            .unwrap_or(area);
        let Some(bounds) = self.frame_state.preview_panel.or(Some(safe_bounds)) else {
            return area;
        };
        let clamped = intersect_rect(area, safe_bounds).unwrap_or(area);
        let bottom = clamped.y.saturating_add(clamped.height);
        let bounds_bottom = bounds.y.saturating_add(bounds.height);
        if bottom >= bounds_bottom {
            return clamped;
        }
        let extra_rows = bounds_bottom.saturating_sub(bottom).min(2);
        Rect {
            x: clamped.x,
            y: clamped.y,
            width: clamped.width,
            height: clamped.height.saturating_add(extra_rows),
        }
    }
}

fn pdf_preview_tools_available() -> bool {
    command_exists("pdfinfo") && command_exists("pdftocairo")
}

pub(in crate::app) fn detect_terminal_identity() -> TerminalIdentity {
    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    let term_program = env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kitty_window_id = env::var_os("KITTY_WINDOW_ID").is_some();

    if kitty_window_id || term.contains("xterm-kitty") || term_program == "kitty" {
        TerminalIdentity::Kitty
    } else if term.contains("ghostty") || term_program == "ghostty" {
        TerminalIdentity::Ghostty
    } else if term.contains("wezterm") || term_program == "wezterm" {
        TerminalIdentity::WezTerm
    } else if term_program.contains("warp") || env::var_os("WARP_SESSION_ID").is_some() {
        TerminalIdentity::Warp
    } else if term.contains("alacritty")
        || term_program.contains("alacritty")
        || env::var_os("ALACRITTY_SOCKET").is_some()
    {
        TerminalIdentity::Alacritty
    } else {
        TerminalIdentity::Other
    }
}

pub(in crate::app) fn select_image_protocol(
    identity: TerminalIdentity,
    image_previews_override: bool,
) -> ImageProtocol {
    match identity {
        TerminalIdentity::Kitty => ImageProtocol::KittyGraphics,
        TerminalIdentity::Ghostty => ImageProtocol::KittyGraphics,
        TerminalIdentity::Warp => ImageProtocol::KittyGraphics,
        TerminalIdentity::WezTerm => ImageProtocol::ItermInline,
        // ELIO_IMAGE_PREVIEWS=1 force-enables KittyGraphics on unrecognised terminals
        // for testing. Alacritty is excluded — it does not support image protocols.
        TerminalIdentity::Other if image_previews_override => ImageProtocol::KittyGraphics,
        TerminalIdentity::Alacritty | TerminalIdentity::Other => ImageProtocol::None,
    }
}

pub(in crate::app) fn query_terminal_window_size() -> Option<TerminalWindowSize> {
    let terminal_size = terminal::window_size().ok();
    let (cells_width, cells_height) = terminal_size
        .as_ref()
        .map(|size| (size.columns, size.rows))
        .or_else(|| terminal::size().ok())?;
    let (pixels_width, pixels_height) = terminal_size
        .as_ref()
        .and_then(|size| {
            let width = u32::from(size.width);
            let height = u32::from(size.height);
            (width > 0 && height > 0).then_some((width, height))
        })
        .unwrap_or_else(|| fallback_window_size_pixels(cells_width, cells_height));
    Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width,
        pixels_height,
    })
}

#[cfg(test)]
pub(in crate::app) fn parse_window_size(output: &str) -> Option<(u32, u32)> {
    let trimmed = output.trim();
    let (width, height) = trimmed.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

pub(in crate::app) fn fallback_window_size_pixels(
    cells_width: u16,
    cells_height: u16,
) -> (u32, u32) {
    (
        u32::from(cells_width.max(1)) * 8,
        u32::from(cells_height.max(1)) * 16,
    )
}

pub(in crate::app) fn read_png_dimensions(path: &Path) -> Option<RenderedImageDimensions> {
    let mut file = File::open(path).ok()?;
    let mut header = [0_u8; 24];
    file.read_exact(&mut header).ok()?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        return None;
    }

    let width_px = u32::from_be_bytes(header[16..20].try_into().ok()?);
    let height_px = u32::from_be_bytes(header[20..24].try_into().ok()?);
    (width_px > 0 && height_px > 0).then_some(RenderedImageDimensions {
        width_px,
        height_px,
    })
}

pub(in crate::app) fn fit_image_pixels(
    area: Rect,
    window_size: TerminalWindowSize,
    aspect_ratio: f32,
) -> (f32, f32) {
    let aspect_ratio = aspect_ratio.max(f32::EPSILON);
    let cell_width_px = window_size.pixels_width as f32 / f32::from(window_size.cells_width.max(1));
    let cell_height_px =
        window_size.pixels_height as f32 / f32::from(window_size.cells_height.max(1));
    let area_width_px = f32::from(area.width.max(1)) * cell_width_px;
    let area_height_px = f32::from(area.height.max(1)) * cell_height_px;

    if area_width_px / area_height_px > aspect_ratio {
        let height = area_height_px;
        (height * aspect_ratio, height)
    } else {
        let width = area_width_px;
        (width, width / aspect_ratio)
    }
}

pub(in crate::app) fn fit_image_area(
    area: Rect,
    window_size: TerminalWindowSize,
    aspect_ratio: f32,
) -> Rect {
    let cell_width_px = window_size.pixels_width as f32 / f32::from(window_size.cells_width.max(1));
    let cell_height_px =
        window_size.pixels_height as f32 / f32::from(window_size.cells_height.max(1));
    let (fit_width_px, fit_height_px) = fit_image_pixels(area, window_size, aspect_ratio);
    let width_cells = ((fit_width_px / cell_width_px).round() as u16).clamp(1, area.width.max(1));
    let height_cells =
        ((fit_height_px / cell_height_px).round() as u16).clamp(1, area.height.max(1));

    Rect {
        x: area.x + (area.width.saturating_sub(width_cells)) / 2,
        y: area.y + (area.height.saturating_sub(height_cells)) / 2,
        width: width_cells,
        height: height_cells,
    }
}

pub(in crate::app) fn place_terminal_image(
    protocol: ImageProtocol,
    path: &Path,
    area: Rect,
    excluded: &[Rect],
    inline_payload: Option<&str>,
) -> Result<Vec<u8>> {
    match protocol {
        ImageProtocol::KittyGraphics => {
            place_terminal_image_with_kitty_protocol(path, area, excluded)
        }
        ImageProtocol::ItermInline => {
            place_terminal_image_with_iterm_protocol(path, area, inline_payload)
        }
        ImageProtocol::None => Ok(Vec::new()),
    }
}

pub(in crate::app) fn clear_terminal_images(protocol: ImageProtocol) -> Result<Vec<u8>> {
    match protocol {
        ImageProtocol::KittyGraphics => clear_terminal_images_with_kitty_protocol(),
        // iTerm2 protocol has no clear primitive — the overlay is erased by
        // the next ratatui draw call overwriting the cell region.
        ImageProtocol::ItermInline | ImageProtocol::None => Ok(Vec::new()),
    }
}

pub(in crate::app) fn build_kitty_upload_sequence(path: &Path, id: u32, area: Rect) -> String {
    let payload =
        base64::engine::general_purpose::STANDARD.encode(path.as_os_str().as_encoded_bytes());
    format!(
        "\u{1b}_Ga=T,q=2,f=100,t=f,U=1,i={id},p=1,c={},r={},C=1;{payload}\u{1b}\\",
        area.width.max(1),
        area.height.max(1),
    )
}

pub(in crate::app) fn build_kitty_placeholder_sequence(
    id: u32,
    area: Rect,
    excluded: &[Rect],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(usize::from(area.width) * usize::from(area.height) * 8 + 32);
    let (r, g, b) = ((id >> 16) & 0xff, (id >> 8) & 0xff, id & 0xff);
    match crate::ui::theme::palette().panel {
        Color::Rgb(bg_r, bg_g, bg_b) => {
            let _ = write!(
                buf,
                "\x1b[38;2;{r};{g};{b};48;2;{bg_r};{bg_g};{bg_b};58;2;0;0;1m"
            );
        }
        _ => {
            let _ = write!(buf, "\x1b[38;2;{r};{g};{b};58;2;0;0;1m");
        }
    }
    for y in 0..area.height {
        let abs_row = area.y.saturating_add(y);
        let dy = usize::from(y).min(DIACRITICS.len() - 1);
        let mut need_pos = true;
        for x in 0..area.width {
            let abs_col = area.x.saturating_add(x);
            if excluded
                .iter()
                .any(|r| kitty_cell_in_rect(abs_col, abs_row, r))
            {
                need_pos = true;
                continue;
            }
            if need_pos {
                let _ = write!(
                    buf,
                    "\x1b[{};{}H",
                    abs_row.saturating_add(1),
                    abs_col.saturating_add(1)
                );
                need_pos = false;
            }
            let dx = usize::from(x).min(DIACRITICS.len() - 1);
            let _ = write!(buf, "\u{10EEEE}{}{}", DIACRITICS[dy], DIACRITICS[dx]);
        }
    }
    let _ = write!(buf, "\x1b[0m");
    buf
}

fn kitty_cell_in_rect(col: u16, row: u16, rect: &Rect) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

pub(in crate::app) fn kitty_image_id() -> u32 {
    std::process::id() % (0xff_ffff + 1)
}

#[rustfmt::skip]
static DIACRITICS: [char; 297] = [
    '\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}', '\u{0312}', '\u{033D}', '\u{033E}',
    '\u{033F}', '\u{0346}', '\u{034A}', '\u{034B}', '\u{034C}', '\u{0350}', '\u{0351}',
    '\u{0352}', '\u{0357}', '\u{035B}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}',
    '\u{0367}', '\u{0368}', '\u{0369}', '\u{036A}', '\u{036B}', '\u{036C}', '\u{036D}',
    '\u{036E}', '\u{036F}', '\u{0483}', '\u{0484}', '\u{0485}', '\u{0486}', '\u{0487}',
    '\u{0592}', '\u{0593}', '\u{0594}', '\u{0595}', '\u{0597}', '\u{0598}', '\u{0599}',
    '\u{059C}', '\u{059D}', '\u{059E}', '\u{059F}', '\u{05A0}', '\u{05A1}', '\u{05A8}',
    '\u{05A9}', '\u{05AB}', '\u{05AC}', '\u{05AF}', '\u{05C4}', '\u{0610}', '\u{0611}',
    '\u{0612}', '\u{0613}', '\u{0614}', '\u{0615}', '\u{0616}', '\u{0617}', '\u{0657}',
    '\u{0658}', '\u{0659}', '\u{065A}', '\u{065B}', '\u{065D}', '\u{065E}', '\u{06D6}',
    '\u{06D7}', '\u{06D8}', '\u{06D9}', '\u{06DA}', '\u{06DB}', '\u{06DC}', '\u{06DF}',
    '\u{06E0}', '\u{06E1}', '\u{06E2}', '\u{06E4}', '\u{06E7}', '\u{06E8}', '\u{06EB}',
    '\u{06EC}', '\u{0730}', '\u{0732}', '\u{0733}', '\u{0735}', '\u{0736}', '\u{073A}',
    '\u{073D}', '\u{073F}', '\u{0740}', '\u{0741}', '\u{0743}', '\u{0745}', '\u{0747}',
    '\u{0749}', '\u{074A}', '\u{07EB}', '\u{07EC}', '\u{07ED}', '\u{07EE}', '\u{07EF}',
    '\u{07F0}', '\u{07F1}', '\u{07F3}', '\u{0816}', '\u{0817}', '\u{0818}', '\u{0819}',
    '\u{081B}', '\u{081C}', '\u{081D}', '\u{081E}', '\u{081F}', '\u{0820}', '\u{0821}',
    '\u{0822}', '\u{0823}', '\u{0825}', '\u{0826}', '\u{0827}', '\u{0829}', '\u{082A}',
    '\u{082B}', '\u{082C}', '\u{082D}', '\u{0951}', '\u{0953}', '\u{0954}', '\u{0F82}',
    '\u{0F83}', '\u{0F86}', '\u{0F87}', '\u{135D}', '\u{135E}', '\u{135F}', '\u{17DD}',
    '\u{193A}', '\u{1A17}', '\u{1A75}', '\u{1A76}', '\u{1A77}', '\u{1A78}', '\u{1A79}',
    '\u{1A7A}', '\u{1A7B}', '\u{1A7C}', '\u{1B6B}', '\u{1B6D}', '\u{1B6E}', '\u{1B6F}',
    '\u{1B70}', '\u{1B71}', '\u{1B72}', '\u{1B73}', '\u{1CD0}', '\u{1CD1}', '\u{1CD2}',
    '\u{1CDA}', '\u{1CDB}', '\u{1CE0}', '\u{1DC0}', '\u{1DC1}', '\u{1DC3}', '\u{1DC4}',
    '\u{1DC5}', '\u{1DC6}', '\u{1DC7}', '\u{1DC8}', '\u{1DC9}', '\u{1DCB}', '\u{1DCC}',
    '\u{1DD1}', '\u{1DD2}', '\u{1DD3}', '\u{1DD4}', '\u{1DD5}', '\u{1DD6}', '\u{1DD7}',
    '\u{1DD8}', '\u{1DD9}', '\u{1DDA}', '\u{1DDB}', '\u{1DDC}', '\u{1DDD}', '\u{1DDE}',
    '\u{1DDF}', '\u{1DE0}', '\u{1DE1}', '\u{1DE2}', '\u{1DE3}', '\u{1DE4}', '\u{1DE5}',
    '\u{1DE6}', '\u{1DFE}', '\u{20D0}', '\u{20D1}', '\u{20D4}', '\u{20D5}', '\u{20D6}',
    '\u{20D7}', '\u{20DB}', '\u{20DC}', '\u{20E1}', '\u{20E7}', '\u{20E9}', '\u{20F0}',
    '\u{2CEF}', '\u{2CF0}', '\u{2CF1}', '\u{2DE0}', '\u{2DE1}', '\u{2DE2}', '\u{2DE3}',
    '\u{2DE4}', '\u{2DE5}', '\u{2DE6}', '\u{2DE7}', '\u{2DE8}', '\u{2DE9}', '\u{2DEA}',
    '\u{2DEB}', '\u{2DEC}', '\u{2DED}', '\u{2DEE}', '\u{2DEF}', '\u{2DF0}', '\u{2DF1}',
    '\u{2DF2}', '\u{2DF3}', '\u{2DF4}', '\u{2DF5}', '\u{2DF6}', '\u{2DF7}', '\u{2DF8}',
    '\u{2DF9}', '\u{2DFA}', '\u{2DFB}', '\u{2DFC}', '\u{2DFD}', '\u{2DFE}', '\u{2DFF}',
    '\u{A66F}', '\u{A67C}', '\u{A67D}', '\u{A6F0}', '\u{A6F1}', '\u{A8E0}', '\u{A8E1}',
    '\u{A8E2}', '\u{A8E3}', '\u{A8E4}', '\u{A8E5}', '\u{A8E6}', '\u{A8E7}', '\u{A8E8}',
    '\u{A8E9}', '\u{A8EA}', '\u{A8EB}', '\u{A8EC}', '\u{A8ED}', '\u{A8EE}', '\u{A8EF}',
    '\u{A8F0}', '\u{A8F1}', '\u{AAB0}', '\u{AAB2}', '\u{AAB3}', '\u{AAB7}', '\u{AAB8}',
    '\u{AABE}', '\u{AABF}', '\u{AAC1}', '\u{FE20}', '\u{FE21}', '\u{FE22}', '\u{FE23}',
    '\u{FE24}', '\u{FE25}', '\u{FE26}', '\u{10A0F}', '\u{10A38}', '\u{1D185}', '\u{1D186}',
    '\u{1D187}', '\u{1D188}', '\u{1D189}', '\u{1D1AA}', '\u{1D1AB}', '\u{1D1AC}', '\u{1D1AD}',
    '\u{1D242}', '\u{1D243}', '\u{1D244}',
];

pub(in crate::app) fn build_kitty_clear_sequence() -> &'static str {
    "\u{1b}_Ga=d,d=A,q=2\u{1b}\\"
}

pub(in crate::app) fn command_exists(program: &str) -> bool {
    if program.is_empty() {
        return false;
    }

    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        return executable_file_exists(program_path);
    }

    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|dir| executable_file_exists(&dir.join(program)))
    })
}

fn executable_file_exists(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

fn place_terminal_image_with_kitty_protocol(
    path: &Path,
    area: Rect,
    excluded: &[Rect],
) -> Result<Vec<u8>> {
    let id = kitty_image_id();
    let mut out = build_kitty_upload_sequence(path, id, area).into_bytes();
    out.extend(build_kitty_placeholder_sequence(id, area, excluded));
    Ok(out)
}

fn clear_terminal_images_with_kitty_protocol() -> Result<Vec<u8>> {
    Ok(build_kitty_clear_sequence().as_bytes().to_vec())
}

pub(in crate::app) fn encode_iterm_inline_payload(path: &Path) -> Option<Arc<str>> {
    let data = fs::read(path).ok()?;
    Some(Arc::<str>::from(
        base64::engine::general_purpose::STANDARD.encode(&data),
    ))
}

/// Overwrite every cell in `area` with a space colored with the panel background
/// so iTerm2 ghost pixels are erased without leaving black traces.
///
/// Using the exact panel color means ratatui's differential renderer can safely
/// skip those cells on the next draw — they already show the right color.
fn erase_cells(area: Rect) -> Vec<u8> {
    use ratatui::style::Color;
    let mut out = Vec::new();
    let blank_row = " ".repeat(usize::from(area.width));
    // Set background to the panel color so empty cells match the pane background.
    // Fall back to default-background reset if the theme returns a non-RGB value.
    match crate::ui::theme::palette().panel {
        Color::Rgb(r, g, b) => {
            let _ = write!(out, "\x1b[0;48;2;{r};{g};{b}m");
        }
        _ => {
            let _ = write!(out, "\x1b[0m");
        }
    }
    for row in 0..area.height {
        let _ = write!(
            out,
            "\x1b[{};{}H{}",
            area.y.saturating_add(1).saturating_add(row),
            area.x.saturating_add(1),
            blank_row
        );
    }
    let _ = write!(out, "\x1b[0m"); // restore default attributes
    out
}

fn push_unique_rect(rects: &mut Vec<Rect>, area: Rect) {
    if area.width == 0 || area.height == 0 || rects.contains(&area) {
        return;
    }
    rects.push(area);
}

fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = a.x.saturating_add(a.width).min(b.x.saturating_add(b.width));
    let bottom =
        a.y.saturating_add(a.height)
            .min(b.y.saturating_add(b.height));
    (right > left && bottom > top).then_some(Rect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    })
}

fn place_terminal_image_with_iterm_protocol(
    path: &Path,
    area: Rect,
    inline_payload: Option<&str>,
) -> Result<Vec<u8>> {
    let encoded = match inline_payload {
        Some(payload) => payload.to_string(),
        None => encode_iterm_inline_payload(path)
            .map(|payload| payload.to_string())
            .context("failed to encode iTerm inline image payload")?,
    };
    // Move cursor to the top-left cell of the placement area, then emit the
    // OSC 1337 sequence. `width` and `height` are in terminal cells.
    let seq = format!(
        "\x1b[{};{}H\x1b]1337;File=inline=1;width={};height={};preserveAspectRatio=1:{}\x07",
        area.y.saturating_add(1),
        area.x.saturating_add(1),
        area.width.max(1),
        area.height.max(1),
        encoded
    );
    Ok(seq.into_bytes())
}
