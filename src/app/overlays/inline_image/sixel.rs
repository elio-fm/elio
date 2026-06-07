use anyhow::{Context, Result};
use color_quant::NeuQuant;
use image::{DynamicImage, GenericImageView, imageops};
use ratatui::layout::Rect;
use std::{
    collections::HashMap,
    io::Write as _,
    path::Path,
    process::{Command, Stdio},
    sync::Arc,
};

use super::{
    TerminalIdentity, TerminalWindowSize, area_pixel_size, fit_image_area,
    protocol::{command_exists, detect_terminal_identity},
    tmux::{self, TmuxPaneOrigin},
};

const SIXEL_COLOR_LIMIT_DEFAULT: usize = 256;
const SIXEL_COLOR_LIMIT_FOOT: usize = 64;
const SIXEL_NEUQUANT_SAMPLE_DEFAULT: i32 = 10;
const SIXEL_NEUQUANT_SAMPLE_FOOT: i32 = 20;

// ── public API ───────────────────────────────────────────────────────────────

/// Encode a Sixel DCS stream for the image at `path`, resized to fit within
/// `target_w × target_h` pixels (aspect-ratio preserving, Triangle filter).
///
/// The returned bytes start with `\x1bP` and end with `\x1b\\`.  No cursor-
/// positioning prefix is included — callers splice one in with
/// [`place_sixel_from_dcs`] so the same encoded buffer can be reused at
/// different screen positions.
pub(in crate::app) fn encode_sixel_dcs(
    path: &Path,
    target_w: u32,
    target_h: u32,
) -> Result<Arc<[u8]>> {
    let profile = sixel_encode_profile();
    if let Some(dcs) = encode_sixel_dcs_with_img2sixel(path, target_w, target_h, profile) {
        return Ok(dcs);
    }

    let img = image::ImageReader::open(path)
        .with_context(|| format!("failed to open sixel preview image {}", path.display()))?
        .decode()
        .with_context(|| format!("failed to decode sixel preview image {}", path.display()))?;

    encode_sixel_dcs_from_image(img, target_w, target_h, profile)
}

/// Prepend the cursor-positioning escape to a pre-encoded Sixel DCS buffer
/// and return the combined bytes ready to write to the terminal.
///
/// This is O(n) in the DCS buffer size due to the memory copy, but avoids
/// re-running the expensive encode for re-renders of the same image.
pub(in crate::app) fn place_sixel_from_dcs(dcs: &[u8], placement: Rect) -> Result<Vec<u8>> {
    if tmux::inside_tmux() {
        if detect_terminal_identity() == TerminalIdentity::WindowsTerminal {
            return Ok(build_sixel_tmux_native_placement_sequence(dcs, placement));
        }
        let origin = tmux::query_pane_origin()
            .ok_or_else(|| anyhow::anyhow!("tmux pane origin unavailable"))?;
        return Ok(build_sixel_tmux_placement_sequence(dcs, placement, origin));
    }
    Ok(build_sixel_placement_sequence(dcs, placement))
}

fn build_sixel_placement_sequence(dcs: &[u8], placement: Rect) -> Vec<u8> {
    build_sixel_placement_sequence_at(
        dcs,
        placement.y.saturating_add(1).into(),
        placement.x.saturating_add(1).into(),
    )
}

// Windows Terminal via WSL+tmux renders tmux passthrough Sixel incorrectly in
// the alternate screen. Let tmux consume the raw Sixel and render it through
// its native Sixel path instead.
fn build_sixel_tmux_native_placement_sequence(dcs: &[u8], placement: Rect) -> Vec<u8> {
    build_sixel_placement_sequence(dcs, placement)
}

fn build_sixel_tmux_placement_sequence(
    dcs: &[u8],
    placement: Rect,
    origin: TmuxPaneOrigin,
) -> Vec<u8> {
    let (row, col) = origin.absolute_cursor_for(placement);
    tmux::wrap_sequence_for_tmux(&build_sixel_placement_sequence_at(dcs, row, col))
}

fn build_sixel_placement_sequence_at(dcs: &[u8], row: u32, col: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(dcs.len() + 16);
    let _ = write!(out, "\x1b[{row};{col}H");
    out.extend_from_slice(dcs);
    out
}

