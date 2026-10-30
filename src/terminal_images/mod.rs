mod geometry;
mod iterm;
mod kitty;
mod konsole;
mod protocol;
mod sixel;
mod tmux;
mod window;

use anyhow::Result;
use ratatui::layout::Rect;
use std::{env, io::Write as _, path::Path};

pub(crate) use self::geometry::{
    area_pixel_size, fit_image_area, fit_image_pixels, intersect_rect, push_unique_rect,
    read_png_dimensions,
};
pub(crate) use self::iterm::{encode_iterm_inline_payload, erase_cells};
pub(crate) use self::protocol::{
    command_exists, detect_terminal_identity, pdf_preview_tools_available, select_image_protocol,
};
pub(crate) use self::sixel::{encode_sixel_dcs, place_sixel_from_dcs};
pub(crate) use self::tmux::{
    SixelTransport, configure_sixel_transport, enable_allow_passthrough, inside_tmux,
};
pub(crate) use self::window::query_terminal_window_size;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum TerminalIdentity {
    Kitty,
    Ghostty,
    Rio,
    Warp,
    WezTerm,
    ITerm2,
    Konsole,
    Foot,
    WindowsTerminal,
    Alacritty,
    #[default]
    Other,
}

/// The wire protocol used to render images in the terminal preview pane.
/// Kept separate from `TerminalIdentity` so that multiple terminals can share
/// the same protocol without coupling detection logic to rendering logic.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ImageProtocol {
    /// Kitty Graphics Protocol (APC `\x1b_G…\x1b\\`) using the Unicode
    /// placeholder extension. Used by Kitty, Ghostty, and Rio.
    KittyGraphics,
    /// Direct-placement variant of the Kitty Graphics Protocol: same APC wire
    /// format, but without the Unicode placeholder extension. Images are
    /// positioned with an explicit CSI cursor move and need explicit delete
    /// commands. Used by Konsole and Warp.
    KittyDirectGraphics,
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
    /// Sixel). These protocols require a pre-draw cell-erase pass before
    /// ratatui can safely overpaint stale image content.
    pub(crate) fn is_raster(self) -> bool {
        matches!(self, Self::ItermInline | Self::Sixel)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TerminalWindowSize {
    pub(crate) cells_width: u16,
    pub(crate) cells_height: u16,
    pub(crate) pixels_width: u32,
    pub(crate) pixels_height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RenderedImageDimensions {
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
}

/// Write a line to `<temp>/elio-preview.log` when `ELIO_DEBUG_PREVIEW` is set.
pub(crate) fn preview_log(msg: impl std::fmt::Display) {
    if env::var_os("ELIO_DEBUG_PREVIEW").is_none() {
        return;
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join("elio-preview.log"))
        .and_then(|mut file| writeln!(file, "{msg}"));
}

pub(crate) fn place_terminal_image(
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
        ImageProtocol::KittyDirectGraphics => {
            konsole::place_terminal_image_with_konsole_protocol(path, area)
        }
        ImageProtocol::ItermInline => {
            iterm::place_terminal_image_with_iterm_protocol(path, area, inline_payload)
        }
        ImageProtocol::Sixel => {
            let window_size = window_size.ok_or_else(|| {
                anyhow::anyhow!("sixel protocol requires terminal window size, but none available")
            })?;
            sixel::place_terminal_image_with_sixel_protocol(path, area, window_size)
        }
        ImageProtocol::None => Ok(Vec::new()),
    }
}

pub(crate) fn place_item_details_icon(
    protocol: ImageProtocol,
    path: &Path,
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> Result<(Vec<u8>, Option<u32>)> {
    match protocol {
        ImageProtocol::KittyGraphics => {
            let (bytes, id) = kitty::place_item_details_icon_with_kitty_protocol(path, area)?;
            Ok((bytes, Some(id)))
        }
        ImageProtocol::KittyDirectGraphics => {
            let icon_area = direct_icon_aspect_area(path, area, window_size).unwrap_or(area);
            let (bytes, id) =
                konsole::place_item_details_icon_with_konsole_protocol(path, icon_area, None)?;
            Ok((bytes, Some(id)))
        }
        ImageProtocol::Sixel => {
            let window_size = window_size.ok_or_else(|| {
                anyhow::anyhow!("sixel protocol requires terminal window size, but none available")
            })?;
            Ok((
                sixel::place_item_details_icon_with_sixel_protocol(path, area, window_size)?,
                None,
            ))
        }
        _ => Ok((
            place_terminal_image(protocol, path, area, &[], None, window_size)?,
            None,
        )),
    }
}

fn direct_icon_aspect_area(
    path: &Path,
    area: Rect,
    window_size: Option<TerminalWindowSize>,
) -> Option<Rect> {
    let window = window_size?;
    let (image_width, image_height) = image::image_dimensions(path).ok()?;
    if image_width == 0 || image_height == 0 || window.cells_width == 0 || window.cells_height == 0
    {
        return None;
    }
    let cell_width_px = window.pixels_width as f64 / f64::from(window.cells_width);
    let cell_height_px = window.pixels_height as f64 / f64::from(window.cells_height);
    if cell_width_px <= 0.0 || cell_height_px <= 0.0 {
        return None;
    }
    if area.width == 0 || area.height == 0 {
        return None;
    }

    // Kitty placement fills a cell rectangle. Since terminal cells are taller
    // than they are wide, using the full 4x3 icon slot stretches square glyphs
    // vertically in terminals such as Konsole. Pick the largest integer cell
    // rectangle whose pixel aspect ratio most closely matches the PNG.
    let target_aspect = f64::from(image_width) / f64::from(image_height);
    let mut best = None;
    for height in 1..=area.height {
        for width in 1..=area.width {
            let aspect = f64::from(width) * cell_width_px / (f64::from(height) * cell_height_px);
            let error = (aspect / target_aspect).ln().abs();
            let area_cells = u32::from(width) * u32::from(height);
            if best.is_none_or(|(best_error, best_area, _, _)| {
                error < best_error || (error == best_error && area_cells > best_area)
            }) {
                best = Some((error, area_cells, width, height));
            }
        }
    }
    let (_, _, width, height) = best?;
    Some(Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
        y: area
            .y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    })
}

pub(crate) fn clear_item_details_icon(
    protocol: ImageProtocol,
    image_id: Option<u32>,
) -> Result<Vec<u8>> {
    match (protocol, image_id) {
        (ImageProtocol::KittyGraphics, Some(id)) => {
            Ok(kitty::clear_item_details_icon_with_kitty_protocol(id))
        }
        (ImageProtocol::KittyDirectGraphics, Some(id)) => {
            Ok(konsole::clear_item_details_icon_with_konsole_protocol(id))
        }
        _ => clear_terminal_images(protocol),
    }
}

pub(crate) fn clear_terminal_images(protocol: ImageProtocol) -> Result<Vec<u8>> {
    match protocol {
        ImageProtocol::KittyGraphics => kitty::clear_terminal_images_with_kitty_protocol(),
        ImageProtocol::KittyDirectGraphics => {
            konsole::clear_terminal_images_with_konsole_protocol()
        }
        // iTerm2 has no clear primitive; ratatui overwrites the cell region.
        ImageProtocol::ItermInline | ImageProtocol::None => Ok(Vec::new()),
        // Sixel has no clear primitive either.
        ImageProtocol::Sixel => sixel::clear_terminal_images_with_sixel_protocol(),
    }
}
