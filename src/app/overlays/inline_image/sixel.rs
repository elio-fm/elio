use anyhow::{Context, Result};
use color_quant::NeuQuant;
use image::{GenericImageView, imageops};
use ratatui::layout::Rect;
use std::{io::Write as _, path::Path};

use super::TerminalWindowSize;

/// Render a PNG image as a Sixel graphics sequence positioned at `area`.
///
/// The image is resized to exactly fill the pixel footprint of `area`
/// (calculated from `window_size`), pre-composited over the panel background,
/// and colour-quantised to 256 entries via NeuQuant before encoding.
pub(super) fn place_terminal_image_with_sixel_protocol(
    path: &Path,
    area: Rect,
    window_size: TerminalWindowSize,
) -> Result<Vec<u8>> {
    let (target_w, target_h) = area_pixel_size(area, window_size);

    let img = image::ImageReader::open(path)
        .with_context(|| format!("failed to open sixel preview image {}", path.display()))?
        .decode()
        .with_context(|| format!("failed to decode sixel preview image {}", path.display()))?;

    // Resize to the pixel footprint of the cell area, preserving aspect ratio.
    let img = img.resize(target_w, target_h, imageops::FilterType::Lanczos3);
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

    // Quantise to 256 colours with NeuQuant (neural-network colour reducer).
    let nq = NeuQuant::new(10, 256, &flat_rgba);
    let color_map = nq.color_map_rgba();
    let palette: Vec<(u8, u8, u8)> = color_map.chunks(4).map(|c| (c[0], c[1], c[2])).collect();
    let indices: Vec<u8> = flat_rgba
        .chunks(4)
        .map(|px| nq.index_of(px) as u8)
        .collect();

    encode_sixel(area, w, h, &palette, &indices)
}

/// No explicit clear primitive exists for Sixel — the next ratatui draw
/// overpaints stale cells, the same as for the iTerm2 protocol.
pub(super) fn clear_terminal_images_with_sixel_protocol() -> Result<Vec<u8>> {
    Ok(Vec::new())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn area_pixel_size(area: Rect, window_size: TerminalWindowSize) -> (u32, u32) {
    let cell_px_w = window_size.pixels_width as f64 / window_size.cells_width.max(1) as f64;
    let cell_px_h = window_size.pixels_height as f64 / window_size.cells_height.max(1) as f64;
    let w = (area.width as f64 * cell_px_w).round() as u32;
    let h = (area.height as f64 * cell_px_h).round() as u32;
    (w.max(1), h.max(1))
}

fn panel_background() -> (u8, u8, u8) {
    match crate::ui::theme::palette().panel {
        ratatui::style::Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

/// Assemble the complete Sixel DCS stream and return it as raw bytes.
fn encode_sixel(
    area: Rect,
    image_w: u32,
    image_h: u32,
    palette: &[(u8, u8, u8)],
    indices: &[u8],
) -> Result<Vec<u8>> {
    let w = image_w as usize;
    let h = image_h as usize;
    let mut out = Vec::new();

    // Move cursor to the top-left corner of the placement area.
    write!(
        out,
        "\x1b[{};{}H",
        area.y.saturating_add(1),
        area.x.saturating_add(1)
    )?;

    // DCS  P0=0 (1:1 pixel aspect)  P1=1 (use colour 0 as background)  P2=0
    write!(out, "\x1bP0;1;0q")?;

    // Raster attributes: pixel aspect 1:1, full image dimensions.
    write!(out, "\"1;1;{image_w};{image_h}")?;

    // Colour definitions.  Sixel uses 0-100 percentages for each RGB channel.
    for (i, &(r, g, b)) in palette.iter().enumerate() {
        let rp = (r as u32 * 100 + 127) / 255;
        let gp = (g as u32 * 100 + 127) / 255;
        let bp = (b as u32 * 100 + 127) / 255;
        write!(out, "#{i};2;{rp};{gp};{bp}")?;
    }

    // Scratch buffer: color_rows[c * w + x] accumulates the raw 6-bit value
    // for palette entry c at column x within the current band.  Initialised to
    // 0 (no pixels set); converted to Sixel characters (+63) at output time.
    let mut color_rows = vec![0u8; 256 * w];
    let mut color_used = [false; 256];

    let mut band_y = 0usize;
    while band_y < h {
        let band_h = (h - band_y).min(6);

        // Reset scratch buffers for this band.
        color_rows.fill(0);
        color_used.fill(false);

        // Accumulate: for each pixel in the band, OR the appropriate bit into
        // its colour's column entry.
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
            // Convert raw bit values (0-63) to Sixel characters (63-126).
            let sixel_chars: Vec<u8> = color_rows[c * w..(c + 1) * w]
                .iter()
                .map(|&b| b + 63)
                .collect();
            rle_encode(&mut out, &sixel_chars)?;
        }

        // '-' advances to the next six-pixel band.
        out.push(b'-');
        band_y += 6;
    }

    // String Terminator ends the DCS sequence.
    write!(out, "\x1b\\")?;

    Ok(out)
}

/// RLE-encode a row of sixel characters.
///
/// Runs of three or more identical bytes are emitted as `!<count><char>`;
/// shorter runs are emitted verbatim.
fn rle_encode(out: &mut Vec<u8>, data: &[u8]) -> Result<()> {
    let mut i = 0;
    while i < data.len() {
        let current = data[i];
        let mut run = 1usize;
        while i + run < data.len() && data[i + run] == current && run < 32767 {
            run += 1;
        }
        if run >= 3 {
            write!(out, "!{run}{}", current as char)?;
        } else {
            for _ in 0..run {
                out.push(current);
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
        fs,
        path::{Path, PathBuf},
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

        // Cursor should be at row 8 (7+1), column 4 (3+1)
        assert!(
            output.starts_with("\x1b[8;4H"),
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
    fn rle_encode_compresses_runs_of_three_or_more() {
        let mut out = Vec::new();
        rle_encode(&mut out, &[63, 63, 63, 63, 95]).expect("rle should succeed");
        let s = String::from_utf8(out).expect("rle output should be utf8");
        // Four '?' → !4?, then one '_'
        assert!(s.starts_with("!4?"), "expected RLE for 4x '?', got: {s}");
        assert!(s.ends_with('_'), "expected trailing '_', got: {s}");
    }

    #[test]
    fn rle_encode_emits_short_runs_verbatim() {
        let mut out = Vec::new();
        rle_encode(&mut out, &[63, 95]).expect("rle should succeed");
        let s = String::from_utf8(out).expect("rle output should be utf8");
        assert_eq!(s, "?_", "two-byte run should be verbatim, got: {s}");
    }
}