/// Full pipeline: fit the image's aspect ratio into `area`, encode the Sixel
/// DCS stream, and return cursor-prefix + DCS ready to write to the terminal.
///
/// This is the uncached fallback path.  Call sites that can provide a cached
/// DCS buffer should use [`encode_sixel_dcs`] + [`place_sixel_from_dcs`]
/// directly to skip the expensive encode.
pub(super) fn place_terminal_image_with_sixel_protocol(
    path: &Path,
    area: Rect,
    window_size: TerminalWindowSize,
) -> Result<Vec<u8>> {
    let img = image::ImageReader::open(path)
        .with_context(|| format!("failed to open sixel preview image {}", path.display()))?
        .decode()
        .with_context(|| format!("failed to decode sixel preview image {}", path.display()))?;

    let (orig_w, orig_h) = img.dimensions();
    let aspect_ratio = orig_w as f32 / orig_h.max(1) as f32;
    let placement = fit_image_area(area, window_size, aspect_ratio);
    let (target_w, target_h) = area_pixel_size(placement, window_size);

    let dcs = encode_sixel_dcs_from_image(img, target_w, target_h, sixel_encode_profile())?;
    place_sixel_from_dcs(&dcs, placement)
}

/// No explicit clear primitive exists for Sixel — the next ratatui draw
/// overpaints stale cells, the same as for the iTerm2 protocol.
pub(super) fn clear_terminal_images_with_sixel_protocol() -> Result<Vec<u8>> {
    Ok(Vec::new())
}

// ── shared encode core ───────────────────────────────────────────────────────

/// Resize `img` to fit within `target_w × target_h`, composite over the panel
/// background, colour-quantise, and encode as a raw Sixel DCS byte stream
/// (no cursor prefix).
///
/// Shared by the public [`encode_sixel_dcs`] (which opens the file) and the
/// uncached [`place_terminal_image_with_sixel_protocol`] (which has already
/// decoded the image to read its dimensions).
fn encode_sixel_dcs_from_image(
    img: DynamicImage,
    target_w: u32,
    target_h: u32,
    profile: SixelEncodeProfile,
) -> Result<Arc<[u8]>> {
    // Triangle is ~5× faster than Lanczos3 and imperceptible at terminal
    // pixel densities.
    let img = img.resize(target_w, target_h, imageops::FilterType::Triangle);
    let (w, h) = img.dimensions();

    // Flatten RGBA and composite alpha over the panel background colour.
    let rgba = img.to_rgba8();
    let (bg_r, bg_g, bg_b) = panel_background();
    let flat_rgba: Vec<u8> = rgba
        .pixels()
        .flat_map(|p| {
            let [r, g, b, a] = p.0;
            let a32 = a as u32;
            let ia = 255 - a32;
            [
                ((r as u32 * a32 + bg_r as u32 * ia) / 255) as u8,
                ((g as u32 * a32 + bg_g as u32 * ia) / 255) as u8,
                ((b as u32 * a32 + bg_b as u32 * ia) / 255) as u8,
                255u8,
            ]
        })
        .collect();

    // Foot is noticeably slower than Kitty/iTerm because Sixel is a textual
    // pixel stream that the terminal must parse. Keep a modest color cap so
    // the payload stays reasonable, but let NeuQuant preserve more gradients.
    let nq = NeuQuant::new(profile.neuquant_sample, profile.color_limit, &flat_rgba);
    let color_map = nq.color_map_rgba();
    let palette: Vec<(u8, u8, u8)> = color_map.chunks(4).map(|c| (c[0], c[1], c[2])).collect();
    let indices: Vec<u8> = flat_rgba
        .chunks(4)
        .map(|px| nq.index_of(px) as u8)
        .collect();
    let (palette, indices) = compact_palette(palette, indices);

    encode_dcs_bytes(w as usize, h as usize, &palette, &indices)
}

#[derive(Clone, Copy)]
struct SixelEncodeProfile {
    color_limit: usize,
    neuquant_sample: i32,
}

fn sixel_encode_profile() -> SixelEncodeProfile {
    match detect_terminal_identity() {
        // Foot spends most of the time parsing the Sixel stream, so reducing
        // palette size helps more than preserving subtle gradients.
        TerminalIdentity::Foot => SixelEncodeProfile {
            color_limit: SIXEL_COLOR_LIMIT_FOOT,
            neuquant_sample: SIXEL_NEUQUANT_SAMPLE_FOOT,
        },
        _ => SixelEncodeProfile {
            color_limit: SIXEL_COLOR_LIMIT_DEFAULT,
            neuquant_sample: SIXEL_NEUQUANT_SAMPLE_DEFAULT,
        },
    }
}

// ── private helpers ───────────────────────────────────────────────────────────

