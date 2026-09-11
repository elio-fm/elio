use anyhow::{Context, Result};
use base64::Engine as _;
use ratatui::layout::Rect;
use std::{
    fs::File,
    io::{Read, Write as _},
    path::Path,
};

use super::tmux::{self, TmuxPaneOrigin};

pub(super) fn place_terminal_image_with_konsole_protocol(
    path: &Path,
    area: Rect,
) -> Result<Vec<u8>> {
    let id = konsole_image_id();
    if tmux::inside_tmux() {
        let origin = tmux::query_pane_origin()
            .ok_or_else(|| anyhow::anyhow!("tmux pane origin unavailable"))?;
        return build_konsole_tmux_placement_sequence(path, id, area, origin);
    }
    build_konsole_placement_sequence(path, id, area)
}

pub(super) fn clear_terminal_images_with_konsole_protocol() -> Result<Vec<u8>> {
    let raw = build_konsole_clear_sequence(konsole_image_id())
        .as_bytes()
        .to_vec();
    if tmux::inside_tmux() {
        Ok(tmux::wrap_sequence_for_tmux(&raw))
    } else {
        Ok(raw)
    }
}

fn build_konsole_placement_sequence(path: &Path, id: u32, area: Rect) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let _ = write!(
        out,
        "\x1b[{};{}H",
        area.y.saturating_add(1),
        area.x.saturating_add(1)
    );
    for chunk in build_konsole_upload_chunks(path, id, area)? {
        out.extend(chunk);
    }
    Ok(out)
}

fn build_konsole_tmux_placement_sequence(
    path: &Path,
    id: u32,
    area: Rect,
    origin: TmuxPaneOrigin,
) -> Result<Vec<u8>> {
    let (row, col) = origin.absolute_cursor_for(area);
    let mut chunks = build_konsole_upload_chunks(path, id, area)?
        .into_iter()
        .peekable();
    let mut out = Vec::new();
    while let Some(chunk) = chunks.next() {
        if chunks.peek().is_some() {
            out.extend(tmux::wrap_sequence_for_tmux(&chunk));
        } else {
            // Direct placement uses the cursor position when the final m=0
            // chunk arrives, so move the outer terminal cursor in the same
            // passthrough envelope as that final chunk.
            let mut final_chunk = Vec::new();
            let _ = write!(final_chunk, "\x1b[{row};{col}H");
            final_chunk.extend(chunk);
            out.extend(tmux::wrap_sequence_for_tmux(&final_chunk));
        }
    }
    Ok(out)
}

#[cfg(test)]
fn build_konsole_upload_sequence(path: &Path, id: u32, area: Rect) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for chunk in build_konsole_upload_chunks(path, id, area)? {
        out.extend(chunk);
    }
    Ok(out)
}

fn build_konsole_upload_chunks(path: &Path, id: u32, area: Rect) -> Result<Vec<Vec<u8>>> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open Konsole preview image {}", path.display()))?;
    let total = file
        .metadata()
        .with_context(|| format!("failed to stat Konsole preview image {}", path.display()))?
        .len() as usize;
    if total == 0 {
        anyhow::bail!("Konsole preview image {} is empty", path.display());
    }

    let mut sent = 0usize;
    let mut chunk = vec![0u8; 3 * 4096 / 4];
    let mut chunks = Vec::new();
    while sent < total {
        let remaining = total.saturating_sub(sent);
        let chunk_len = remaining.min(chunk.len());
        file.read_exact(&mut chunk[..chunk_len])
            .with_context(|| format!("failed to read Konsole preview image {}", path.display()))?;
        sent += chunk_len;
        let more = sent < total;
        let payload = base64::engine::general_purpose::STANDARD.encode(&chunk[..chunk_len]);
        let mut out = Vec::new();
        if sent == chunk_len {
            write!(
                out,
                "\u{1b}_Ga=T,q=2,f=100,i={id},p=1,c={},r={},C=1,m={};{payload}\u{1b}\\",
                area.width.max(1),
                area.height.max(1),
                if more { 1 } else { 0 },
            )?;
        } else {
            write!(
                out,
                "\u{1b}_Gm={};{payload}\u{1b}\\",
                if more { 1 } else { 0 },
            )?;
        }
        chunks.push(out);
    }
    Ok(chunks)
}

fn build_konsole_clear_sequence(id: u32) -> String {
    format!("\u{1b}_Ga=d,d=I,i={id},p=1,q=2\u{1b}\\")
}

