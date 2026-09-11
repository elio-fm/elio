use anyhow::{Context, Result};
use base64::Engine as _;
use ratatui::{layout::Rect, style::Color};
use std::{fs, io::Write as _, path::Path, sync::Arc};

use super::tmux::{self, TmuxPaneOrigin};

pub(crate) fn encode_iterm_inline_payload(path: &Path) -> Option<Arc<str>> {
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
    if tmux::inside_tmux() {
        let origin = tmux::query_pane_origin()
            .ok_or_else(|| anyhow::anyhow!("tmux pane origin unavailable"))?;
        return Ok(build_iterm_tmux_placement_sequence(&encoded, area, origin));
    }
    Ok(build_iterm_placement_sequence(&encoded, area))
}

fn build_iterm_placement_sequence(encoded: &str, area: Rect) -> Vec<u8> {
    build_iterm_placement_sequence_at(
        encoded,
        area.y.saturating_add(1).into(),
        area.x.saturating_add(1).into(),
        area,
    )
}

fn build_iterm_tmux_placement_sequence(
    encoded: &str,
    area: Rect,
    origin: TmuxPaneOrigin,
) -> Vec<u8> {
    let (row, col) = origin.absolute_cursor_for(area);
    tmux::wrap_sequence_for_tmux(&build_iterm_placement_sequence_at(encoded, row, col, area))
}

fn build_iterm_placement_sequence_at(encoded: &str, row: u32, col: u32, area: Rect) -> Vec<u8> {
    // Move cursor to the top-left cell of the placement area, then emit the
    // OSC 1337 sequence. `width` and `height` are in terminal cells.
    format!(
        "\x1b[{};{}H\x1b]1337;File=inline=1;width={};height={};preserveAspectRatio=1:{}\x07",
        row,
        col,
        area.width.max(1),
        area.height.max(1),
        encoded
    )
    .into_bytes()
}

/// Overwrite every cell in `area` with a space colored with the panel background
/// so ghost pixels are erased without leaving black traces.
///
/// Using the exact panel color means ratatui's differential renderer can safely
/// skip those cells on the next draw — they already show the right color.
pub(crate) fn erase_cells(area: Rect) -> Vec<u8> {
    let mut out = Vec::new();
    let blank_row = " ".repeat(usize::from(area.width));
    // Set background to the panel color so empty cells match the pane background.
    // Fall back to default-background reset if the theme returns a non-RGB value.
    match crate::theme::palette().panel {
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

#[cfg(test)]
#[path = "tests/iterm.rs"]
mod tests;
