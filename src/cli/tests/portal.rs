use super::{enable_with, parse_parent_process, routing_summary};
use elio::portal::terminal::{TerminalAdapter, classify_command, detect_with};
use elio::{ManagedPortalDesktop, PortalRoutingState};

#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
#[test]
fn portal_commands_are_rejected_on_unsupported_platforms() {
    for command in [
        super::PortalCommand::Enable,
        super::PortalCommand::Disable,
        super::PortalCommand::Status,
    ] {
        assert_eq!(
            super::execute(command).unwrap_err().to_string(),
            "error: `elio portal` is only supported on Linux and FreeBSD"
        );
    }
}

#[test]
fn routing_summary_lists_managed_desktops_compactly() {
    assert_eq!(
        routing_summary(PortalRoutingState::Managed(vec![
            ManagedPortalDesktop {
                name: "Hyprland".to_string(),
                current: true,
                desktop_specific: true,
            },
            ManagedPortalDesktop {
                name: "GNOME".to_string(),
                current: false,
                desktop_specific: true,
            },
        ])),
        vec![
            "Portal routing: enabled for 2 desktops",
            "  • Hyprland (current)",
            "  • GNOME",
        ],
    );
}

fn detect(env: &[(&str, &str)], ancestors: &[&str]) -> Option<TerminalAdapter> {
    let lookup = |name: &str| {
        env.iter()
            .find_map(|(key, value)| (*key == name).then(|| (*value).to_string()))
    };
    let ancestors = ancestors
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    detect_with(&lookup, &ancestors)
}

#[test]
fn terminal_specific_environment_markers_are_used_without_ancestry() {
    assert_eq!(
        detect(
            &[("KITTY_WINDOW_ID", "1"), ("TERM_PROGRAM", "WezTerm")],
            &[]
        ),
        Some(TerminalAdapter::Kitty)
    );
    assert_eq!(
        detect(&[("GNOME_TERMINAL_SCREEN", ":0")], &[]),
        Some(TerminalAdapter::Gnome)
    );
}

#[test]
fn term_program_and_term_classify_the_supported_set() {
    assert_eq!(
        detect(&[("TERM_PROGRAM", "ghostty")], &[]),
        Some(TerminalAdapter::Ghostty)
    );
    assert_eq!(
        detect(&[("TERM", "foot-extra")], &[]),
        Some(TerminalAdapter::Foot)
    );
    assert_eq!(
        detect(&[("TERM", "xterm-256color")], &[]),
        Some(TerminalAdapter::Xterm)
    );
    assert_eq!(
        detect(&[("TERM", "xterm-rio")], &[]),
        Some(TerminalAdapter::Rio)
    );
    assert_eq!(
        detect(&[("TERM", "xterm-256color")], &["rio"]),
        Some(TerminalAdapter::Rio)
    );
}

#[test]
fn nearest_recognized_parent_terminal_takes_priority() {
    for (command, expected) in [
        ("/usr/bin/lxterminal", TerminalAdapter::Lxterminal),
        ("/usr/bin/contour", TerminalAdapter::Contour),
    ] {
        assert_eq!(
            detect(&[("TERM", "xterm-256color")], &["bash", command]),
            Some(expected)
        );
        assert_eq!(
            detect(&[("TERM_PROGRAM", expected.name())], &[]),
            Some(expected)
        );
    }
    assert_eq!(
        detect(
            &[("TERM", "screen")],
            &["bash", "/usr/bin/gnome-terminal-server"]
        ),
        Some(TerminalAdapter::Gnome)
    );
    assert_eq!(
        detect(&[("KITTY_WINDOW_ID", "1")], &["bash", "/usr/bin/foot"]),
        Some(TerminalAdapter::Foot)
    );
}

#[test]
fn unknown_terminal_stays_unknown() {
    assert_eq!(detect(&[("TERM", "screen")], &["bash", "tmux"]), None);
    assert_eq!(
        classify_command("/usr/bin/wezterm-gui --class elio"),
        Some(TerminalAdapter::WezTerm)
    );
}

#[test]
fn enable_is_gated_to_linux_and_freebsd_without_running_detection_elsewhere() {
    for platform in ["macos", "windows", "openbsd"] {
        let error = enable_with(platform, || -> anyhow::Result<()> {
            panic!("unsupported platforms must not detect")
        })
        .unwrap_err()
        .to_string();
        assert_eq!(
            error,
            "error: `elio portal enable` is only supported on Linux and FreeBSD"
        );
    }

    assert_eq!(
        enable_with("linux", || Ok(TerminalAdapter::Konsole)).unwrap(),
        TerminalAdapter::Konsole
    );
    assert_eq!(
        enable_with("freebsd", || Ok(TerminalAdapter::Xterm)).unwrap(),
        TerminalAdapter::Xterm
    );
}

#[test]
fn parent_process_parser_accepts_only_complete_ps_records() {
    assert_eq!(
        parse_parent_process("42 /usr/bin/kitty\n"),
        Some(("42".to_string(), "/usr/bin/kitty".to_string()))
    );
    assert_eq!(parse_parent_process("42\n"), None);
    assert_eq!(parse_parent_process(""), None);
}