fn konsole_image_id() -> u32 {
    std::process::id() % (0xff_ffff + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
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
        std::env::temp_dir().join(format!("elio-konsole-inline-image-{label}-{unique}"))
    }

    fn write_test_raster_image(path: &Path, format: ImageFormat, width: u32, height: u32) {
        let image =
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([(x % 255) as u8, (y % 255) as u8, 0x80, 0xff])
            }));
        image
            .save_with_format(path, format)
            .expect("test raster image should save");
    }

    #[test]
    fn build_konsole_upload_sequence_uses_direct_placement_mode() {
        let root = temp_root("konsole-upload-sequence");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("demo.png");
        write_test_raster_image(&path, ImageFormat::Png, 24, 16);
        let payload = fs::read(&path).expect("png payload should exist");
        let id = 42_u32;
        let area = Rect {
            x: 10,
            y: 4,
            width: 30,
            height: 20,
        };

        let sequence = String::from_utf8(
            build_konsole_upload_sequence(&path, id, area)
                .expect("Konsole upload sequence should build"),
        )
        .expect("Konsole upload sequence should be utf8");

        assert!(sequence.starts_with("\u{1b}_G"));
        assert!(sequence.contains("a=T"));
        assert!(sequence.contains("q=2"));
        assert!(sequence.contains("f=100"));
        assert!(sequence.contains(&format!("i={id}")));
        assert!(sequence.contains("p=1"));
        assert!(sequence.contains("c=30"));
        assert!(sequence.contains("r=20"));
        assert!(sequence.contains("C=1"));
        assert!(sequence.contains("m=0"));
        assert!(!sequence.contains("U=1"));
        assert!(sequence.contains(&BASE64_STANDARD.encode(payload)));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn place_konsole_terminal_image_prefixes_cursor_move() {
        let root = temp_root("konsole-cursor-prefix");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("demo.png");
        write_test_raster_image(&path, ImageFormat::Png, 16, 16);

        let output = String::from_utf8(
            build_konsole_placement_sequence(
                &path,
                42,
                Rect {
                    x: 10,
                    y: 4,
                    width: 8,
                    height: 6,
                },
            )
            .expect("Konsole placement should build"),
        )
        .expect("Konsole placement should be utf8");

        assert!(output.starts_with("\x1b[5;11H\x1b_G"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn tmux_konsole_placement_wraps_absolute_cursor_and_upload_together() {
        let root = temp_root("konsole-tmux-placement");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("demo.png");
        write_test_raster_image(&path, ImageFormat::Png, 16, 16);

        let output = String::from_utf8(
            build_konsole_tmux_placement_sequence(
                &path,
                42,
                Rect {
                    x: 10,
                    y: 4,
                    width: 8,
                    height: 6,
                },
                TmuxPaneOrigin { top: 2, left: 3 },
            )
            .expect("Konsole tmux placement should build"),
        )
        .expect("Konsole tmux placement should be utf8");

        assert!(output.starts_with("\x1bPtmux;\x1b\x1b[7;14H\x1b\x1b_G"));
        assert!(output.ends_with("\x1b\x1b\\\x1b\\"));
        assert_eq!(output.matches("\x1bPtmux;").count(), 1);
        assert!(output.contains("a=T"));
        assert!(output.contains("c=8"));
        assert!(output.contains("r=6"));
        assert!(output.contains("C=1"));
        assert!(!output.contains("\x1b[5;11H"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn tmux_konsole_placement_wraps_each_chunk_and_positions_final_chunk() {
        let root = temp_root("konsole-tmux-chunked-placement");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("demo.png");
        let payload = (0..5000).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        fs::write(&path, payload).expect("failed to write chunked payload");

        let output = String::from_utf8(
            build_konsole_tmux_placement_sequence(
                &path,
                42,
                Rect {
                    x: 10,
                    y: 4,
                    width: 8,
                    height: 6,
                },
                TmuxPaneOrigin { top: 2, left: 3 },
            )
            .expect("Konsole tmux placement should build"),
        )
        .expect("Konsole tmux placement should be utf8");

        assert_eq!(output.matches("\x1bPtmux;").count(), 2);
        assert!(output.starts_with("\x1bPtmux;\x1b\x1b_Ga=T"));
        assert!(output.contains("m=1;"));
        assert!(output.contains("\x1b\x1b\\\x1b\\\x1bPtmux;\x1b\x1b[7;14H\x1b\x1b_Gm=0;"));
        assert!(!output.starts_with("\x1bPtmux;\x1b\x1b[7;14H\x1b\x1b_Ga=T"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn clear_konsole_uses_targeted_delete_sequence() {
        let sequence = String::from_utf8(
            build_konsole_clear_sequence(konsole_image_id())
                .as_bytes()
                .to_vec(),
        )
        .expect("Konsole clear sequence should be utf8");

        assert_eq!(
            sequence,
            format!("\u{1b}_Ga=d,d=I,i={},p=1,q=2\u{1b}\\", konsole_image_id())
        );
    }

    #[test]
    fn tmux_konsole_clear_wraps_delete_sequence() {
        let raw = build_konsole_clear_sequence(42);
        let wrapped = String::from_utf8(tmux::wrap_sequence_for_tmux(raw.as_bytes()))
            .expect("wrapped clear should be utf8");

        assert_eq!(
            wrapped,
            "\x1bPtmux;\x1b\x1b_Ga=d,d=I,i=42,p=1,q=2\x1b\x1b\\\x1b\\"
        );
    }
}
