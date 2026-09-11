use super::*;

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
