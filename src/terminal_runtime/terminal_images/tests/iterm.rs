use super::*;
use crate::terminal_runtime::terminal_images::{ImageProtocol, place_terminal_image};

#[test]
fn build_iterm_tmux_placement_wraps_absolute_cursor_and_inline_payload() {
    let output = String::from_utf8(build_iterm_tmux_placement_sequence(
        "YWJj",
        Rect {
            x: 10,
            y: 4,
            width: 8,
            height: 6,
        },
        TmuxPaneOrigin { top: 2, left: 3 },
    ))
    .expect("tmux iTerm placement should be utf8");

    assert!(output.starts_with("\x1bPtmux;\x1b\x1b[7;14H\x1b\x1b]1337;File=inline=1;"));
    assert!(output.contains("width=8"));
    assert!(output.contains("height=6"));
    assert!(output.contains("preserveAspectRatio=1:YWJj\x07"));
    assert!(output.ends_with("\x1b\\"));
    assert!(!output.contains("\x1b[5;11H"));
}

#[test]
fn iterm_inline_protocol_uses_preencoded_payload_without_reading_source() {
    let output = String::from_utf8(
        place_terminal_image(
            ImageProtocol::ItermInline,
            Path::new("/definitely/missing.png"),
            Rect {
                x: 2,
                y: 3,
                width: 10,
                height: 4,
            },
            &[],
            Some("YWJj"),
            None,
        )
        .expect("preencoded iterm payload should not require source file"),
    )
    .expect("iterm payload should be utf8");

    assert!(output.contains("]1337;File=inline=1;"));
    assert!(output.contains("YWJj"));
}
