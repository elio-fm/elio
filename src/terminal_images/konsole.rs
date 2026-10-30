use anyhow::{Context, Result};
use base64::Engine as _;
use ratatui::layout::Rect;
use std::{
    fs::File,
    io::{Read, Write as _},
    path::Path,
    sync::atomic::{AtomicU32, Ordering},
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

pub(super) fn place_item_details_icon_with_konsole_protocol(
    path: &Path,
    area: Rect,
    vertical_offset: Option<u32>,
) -> Result<(Vec<u8>, u32)> {
    static NEXT_ICON_ID: AtomicU32 = AtomicU32::new(0);
    let id = konsole_image_id()
        .wrapping_add(NEXT_ICON_ID.fetch_add(1, Ordering::Relaxed).wrapping_add(1))
        & 0x00ff_ffff;
    let id = if id == 0 { 1 } else { id };
    let bytes = if tmux::inside_tmux() {
        let origin = tmux::query_pane_origin()
            .ok_or_else(|| anyhow::anyhow!("tmux pane origin unavailable"))?;
        build_konsole_tmux_placement_sequence_with_offset(path, id, area, origin, vertical_offset)?
    } else {
        build_konsole_placement_sequence_with_offset(path, id, area, vertical_offset)?
    };
    Ok((bytes, id))
}

pub(super) fn clear_item_details_icon_with_konsole_protocol(id: u32) -> Vec<u8> {
    let raw = build_konsole_clear_sequence(id).into_bytes();
    if tmux::inside_tmux() {
        tmux::wrap_sequence_for_tmux(&raw)
    } else {
        raw
    }
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
    build_konsole_placement_sequence_with_offset(path, id, area, None)
}

fn build_konsole_placement_sequence_with_offset(
    path: &Path,
    id: u32,
    area: Rect,
    vertical_offset: Option<u32>,
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let _ = write!(
        out,
        "\x1b[{};{}H",
        area.y.saturating_add(1),
        area.x.saturating_add(1)
    );
    for chunk in build_konsole_upload_chunks_with_offset(path, id, area, vertical_offset)? {
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
    build_konsole_tmux_placement_sequence_with_offset(path, id, area, origin, None)
}

fn build_konsole_tmux_placement_sequence_with_offset(
    path: &Path,
    id: u32,
    area: Rect,
    origin: TmuxPaneOrigin,
    vertical_offset: Option<u32>,
) -> Result<Vec<u8>> {
    let (row, col) = origin.absolute_cursor_for(area);
    let mut chunks = build_konsole_upload_chunks_with_offset(path, id, area, vertical_offset)?
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

fn build_konsole_upload_chunks_with_offset(
    path: &Path,
    id: u32,
    area: Rect,
    vertical_offset: Option<u32>,
) -> Result<Vec<Vec<u8>>> {
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
            let vertical_offset = vertical_offset
                .filter(|offset| *offset > 0)
                .map(|offset| format!(",Y={offset}"))
                .unwrap_or_default();
            write!(
                out,
                "\u{1b}_Ga=T,q=2,f=100,i={id},p=1,c={},r={},C=1{vertical_offset},m={};{payload}\u{1b}\\",
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
#[path = "tests/konsole.rs"]
mod tests;
