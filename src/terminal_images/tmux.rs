//! tmux DCS-passthrough helpers for terminal image escape sequences.

use ratatui::layout::Rect;
use std::process::{Command, Stdio};

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

    let mut command = Command::new("tmux");
    command.args(allow_passthrough_args(
        std::env::var_os("TMUX_PANE").as_deref(),
    ));
    let _ = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn allow_passthrough_args(target_pane: Option<&std::ffi::OsStr>) -> Vec<std::ffi::OsString> {
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
    args.extend(["allow-passthrough", "on"].into_iter().map(Into::into));
    args
}

pub(super) fn query_pane_origin() -> Option<TmuxPaneOrigin> {
    if !inside_tmux() {
        return None;
    }
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#{pane_top},#{pane_left}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    parse_pane_origin(&stdout)
}

pub(super) fn parse_pane_origin(raw: &str) -> Option<TmuxPaneOrigin> {
    let trimmed = raw.trim();
    let (top, left) = trimmed.split_once(',')?;
    Some(TmuxPaneOrigin {
        top: top.parse().ok()?,
        left: left.parse().ok()?,
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
