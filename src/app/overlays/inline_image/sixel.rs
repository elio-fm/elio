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
pub(in crate::app) fn place_sixel_from_dcs(dcs: &[u8], placement: Rect) -> Vec<u8> {
    let mut out = Vec::with_capacity(dcs.len() + 16);
    let _ = write!(
        out,
        "\x1b[{};{}H",
        placement.y.saturating_add(1),
        placement.x.saturating_add(1)
    );
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
    Ok(place_sixel_from_dcs(&dcs, placement))
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
mod tests {
    use super::*;
    use image::ImageFormat;
    use std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-inline-image-{label}-{unique}"))
    }

    fn write_test_png(path: &Path, width: u32, height: u32) {
        let img =
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([(x % 255) as u8, (y % 255) as u8, 0x80, 0xff])
            }));
        img.save_with_format(path, ImageFormat::Png)
            .expect("test png should save");
    }

    fn test_window_size() -> TerminalWindowSize {
        TerminalWindowSize {
            cells_width: 200,
            cells_height: 50,
            pixels_width: 1600,
            pixels_height: 800,
        }
    }

    fn terminal_env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct TerminalEnvGuard {
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl TerminalEnvGuard {
        fn isolate() -> Self {
            const VARS: &[&str] = &[
                "TERM",
                "TERM_PROGRAM",
                "KITTY_WINDOW_ID",
                "WARP_SESSION_ID",
                "ALACRITTY_SOCKET",
                "WT_SESSION",
            ];

            let saved = VARS
                .iter()
                .map(|&var| (var, std::env::var_os(var)))
                .collect::<Vec<_>>();
            unsafe {
                for &var in VARS {
                    std::env::remove_var(var);
                }
            }
            Self { saved }
        }
    }

    impl Drop for TerminalEnvGuard {
        fn drop(&mut self) {
            unsafe {
                for (var, value) in self.saved.drain(..) {
                    match value {
                        Some(value) => std::env::set_var(var, value),
                        None => std::env::remove_var(var),
                    }
                }
            }
        }
    }

    #[test]
    fn sixel_sequence_has_dcs_preamble_and_terminator() {
        let root = temp_root("sixel-preamble");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("demo.png");
        write_test_png(&path, 24, 16);

        let area = Rect {
            x: 5,
            y: 2,
            width: 20,
            height: 10,
        };
        let output = String::from_utf8(
            place_terminal_image_with_sixel_protocol(&path, area, test_window_size())
                .expect("sixel encoding should succeed"),
        )
        .expect("sixel output should be valid utf8");

        assert!(output.contains("\x1bP"), "missing DCS introducer");
        assert!(output.ends_with("\x1b\\"), "missing String Terminator");
        assert!(output.contains("q"), "missing 'q' Sixel introducer");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn sixel_sequence_positions_cursor_at_area_top_left() {
        let root = temp_root("sixel-cursor");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("demo.png");
        write_test_png(&path, 24, 16);

        let area = Rect {
            x: 3,
            y: 7,
            width: 20,
            height: 10,
        };
        let output = String::from_utf8(
            place_terminal_image_with_sixel_protocol(&path, area, test_window_size())
                .expect("sixel encoding should succeed"),
        )
        .expect("sixel output should be valid utf8");

        // Image aspect 3:2, area 20×10 cells at 8×16 px/cell (160×160 px).
        // Fit: width-constrained → 160×107 px → 20×7 cells.
        // x offset: (20-20)/2=0 → col 4 (3+1).
        // y offset: (10-7)/2=1 → row 9 (7+1+1).
        assert!(
            output.starts_with("\x1b[9;4H"),
            "cursor positioning missing or wrong"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn sixel_sequence_contains_raster_attributes_and_palette() {
        let root = temp_root("sixel-raster");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("demo.png");
        write_test_png(&path, 24, 16);

        let area = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 5,
        };
        let output = String::from_utf8(
            place_terminal_image_with_sixel_protocol(&path, area, test_window_size())
                .expect("sixel encoding should succeed"),
        )
        .expect("sixel output should be valid utf8");

        // Raster attributes ("1;1;...)
        assert!(output.contains("\"1;1;"), "missing raster attributes");
        // Palette entries (#0;2;...)
        assert!(output.contains("#0;2;"), "missing palette definitions");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn clear_sixel_returns_empty() {
        let bytes =
            clear_terminal_images_with_sixel_protocol().expect("sixel clear should not fail");
        assert!(bytes.is_empty(), "sixel clear should return empty bytes");
    }

    #[test]
    fn rle_encode_sixel_row_compresses_runs_of_three_or_more() {
        let mut out = Vec::new();
        rle_encode_sixel_row(&mut out, &[0, 0, 0, 0, 32]).expect("rle should succeed");
        let s = String::from_utf8(out).expect("rle output should be utf8");
        // Four '?' → !4?, then one '_'
        assert!(s.starts_with("!4?"), "expected RLE for 4x '?', got: {s}");
        assert!(s.ends_with('_'), "expected trailing '_', got: {s}");
    }

    #[test]
    fn rle_encode_sixel_row_emits_short_runs_verbatim() {
        let mut out = Vec::new();
        rle_encode_sixel_row(&mut out, &[0, 32]).expect("rle should succeed");
        let s = String::from_utf8(out).expect("rle output should be utf8");
        assert_eq!(s, "?_", "two-byte run should be verbatim, got: {s}");
    }

    #[test]
    fn encode_sixel_dcs_returns_dcs_without_cursor_prefix() {
        let root = temp_root("sixel-dcs-no-cursor");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("demo.png");
        write_test_png(&path, 24, 16);

        let dcs = encode_sixel_dcs(&path, 80, 48).expect("encode_sixel_dcs should succeed");
        let s = String::from_utf8(dcs.to_vec()).expect("sixel dcs should be valid utf8");

        assert!(
            s.starts_with("\x1bP"),
            "dcs should start with DCS introducer, not cursor move"
        );
        assert!(s.ends_with("\x1b\\"), "missing String Terminator");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn sixel_encode_profile_uses_aggressive_settings_for_foot() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();
        unsafe {
            std::env::set_var("TERM", "foot");
        }

        let profile = sixel_encode_profile();

        assert_eq!(profile.color_limit, SIXEL_COLOR_LIMIT_FOOT);
        assert_eq!(profile.neuquant_sample, SIXEL_NEUQUANT_SAMPLE_FOOT);
    }

    #[test]
    fn sixel_encode_profile_keeps_full_palette_elsewhere() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();
        unsafe {
            std::env::set_var("TERM", "xterm-kitty");
        }

        let profile = sixel_encode_profile();

        assert_eq!(profile.color_limit, SIXEL_COLOR_LIMIT_DEFAULT);
        assert_eq!(profile.neuquant_sample, SIXEL_NEUQUANT_SAMPLE_DEFAULT);
    }

    #[test]
    fn place_sixel_from_dcs_prepends_cursor_move() {
        let dcs = b"\x1bP0;1;0q\"1;1;8;8#0;2;0;0;0\x1b\\";
        let placement = Rect {
            x: 4,
            y: 2,
            width: 1,
            height: 1,
        };
        let out = place_sixel_from_dcs(dcs, placement);
        let s = String::from_utf8(out).expect("output should be valid utf8");

        assert!(
            s.starts_with("\x1b[3;5H"),
            "expected cursor move to row 3 col 5, got: {s}"
        );
        assert!(s.contains("\x1bP"), "DCS stream should follow cursor move");
    }

    #[test]
    fn compact_palette_removes_unused_entries_and_reindexes_pixels() {
        let palette = vec![(1, 2, 3), (4, 5, 6), (7, 8, 9)];
        let indices = vec![2, 2, 0, 2, 0];

        let (dense_palette, dense_indices) = compact_palette(palette, indices);

        assert_eq!(dense_palette, vec![(7, 8, 9), (1, 2, 3)]);
        assert_eq!(dense_indices, vec![0, 0, 1, 0, 1]);
    }

    #[test]
    fn rle_encode_sixel_row_trims_trailing_blank_columns() {
        let mut out = Vec::new();

        rle_encode_sixel_row(&mut out, &[0, 0, 1, 1, 0, 0]).expect("row encode should succeed");

        let s = String::from_utf8(out).expect("row encode should be utf8");
        assert_eq!(s, "??@@");
    }
}
