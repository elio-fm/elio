use super::*;

#[test]
fn wrap_sequence_for_tmux_doubles_inner_escape_bytes() {
    let input = b"\x1b[7;14H\x1b_Ga=T;AAAA\x1b\\";
    let expected: &[u8] = b"\x1bPtmux;\x1b\x1b[7;14H\x1b\x1b_Ga=T;AAAA\x1b\x1b\\\x1b\\";
    assert_eq!(wrap_sequence_for_tmux(input), expected);
}

#[test]
fn wrap_kitty_apcs_for_tmux_envelopes_each_apc_and_leaves_csi_alone() {
    let input = b"\x1b_Ga=T,i=1,c=2,r=2;AAAA\x1b\\\x1b[5;10H\xf4\x8e\xbb\xae\x1b_Gm=0;BBBB\x1b\\";
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
fn allow_passthrough_args_target_current_pane_when_available() {
    assert_eq!(
        allow_passthrough_args(Some(std::ffi::OsStr::new("%7"))),
        vec![
            "set-option",
            "-p",
            "-q",
            "-t",
            "%7",
            "allow-passthrough",
            "on"
        ]
    );
}

#[test]
fn allow_passthrough_args_fall_back_to_implicit_target() {
    assert_eq!(
        allow_passthrough_args(None),
        vec!["set-option", "-p", "-q", "allow-passthrough", "on"]
    );
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