fn panel_background() -> (u8, u8, u8) {
    match crate::ui::theme::palette().panel {
        ratatui::style::Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

fn encode_sixel_dcs_with_img2sixel(
    path: &Path,
    target_w: u32,
    target_h: u32,
    profile: SixelEncodeProfile,
) -> Option<Arc<[u8]>> {
    if !command_exists("img2sixel") {
        return None;
    }

    let (bg_r, bg_g, bg_b) = panel_background();
    let bgcolor = format!("#{bg_r:02x}{bg_g:02x}{bg_b:02x}");
    let output = Command::new("img2sixel")
        .arg("-w")
        .arg(target_w.max(1).to_string())
        .arg("-h")
        .arg(target_h.max(1).to_string())
        .arg("-o")
        .arg("-")
        .arg("-p")
        .arg(profile.color_limit.to_string())
        .arg("-E")
        .arg("size")
        .arg("-q")
        .arg("low")
        .arg("-B")
        .arg(bgcolor)
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = output.stdout;
    if !(stdout.starts_with(b"\x1bP") || stdout.starts_with(b"\x90")) {
        return None;
    }
    Some(Arc::from(stdout))
}

fn compact_palette(palette: Vec<(u8, u8, u8)>, indices: Vec<u8>) -> (Vec<(u8, u8, u8)>, Vec<u8>) {
    let mut remap = HashMap::new();
    let mut dense_palette = Vec::new();
    let mut dense_indices = Vec::with_capacity(indices.len());
    for index in indices {
        let mapped = match remap.get(&index) {
            Some(&mapped) => mapped,
            None => {
                let mapped = dense_palette.len() as u8;
                dense_palette.push(palette[index as usize]);
                remap.insert(index, mapped);
                mapped
            }
        };
        dense_indices.push(mapped);
    }
    (dense_palette, dense_indices)
}

/// Assemble the complete Sixel DCS stream body (no cursor prefix) and return
/// it as a reference-counted byte slice.
fn encode_dcs_bytes(
    w: usize,
    h: usize,
    palette: &[(u8, u8, u8)],
    indices: &[u8],
) -> Result<Arc<[u8]>> {
    let mut out = Vec::with_capacity(w.saturating_mul(h / 3).saturating_add(4096));

    // DCS  P0=0 (1:1 pixel aspect)  P1=1 (use colour 0 as background)  P2=0
    write!(out, "\x1bP0;1;0q")?;

    // Raster attributes: pixel aspect 1:1, full image dimensions.
    write!(out, "\"1;1;{w};{h}")?;

    // Colour definitions.  Sixel uses 0-100 percentages for each RGB channel.
    for (i, &(r, g, b)) in palette.iter().enumerate() {
        let rp = (r as u32 * 100 + 127) / 255;
        let gp = (g as u32 * 100 + 127) / 255;
        let bp = (b as u32 * 100 + 127) / 255;
        write!(out, "#{i};2;{rp};{gp};{bp}")?;
    }

    // Scratch buffer: color_rows[c * w + x] accumulates the raw 6-bit value
    // for palette entry c at column x within the current band.
    let mut color_rows = vec![0u8; palette.len() * w];
    let mut color_used = vec![false; palette.len()];

    let mut band_y = 0usize;
    while band_y < h {
        let band_h = (h - band_y).min(6);

        color_rows.fill(0);
        color_used.fill(false);

        for bit in 0..band_h {
            let row_start = (band_y + bit) * w;
            let row = &indices[row_start..row_start + w];
            for (x, &c) in row.iter().enumerate() {
                let c = c as usize;
                color_rows[c * w + x] |= 1 << bit;
                color_used[c] = true;
            }
        }

        // Emit one colour layer per used palette entry, separated by '$'
        // (Graphics Carriage Return) to replay the same band row.
        let mut first = true;
        for c in 0..palette.len() {
            if !color_used[c] {
                continue;
            }
            if !first {
                out.push(b'$');
            }
            first = false;
            write!(out, "#{c}")?;
            rle_encode_sixel_row(&mut out, &color_rows[c * w..(c + 1) * w])?;
        }

        // '-' advances to the next six-pixel band.
        out.push(b'-');
        band_y += 6;
    }

    // String Terminator ends the DCS sequence.
    write!(out, "\x1b\\")?;

    Ok(Arc::from(out.as_slice()))
}

fn rle_encode_sixel_row(out: &mut Vec<u8>, data: &[u8]) -> Result<()> {
    let Some(end) = data
        .iter()
        .rposition(|&value| value != 0)
        .map(|index| index + 1)
    else {
        return Ok(());
    };
    let mut i = 0;
    while i < end {
        let current = data[i];
        let mut run = 1usize;
        while i + run < end && data[i + run] == current && run < 32767 {
            run += 1;
        }
        let encoded = current + 63;
        if run >= 3 {
            write!(out, "!{run}{}", encoded as char)?;
        } else {
            for _ in 0..run {
                out.push(encoded);
            }
        }
        i += run;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
