//! tmux transport and DCS-passthrough helpers for terminal images.

use anyhow::{Context, Result, ensure};
use ratatui::layout::Rect;
use std::process::{Command, Stdio};

/// Sixel placement and redraw behavior for the current terminal session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum SixelTransport {
    #[default]
    Direct,
    TmuxNative,
    TmuxPassthrough,
}

impl SixelTransport {
    pub(crate) fn supports_synchronized_updates(self) -> bool {
        // tmux redraws its cells when synchronization ends. Passthrough images
        // are absent from its screen state and get erased by that redraw.
        self != Self::TmuxPassthrough
    }
}

pub(crate) fn configure_sixel_transport() -> SixelTransport {
    if !inside_tmux() {
        return SixelTransport::Direct;
    }
    let transport = query_pane_format("#{sixel_support}|#{client_termfeatures}")
        .map(|reply| parse_sixel_transport(&reply))
        .unwrap_or(SixelTransport::TmuxPassthrough);
    if transport == SixelTransport::TmuxPassthrough {
        // 'all' also permits passthrough while tmux has a redraw pending.
        set_allow_passthrough("all");
    }
    transport
}

fn parse_sixel_transport(reply: &str) -> SixelTransport {
    match reply.trim().split_once('|') {
        Some(("1", features)) if features.split(',').any(|f| f == "sixel") => {
            SixelTransport::TmuxNative
        }
        _ => SixelTransport::TmuxPassthrough,
    }
}

const TMUX_MIN_INPUT_BUFFER_SIZE: usize = 1024 * 1024;

pub(super) fn ensure_sixel_input_capacity(payload_len: usize) -> Result<()> {
    let required = sixel_input_capacity(payload_len)
        .context("sixel payload exceeds tmux's input buffer capacity")?;
    if required <= TMUX_MIN_INPUT_BUFFER_SIZE {
        return Ok(());
    }
    // tmux grows its DCS input buffer in powers of two. Raise the server limit
    // only for an image that needs it, and never lower an existing larger limit.
    // Both native Sixel and passthrough DCS sequences are subject to this limit.
    let status = Command::new("tmux")
        .args([
            "if-shell",
            "-F",
            &format!("#{{e|<:#{{input-buffer-size}},{required}}}"),
            &format!("set-option -s input-buffer-size {required}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .context("failed to configure tmux's Sixel input buffer")?;
    ensure!(
        status.success(),
        "tmux rejected the Sixel input buffer size"
    );
    Ok(())
}

fn sixel_input_capacity(payload_len: usize) -> Option<usize> {
    let size = payload_len.checked_add(1)?.checked_next_power_of_two()?;
    // The tmux option is bounded by UINT_MAX, even on a 64-bit host.
    u32::try_from(size).ok()?;
    Some(size)
}

fn query_pane_format(format: &str) -> Option<String> {
    let mut command = Command::new("tmux");
    command.args(["display-message", "-p"]);
    if let Some(pane) = std::env::var_os("TMUX_PANE").filter(|p| !p.is_empty()) {
        command.arg("-t").arg(pane);
    }
    let output = command.arg(format).stdin(Stdio::null()).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TmuxPaneOrigin {
    pub(super) top: u16,
    pub(super) left: u16,
}

impl TmuxPaneOrigin {
    pub(super) fn absolute_cursor_for(self, area: Rect) -> (u32, u32) {
        (
            u32::from(self.top) + u32::from(area.y) + 1,
            u32::from(self.left) + u32::from(area.x) + 1,
        )
    }
}

pub(crate) fn inside_tmux() -> bool {
    std::env::var_os("TMUX").is_some()
}

pub(crate) fn enable_allow_passthrough() {
    if !inside_tmux() {
        return;
    }

    set_allow_passthrough("on");
}

fn set_allow_passthrough(value: &str) {
    let mut command = Command::new("tmux");
    command.args(allow_passthrough_args(
        std::env::var_os("TMUX_PANE").as_deref(),
        value,
    ));
    let _ = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn allow_passthrough_args(
    target_pane: Option<&std::ffi::OsStr>,
    value: &str,
) -> Vec<std::ffi::OsString> {
    let mut args = ["set-option", "-p", "-q"]
        .into_iter()
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>();
    if let Some(pane) = target_pane
        && !pane.is_empty()
    {
        args.push("-t".into());
        args.push(pane.into());
    }
    args.extend(["allow-passthrough", value].into_iter().map(Into::into));
    args
}

pub(super) fn query_pane_origin() -> Option<TmuxPaneOrigin> {
    if !inside_tmux() {
        return None;
    }
    let stdout =
        query_pane_format("#{pane_top},#{pane_left},#{?#{==:#{status-position},top},#{status},0}")?;
    parse_pane_origin(&stdout)
}

pub(super) fn parse_pane_origin(raw: &str) -> Option<TmuxPaneOrigin> {
    let trimmed = raw.trim();
    let mut fields = trimmed.split(',');
    let top: u16 = fields.next()?.parse().ok()?;
    let left = fields.next()?.parse().ok()?;
    // pane_top excludes the status lines above the window. Passthrough cursor
    // positions are relative to the outer terminal, so include those lines.
    let status_rows = match fields.next()? {
        "on" => 1,
        "off" => 0,
        rows => rows.parse::<u16>().ok()?,
    };
    if fields.next().is_some() {
        return None;
    }
    Some(TmuxPaneOrigin {
        top: top.checked_add(status_rows)?,
        left,
    })
}

/// Wrap a complete escape sequence for tmux passthrough. Every ESC byte inside
/// the payload must be doubled so tmux does not treat it as the outer DCS
/// terminator.
pub(super) fn wrap_sequence_for_tmux(seq: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seq.len() + seq.len() / 8 + 16);
    out.extend_from_slice(b"\x1bPtmux;");
    for &byte in seq {
        if byte == 0x1b {
            out.extend_from_slice(b"\x1b\x1b");
        } else {
            out.push(byte);
        }
    }
    out.extend_from_slice(b"\x1b\\");
    out
}

/// Wrap each Kitty APC sequence in the tmux DCS passthrough envelope when the
/// current process is running inside tmux. Non-APC bytes remain outside the
/// passthrough wrapper so the Kitty placeholder path still lets tmux lay out
/// its text placeholders normally.
pub(super) fn maybe_wrap_kitty_apcs_for_tmux(buf: Vec<u8>) -> Vec<u8> {
    if !inside_tmux() {
        return buf;
    }
    wrap_kitty_apcs_for_tmux(&buf)
}

fn wrap_kitty_apcs_for_tmux(buf: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len() + buf.len() / 4);
    let mut i = 0;
    while i < buf.len() {
        if buf.len() - i >= 3
            && &buf[i..i + 3] == b"\x1b_G"
            && let Some(rel) = buf[i + 3..].iter().position(|&b| b == 0x1b)
            && buf.get(i + 3 + rel + 1) == Some(&b'\\')
        {
            let body_end = i + 3 + rel;
            out.extend(wrap_sequence_for_tmux(&buf[i..body_end + 2]));
            i = body_end + 2;
            continue;
        }
        out.push(buf[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
#[path = "tests/tmux.rs"]
mod tests;
