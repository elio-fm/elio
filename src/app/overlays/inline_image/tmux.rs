//! tmux DCS-passthrough helpers for terminal image escape sequences.

use ratatui::layout::Rect;
use std::process::Command;

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

pub(super) fn inside_tmux() -> bool {
    std::env::var_os("TMUX").is_some()
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
mod tests {
    use super::*;

    #[test]
    fn wrap_sequence_for_tmux_doubles_inner_escape_bytes() {
        let input = b"\x1b[7;14H\x1b_Ga=T;AAAA\x1b\\";
        let expected: &[u8] = b"\x1bPtmux;\x1b\x1b[7;14H\x1b\x1b_Ga=T;AAAA\x1b\x1b\\\x1b\\";
        assert_eq!(wrap_sequence_for_tmux(input), expected);
    }

    #[test]
    fn wrap_kitty_apcs_for_tmux_envelopes_each_apc_and_leaves_csi_alone() {
        let input =
            b"\x1b_Ga=T,i=1,c=2,r=2;AAAA\x1b\\\x1b[5;10H\xf4\x8e\xbb\xae\x1b_Gm=0;BBBB\x1b\\";
        let out = wrap_kitty_apcs_for_tmux(input);
        let expected: &[u8] = b"\x1bPtmux;\x1b\x1b_Ga=T,i=1,c=2,r=2;AAAA\x1b\x1b\\\x1b\\\x1b[5;10H\xf4\x8e\xbb\xae\x1bPtmux;\x1b\x1b_Gm=0;BBBB\x1b\x1b\\\x1b\\";
        assert_eq!(out, expected);
    }

    #[test]
    fn wrap_kitty_apcs_for_tmux_is_noop_without_apcs() {
        let input = b"\x1b[5;10Hhello\x1b[0m";
        let out = wrap_kitty_apcs_for_tmux(input);
        assert_eq!(out, input);
    }

    #[test]
    fn parse_pane_origin_reads_top_and_left() {
        assert_eq!(
            parse_pane_origin("12,34\n"),
            Some(TmuxPaneOrigin { top: 12, left: 34 })
        );
    }

    #[test]
    fn parse_pane_origin_rejects_bad_values() {
        assert_eq!(parse_pane_origin("12\n"), None);
        assert_eq!(parse_pane_origin("top,4\n"), None);
        assert_eq!(parse_pane_origin("4,left\n"), None);
    }

    #[test]
    fn pane_origin_calculates_one_based_absolute_cursor() {
        let origin = TmuxPaneOrigin { top: 2, left: 3 };
        let area = Rect {
            x: 10,
            y: 4,
            width: 8,
            height: 6,
        };
        assert_eq!(origin.absolute_cursor_for(area), (7, 14));
    }
}
