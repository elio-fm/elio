use super::super::*;
use anyhow::{Context, Result};
use base64::Engine as _;
use crossterm::terminal;
use ratatui::layout::Rect;
use std::{
    env,
    fs::{self, File},
    io::{Read, Write as _},
    path::Path,
    process::Command,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct TerminalImageState {
    pub(super) protocol: ImageProtocol,
    pub(super) window: Option<TerminalWindowSize>,
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
        preview_log(format_args!(
            "  window={:?}",
            self.terminal_images.window
        ));
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

    pub(crate) fn present_preview_overlay(&mut self) -> Result<Vec<u8>> {
        if self.browser_wheel_burst_active() {
            return Ok(Vec::new());
        }

        let protocol = self.terminal_images.protocol;
        if protocol == ImageProtocol::None {
            preview_log("present_preview_overlay: no protocol → clear");
            return self.clear_preview_overlay();
        }

        let keep_stale_page_preview_overlay =
            self.keep_displayed_page_preview_overlay_while_pending();
        let mut out = Vec::new();
        if (self.static_image_overlay_displayed()
            && !self.displayed_static_image_matches_active()
            && !keep_stale_page_preview_overlay)
            || self.pdf_overlay_displayed() && !self.displayed_pdf_overlay_matches_active()
        {
            out.extend(self.clear_preview_overlay()?);
        }

        let static_state = self.present_static_image_overlay(protocol, &mut out)?;
        preview_log(format_args!(
            "present_preview_overlay: protocol={protocol:?} static={static_state:?} out_len={}",
            out.len()
        ));
        match static_state {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => return Ok(out),
            OverlayPresentState::NotRequested => {}
        }

        let pdf_state = self.present_pdf_overlay(protocol, &mut out)?;
        preview_log(format_args!(
            "present_preview_overlay: pdf={pdf_state:?} out_len={}",
            out.len()
        ));
        match pdf_state {
            OverlayPresentState::Displayed | OverlayPresentState::Waiting => return Ok(out),
            OverlayPresentState::NotRequested => {}
        }

        let visual_state = self.present_preview_visual_overlay(protocol, &mut out)?;
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

    pub(crate) fn clear_preview_overlay(&mut self) -> Result<Vec<u8>> {
        if !self.static_image_overlay_displayed() && !self.pdf_overlay_displayed() {
            return Ok(Vec::new());
        }
        let mut bytes = clear_terminal_images(self.terminal_images.protocol)
            .context("failed to clear preview overlay")?;
        // iTerm2 has no delete primitive — overwrite the last displayed area
        // with blank cells so ghost pixels don't show through on the next draw.
        if self.terminal_images.protocol == ImageProtocol::ItermInline {
            // Use the full pane rect for erasure so pixels from images of any
            // aspect ratio are fully cleared, not just the fitted placement rect.
            let area = self.displayed_static_image_pane_area()
                .or_else(|| self.displayed_pdf_overlay_area());
            if let Some(area) = area {
                bytes.extend(erase_cells(area));
            }
        }
        self.clear_displayed_static_image();
        self.clear_displayed_pdf_overlay();
        Ok(bytes)
    }

    pub(crate) fn preview_uses_image_overlay(&self) -> bool {
        self.displayed_static_image_replaces_preview()
            || self.displayed_pdf_overlay_matches_active()
    }

    pub(crate) fn preview_prefers_image_surface(&self) -> bool {
        self.preview_prefers_static_image_surface() || self.preview_prefers_pdf_surface()
    }

    fn refresh_terminal_image_window_size(&mut self) {
        self.terminal_images.window =
            (self.terminal_images.protocol != ImageProtocol::None)
                .then(query_terminal_window_size)
                .flatten();
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
) -> Result<Vec<u8>> {
    match protocol {
        ImageProtocol::KittyGraphics => place_terminal_image_with_kitty_protocol(path, area),
        ImageProtocol::ItermInline => place_terminal_image_with_iterm_protocol(path, area),
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

pub(in crate::app) fn build_kitty_display_sequence(path: &Path, area: Rect) -> String {
    let payload =
        base64::engine::general_purpose::STANDARD.encode(path.as_os_str().as_encoded_bytes());
    format!(
        "\u{1b}[{};{}H\u{1b}_Ga=T,q=2,f=100,t=f,c={},r={},C=1;{}\u{1b}\\",
        area.y.saturating_add(1),
        area.x.saturating_add(1),
        area.width.max(1),
        area.height.max(1),
        payload
    )
}

pub(in crate::app) fn build_kitty_clear_sequence() -> &'static str {
    "\u{1b}_Ga=d,d=A,q=2\u{1b}\\"
}

pub(in crate::app) fn command_exists(program: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {program} >/dev/null 2>&1"))
        .status()
        .is_ok_and(|status| status.success())
}

fn place_terminal_image_with_kitty_protocol(path: &Path, area: Rect) -> Result<Vec<u8>> {
    Ok(build_kitty_display_sequence(path, area).into_bytes())
}

fn clear_terminal_images_with_kitty_protocol() -> Result<Vec<u8>> {
    Ok(build_kitty_clear_sequence().as_bytes().to_vec())
}

/// Overwrite every cell in `area` with a space so iTerm2 ghost pixels are
/// erased before ratatui redraws the region with text content.
/// Emits SGR reset first to avoid inheriting any active foreground/background.
fn erase_cells(area: Rect) -> Vec<u8> {
    let mut out = Vec::new();
    let blank_row = " ".repeat(usize::from(area.width));
    let _ = write!(out, "\x1b[0m"); // reset attributes
    for row in 0..area.height {
        let _ = write!(
            out,
            "\x1b[{};{}H{}",
            area.y.saturating_add(1).saturating_add(row),
            area.x.saturating_add(1),
            blank_row
        );
    }
    out
}

fn place_terminal_image_with_iterm_protocol(path: &Path, area: Rect) -> Result<Vec<u8>> {
    let data = fs::read(path)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
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
