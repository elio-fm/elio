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
        parse_pane_origin("12,34,0\n"),
        Some(TmuxPaneOrigin { top: 12, left: 34 })
    );
}

#[test]
fn parse_pane_origin_rejects_bad_values() {
    assert_eq!(parse_pane_origin("12\n"), None);
    assert_eq!(parse_pane_origin("top,4,0\n"), None);
    assert_eq!(parse_pane_origin("4,left,0\n"), None);
}

#[test]
fn allow_passthrough_args_target_current_pane_when_available() {
    assert_eq!(
        allow_passthrough_args(Some(std::ffi::OsStr::new("%7")), "on"),
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
        allow_passthrough_args(None, "on"),
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

#[test]
fn native_sixel_requires_support_in_both_tmux_and_client() {
    assert_eq!(
        parse_sixel_transport("1|RGB,sixel,sync\n"),
        SixelTransport::TmuxNative
    );
    for reply in [
        "0|RGB,sixel",
        "1|RGB,sync",
        "1|",
        "|sixel",
        "",
        "1|not-sixel",
    ] {
        assert_eq!(
            parse_sixel_transport(reply),
            SixelTransport::TmuxPassthrough,
            "{reply}"
        );
    }
}

#[test]
fn passthrough_sixel_does_not_use_synchronized_redraws() {
    assert!(SixelTransport::Direct.supports_synchronized_updates());
    assert!(SixelTransport::TmuxNative.supports_synchronized_updates());
    assert!(!SixelTransport::TmuxPassthrough.supports_synchronized_updates());
}

#[test]
fn input_capacity_includes_terminator_and_tmux_allocation_rounding() {
    assert_eq!(sixel_input_capacity(1024 * 1024 - 1), Some(1024 * 1024));
    assert_eq!(sixel_input_capacity(1024 * 1024), Some(2 * 1024 * 1024));
    assert_eq!(sixel_input_capacity(1_200_035), Some(2 * 1024 * 1024));
    assert_eq!(sixel_input_capacity(usize::MAX), None);
    assert_eq!(sixel_input_capacity(u32::MAX as usize), None);
}

#[test]
fn sixel_passthrough_all_targets_only_the_current_pane() {
    assert_eq!(
        allow_passthrough_args(Some(std::ffi::OsStr::new("%9")), "all"),
        [
            "set-option",
            "-p",
            "-q",
            "-t",
            "%9",
            "allow-passthrough",
            "all"
        ]
    );
}

#[test]
fn pane_origin_includes_top_status_lines() {
    for (reply, top) in [("12,34,on", 13), ("12,34,off", 12), ("12,34,3", 15)] {
        assert_eq!(
            parse_pane_origin(reply),
            Some(TmuxPaneOrigin { top, left: 34 })
        );
    }
    for reply in ["12,34", "12,34,unknown", "65535,0,1", "0,0,1,2"] {
        assert_eq!(parse_pane_origin(reply), None, "{reply}");
    }
}
