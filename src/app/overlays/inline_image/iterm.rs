use anyhow::{Context, Result};
use base64::Engine as _;
use ratatui::{layout::Rect, style::Color};
use std::{fs, io::Write as _, path::Path, sync::Arc};

use crate::app::FrameState;

use super::geometry::intersect_rect;

pub(in crate::app) fn encode_iterm_inline_payload(path: &Path) -> Option<Arc<str>> {
    let data = fs::read(path).ok()?;
    Some(Arc::<str>::from(
        base64::engine::general_purpose::STANDARD.encode(&data),
    ))
}

pub(super) fn place_terminal_image_with_iterm_protocol(
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

/// Overwrite every cell in `area` with a space colored with the panel background
/// so ghost pixels are erased without leaving black traces.
///
/// Using the exact panel color means ratatui's differential renderer can safely
/// skip those cells on the next draw — they already show the right color.
pub(super) fn erase_cells(area: Rect) -> Vec<u8> {
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
    let _ = write!(out, "\x1b[0m");
    out
}

pub(super) fn expand_iterm_erase_area(frame_state: &FrameState, area: Rect) -> Rect {
    let safe_bounds = frame_state
        .preview_body_area
        .or(frame_state.preview_content_area)
        .unwrap_or(area);
    let Some(bounds) = frame_state.preview_panel.or(Some(safe_bounds)) else {
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
