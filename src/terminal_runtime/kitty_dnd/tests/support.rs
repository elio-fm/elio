use super::*;

fn kitty_env() -> RuntimeEnv {
    RuntimeEnv {
        term: Some("xterm-kitty".to_string()),
        ..RuntimeEnv::default()
    }
}

#[test]
fn gates_to_kitty_047_or_newer() {
    assert!(
        detect_from_env(
            &kitty_env(),
            || parse_kitty_version("kitty 0.47.0"),
            || Some("local".to_string())
        )
        .is_enabled()
    );
    assert!(
        detect_from_env(
            &kitty_env(),
            || parse_kitty_version("kitty 0.47.4"),
            || Some("local".to_string())
        )
        .is_enabled()
    );
    assert!(
        !detect_from_env(
            &kitty_env(),
            || parse_kitty_version("kitty 0.46.2"),
            || Some("local".to_string())
        )
        .is_enabled()
    );
}

#[test]
fn uses_empty_machine_id_for_local_drag_out() {
    let runtime = detect_from_env(
        &kitty_env(),
        || parse_kitty_version("kitty 0.47.4"),
        || Some("host;bad\x1b\\".to_string()),
    );
    assert_eq!(runtime.drag_machine_id(), None);
}

#[test]
fn disables_without_queryable_version() {
    assert!(!detect_from_env(&kitty_env(), || None, || Some("local".to_string())).is_enabled());
}

#[test]
fn disables_inside_mux_or_ssh() {
    let mut env = kitty_env();
    env.tmux = true;
    assert!(
        !detect_from_env(
            &env,
            || parse_kitty_version("kitty 0.47.4"),
            || Some("local".to_string())
        )
        .is_enabled()
    );

    let mut env = kitty_env();
    env.zellij = true;
    assert!(
        !detect_from_env(
            &env,
            || parse_kitty_version("kitty 0.47.4"),
            || Some("local".to_string())
        )
        .is_enabled()
    );

    let mut env = kitty_env();
    env.ssh_connection = true;
    assert!(
        !detect_from_env(
            &env,
            || parse_kitty_version("kitty 0.47.4"),
            || Some("local".to_string())
        )
        .is_enabled()
    );

    let mut env = kitty_env();
    env.ssh_tty = true;
    assert!(
        !detect_from_env(
            &env,
            || parse_kitty_version("kitty 0.47.4"),
            || Some("local".to_string())
        )
        .is_enabled()
    );
}

#[test]
fn disables_non_kitty_terminals() {
    let env = RuntimeEnv {
        term: Some("xterm-ghostty".to_string()),
        ..RuntimeEnv::default()
    };
    assert!(
        !detect_from_env(
            &env,
            || parse_kitty_version("kitty 0.47.4"),
            || Some("local".to_string())
        )
        .is_enabled()
    );
}

#[test]
fn parses_common_version_outputs() {
    assert_eq!(
        parse_kitty_version("kitty 0.47.4 created by Kovid Goyal"),
        Some(KittyVersion {
            major: 0,
            minor: 47,
            patch: 4
        })
    );
    assert_eq!(
        parse_kitty_version("kitty 0.48.0-alpha"),
        Some(KittyVersion {
            major: 0,
            minor: 48,
            patch: 0
        })
    );
}
