use super::{ImageProtocol, TerminalIdentity};
use std::{env, fs, path::Path, process::Command};

pub(super) fn pdf_preview_tools_available() -> bool {
    command_exists("pdfinfo") && command_exists("pdftocairo")
}

pub(in crate::app) fn detect_terminal_identity() -> TerminalIdentity {
    detect_terminal_identity_with(real_env_lookup, query_tmux_client_termname, query_tmux_env)
}

/// Inner detection logic with injectable lookups so tests can drive the tmux
/// fallback without spawning tmux. Direct process env wins; if it returns
/// `Other` and `$TMUX` is set, recover Kitty/Ghostty by consulting (in order)
/// the live tmux client `TERM`, the tmux session env, and the tmux server
/// global env. Other identities are intentionally not recovered via tmux —
/// adding them needs per-protocol passthrough verification.
fn detect_terminal_identity_with(
    env_lookup: impl Fn(&str) -> Option<String>,
    tmux_client_term: impl Fn() -> Option<String>,
    tmux_env_lookup: impl Fn(&str) -> Option<String>,
) -> TerminalIdentity {
    let identity = classify_from_env(&env_lookup);
    if identity != TerminalIdentity::Other {
        return identity;
    }
    if env_lookup("TMUX").is_none() {
        return identity;
    }

    if let Some(term) = tmux_client_term()
        && let Some(id) = classify_kitty_or_ghostty(&term, "", false)
    {
        return id;
    }

    let kitty_window_id_set = tmux_env_lookup("KITTY_WINDOW_ID").is_some();
    let term_program = tmux_env_lookup("TERM_PROGRAM").unwrap_or_default();
    let term = tmux_env_lookup("TERM").unwrap_or_default();
    if let Some(id) = classify_kitty_or_ghostty(&term, &term_program, kitty_window_id_set) {
        return id;
    }

    TerminalIdentity::Other
}

fn classify_from_env(env_lookup: &impl Fn(&str) -> Option<String>) -> TerminalIdentity {
    let term = env_lookup("TERM").unwrap_or_default().to_ascii_lowercase();
    let term_program = env_lookup("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kitty_window_id = env_lookup("KITTY_WINDOW_ID").is_some();
    let konsole_dbus = env_lookup("KONSOLE_DBUS_SESSION").is_some()
        || env_lookup("KONSOLE_DBUS_SERVICE").is_some()
        || env_lookup("KONSOLE_DBUS_WINDOW").is_some();

    if kitty_window_id || term.contains("xterm-kitty") || term_program == "kitty" {
        TerminalIdentity::Kitty
    } else if term.contains("ghostty") || term_program == "ghostty" {
        TerminalIdentity::Ghostty
    } else if term.contains("wezterm") || term_program == "wezterm" {
        TerminalIdentity::WezTerm
    } else if term_program.contains("warp") || env_lookup("WARP_SESSION_ID").is_some() {
        TerminalIdentity::Warp
    } else if term_program == "iterm.app" {
        TerminalIdentity::ITerm2
    } else if term.contains("alacritty")
        || term_program.contains("alacritty")
        || env_lookup("ALACRITTY_SOCKET").is_some()
    {
        TerminalIdentity::Alacritty
    } else if konsole_dbus {
        // Konsole exports D-Bus identifiers into child shells so scripts can
        // address the current window/session.
        TerminalIdentity::Konsole
    } else if term == "foot" || term == "foot-extra" {
        // Foot sets TERM=foot or TERM=foot-extra and supports Sixel natively.
        TerminalIdentity::Foot
    } else if env_lookup("WT_SESSION").is_some() {
        // WT_SESSION is a GUID set by Windows Terminal in every shell it hosts.
        // WT_PROFILE_ID is also available but WT_SESSION is the canonical marker.
        TerminalIdentity::WindowsTerminal
    } else {
        TerminalIdentity::Other
    }
}

/// Narrow Kitty/Ghostty match used by the tmux fallback. `kitty_window_id_set`
/// is presence-only (empty values still count as Kitty), matching the
/// `env::var_os(...).is_some()` semantics of the direct-env path.
fn classify_kitty_or_ghostty(
    term: &str,
    term_program: &str,
    kitty_window_id_set: bool,
) -> Option<TerminalIdentity> {
    let term = term.to_ascii_lowercase();
    let term_program = term_program.to_ascii_lowercase();
    if kitty_window_id_set || term.contains("xterm-kitty") || term_program == "kitty" {
        Some(TerminalIdentity::Kitty)
    } else if term.contains("ghostty") || term_program == "ghostty" {
        Some(TerminalIdentity::Ghostty)
    } else {
        None
    }
}

fn real_env_lookup(name: &str) -> Option<String> {
    // `to_string_lossy` keeps presence-only semantics for non-UTF-8 values:
    // they still produce `Some(_)` rather than collapsing to `None` like
    // `env::var` would.
    env::var_os(name).map(|v| v.to_string_lossy().into_owned())
}

/// Live `TERM` of the currently attached tmux client. Any failure (missing
/// tmux binary, detached session, non-zero exit, non-UTF-8 output) is silent
/// and returns `None` so the next fallback layer can try.
fn query_tmux_client_termname() -> Option<String> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#{client_termname}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Look up `name` in the tmux session environment, then in the server-global
/// environment. Each `tmux show-environment` invocation is fallible and
/// silent: any error or unset (`-NAME`) line yields `None` so the caller can
/// fall through to the next layer.
fn query_tmux_env(name: &str) -> Option<String> {
    show_environment_value(&[], name).or_else(|| show_environment_value(&["-g"], name))
}

fn show_environment_value(extra_args: &[&str], name: &str) -> Option<String> {
    let output = Command::new("tmux")
        .arg("show-environment")
        .args(extra_args)
        .arg(name)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    parse_show_environment_line(&stdout, name)
}

fn parse_show_environment_line(stdout: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    let unset = format!("-{name}");
    for line in stdout.lines() {
        let line = line.trim_end_matches('\r');
        if line == unset {
            return None;
        }
        if let Some(value) = line.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
    }
    None
}

pub(in crate::app) fn select_image_protocol(
    identity: TerminalIdentity,
    image_previews_override: bool,
) -> ImageProtocol {
    match identity {
        TerminalIdentity::Kitty => ImageProtocol::KittyGraphics,
        TerminalIdentity::Ghostty => ImageProtocol::KittyGraphics,
        // Warp implements the basic Kitty Graphics transmit-and-place protocol
        // but not the Unicode placeholder extension that the `KittyGraphics`
        // path emits (`U=1`). Route Warp through `KittyDirectGraphics`, which uses
        // direct CSI cursor placement and matches what Warp actually renders.
        TerminalIdentity::Warp => ImageProtocol::KittyDirectGraphics,
        TerminalIdentity::Konsole => ImageProtocol::KittyDirectGraphics,
        TerminalIdentity::WezTerm | TerminalIdentity::ITerm2 => ImageProtocol::ItermInline,
        // Foot and Windows Terminal ≥ 1.22 both support Sixel graphics.
        TerminalIdentity::Foot | TerminalIdentity::WindowsTerminal => ImageProtocol::Sixel,
        // ELIO_IMAGE_PREVIEWS=1 force-enables KittyGraphics on unrecognised terminals
        // for testing. Alacritty is excluded — it does not support image protocols.
        TerminalIdentity::Other if image_previews_override => ImageProtocol::KittyGraphics,
        TerminalIdentity::Alacritty | TerminalIdentity::Other => ImageProtocol::None,
    }
}

pub(in crate::app) fn command_exists(program: &str) -> bool {
    if program.is_empty() {
        return false;
    }

    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        return executable_file_exists(program_path)
            || cfg!(windows) && executable_file_exists(&program_path.with_extension("exe"));
    }

    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|dir| {
            let candidate = dir.join(program);
            executable_file_exists(&candidate)
                || cfg!(windows) && executable_file_exists(&candidate.with_extension("exe"))
        })
    })
}

fn executable_file_exists(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsString,
        fs,
        path::PathBuf,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-inline-image-{label}-{unique}"))
    }

    fn terminal_env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct TerminalEnvGuard {
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl TerminalEnvGuard {
        fn isolate() -> Self {
            const VARS: &[&str] = &[
                "TERM",
                "TERM_PROGRAM",
                "KITTY_WINDOW_ID",
                "WARP_SESSION_ID",
                "ALACRITTY_SOCKET",
                "WT_SESSION",
                "KONSOLE_DBUS_SESSION",
                "KONSOLE_DBUS_SERVICE",
                "KONSOLE_DBUS_WINDOW",
                "TMUX",
            ];

            let saved = VARS
                .iter()
                .map(|name| (*name, env::var_os(name)))
                .collect::<Vec<_>>();
            for name in VARS {
                unsafe {
                    env::remove_var(name);
                }
            }

            Self { saved }
        }
    }

    impl Drop for TerminalEnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.saved {
                if let Some(value) = value {
                    unsafe {
                        env::set_var(name, value);
                    }
                } else {
                    unsafe {
                        env::remove_var(name);
                    }
                }
            }
        }
    }

    #[test]
    fn detect_terminal_identity_recognizes_iterm2_term_program() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();

        unsafe {
            env::set_var("TERM_PROGRAM", "iTerm.app");
        }

        assert_eq!(detect_terminal_identity(), TerminalIdentity::ITerm2);
    }

    #[test]
    fn select_image_protocol_kitty_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Kitty, false),
            ImageProtocol::KittyGraphics
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Kitty, true),
            ImageProtocol::KittyGraphics
        );
    }

    #[test]
    fn select_image_protocol_ghostty_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Ghostty, false),
            ImageProtocol::KittyGraphics
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Ghostty, true),
            ImageProtocol::KittyGraphics
        );
    }

    #[test]
    fn select_image_protocol_wezterm_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::WezTerm, false),
            ImageProtocol::ItermInline
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::WezTerm, true),
            ImageProtocol::ItermInline
        );
    }

    #[test]
    fn select_image_protocol_iterm2_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::ITerm2, false),
            ImageProtocol::ItermInline
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::ITerm2, true),
            ImageProtocol::ItermInline
        );
    }

    #[test]
    fn detect_terminal_identity_recognizes_konsole_dbus_session() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();

        unsafe {
            env::set_var("KONSOLE_DBUS_SESSION", "/Sessions/7");
        }

        assert_eq!(detect_terminal_identity(), TerminalIdentity::Konsole);
    }

    #[test]
    fn detect_terminal_identity_recognizes_konsole_dbus_service() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();

        unsafe {
            env::set_var("KONSOLE_DBUS_SERVICE", "org.kde.konsole-12345");
        }

        assert_eq!(detect_terminal_identity(), TerminalIdentity::Konsole);
    }

    #[test]
    fn select_image_protocol_konsole_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Konsole, false),
            ImageProtocol::KittyDirectGraphics
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Konsole, true),
            ImageProtocol::KittyDirectGraphics
        );
    }

    #[test]
    fn wezterm_takes_precedence_over_inherited_konsole_markers() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();

        unsafe {
            env::set_var("TERM_PROGRAM", "WezTerm");
            env::set_var("KONSOLE_DBUS_SESSION", "/Sessions/9");
        }

        assert_eq!(detect_terminal_identity(), TerminalIdentity::WezTerm);
    }

    #[test]
    fn select_image_protocol_warp_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Warp, false),
            ImageProtocol::KittyDirectGraphics
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Warp, true),
            ImageProtocol::KittyDirectGraphics
        );
    }

    #[test]
    fn select_image_protocol_alacritty_disabled_and_other_override_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Alacritty, true),
            ImageProtocol::None
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Other, false),
            ImageProtocol::None
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Other, true),
            ImageProtocol::KittyGraphics
        );
    }

    #[test]
    fn detect_terminal_identity_recognizes_foot_term() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();

        unsafe {
            env::set_var("TERM", "foot");
        }

        assert_eq!(detect_terminal_identity(), TerminalIdentity::Foot);
    }

    #[test]
    fn detect_terminal_identity_recognizes_foot_extra_term() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();

        unsafe {
            env::set_var("TERM", "foot-extra");
        }

        assert_eq!(detect_terminal_identity(), TerminalIdentity::Foot);
    }

    #[test]
    fn select_image_protocol_foot_uses_sixel() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Foot, false),
            ImageProtocol::Sixel
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Foot, true),
            ImageProtocol::Sixel
        );
    }

    #[test]
    fn detect_terminal_identity_recognizes_windows_terminal_wt_session() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();

        unsafe {
            env::set_var("WT_SESSION", "00000000-0000-0000-0000-000000000001");
        }

        assert_eq!(
            detect_terminal_identity(),
            TerminalIdentity::WindowsTerminal
        );
    }

    #[test]
    fn select_image_protocol_windows_terminal_uses_sixel() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::WindowsTerminal, false),
            ImageProtocol::Sixel
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::WindowsTerminal, true),
            ImageProtocol::Sixel
        );
    }

    #[test]
    fn windows_terminal_takes_precedence_over_other_fallback() {
        let _lock = terminal_env_lock();
        let _guard = TerminalEnvGuard::isolate();

        // WT_SESSION present, no other terminal markers → WindowsTerminal
        unsafe {
            env::set_var("WT_SESSION", "some-guid");
        }

        assert_eq!(
            detect_terminal_identity(),
            TerminalIdentity::WindowsTerminal
        );
    }

    #[cfg(unix)]
    #[test]
    fn command_exists_checks_direct_executable_paths_without_shelling_out() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("command-exists-direct-path");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let executable = root.join("demo-tool");
        fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("failed to write test executable");

        let mut permissions = fs::metadata(&executable)
            .expect("test executable metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).expect("failed to mark test executable");

        assert!(command_exists(
            executable.to_str().expect("path should be valid utf-8")
        ));

        let not_executable = root.join("demo-data");
        fs::write(&not_executable, b"plain data").expect("failed to write plain file");
        assert!(!command_exists(
            not_executable.to_str().expect("path should be valid utf-8")
        ));
    }

    fn no_tmux_client_term() -> Option<String> {
        None
    }

    fn no_tmux_env_lookup(_: &str) -> Option<String> {
        None
    }

    fn tmux_set_only(env: &str) -> impl Fn(&str) -> Option<String> + '_ {
        move |name: &str| {
            if name == "TMUX" {
                Some(env.to_string())
            } else {
                None
            }
        }
    }

    #[test]
    fn detect_terminal_identity_inside_tmux_uses_client_termname_for_kitty() {
        let id = detect_terminal_identity_with(
            tmux_set_only("/tmp/tmux-1000/default,123,4"),
            || Some("xterm-kitty".to_string()),
            no_tmux_env_lookup,
        );
        assert_eq!(id, TerminalIdentity::Kitty);
    }

    #[test]
    fn detect_terminal_identity_inside_tmux_uses_client_termname_for_ghostty() {
        let id = detect_terminal_identity_with(
            tmux_set_only("/tmp/tmux-1000/default,123,4"),
            || Some("xterm-ghostty".to_string()),
            no_tmux_env_lookup,
        );
        assert_eq!(id, TerminalIdentity::Ghostty);
    }

    #[test]
    fn detect_terminal_identity_inside_tmux_falls_back_to_session_env_kitty_window_id_presence() {
        // Empty value still counts as Kitty: tmux records `KITTY_WINDOW_ID=` for
        // present-but-empty vars, which must mirror `env::var_os(...).is_some()`.
        let id = detect_terminal_identity_with(
            tmux_set_only("/tmp/tmux-1000/default,123,4"),
            no_tmux_client_term,
            |name| {
                if name == "KITTY_WINDOW_ID" {
                    Some(String::new())
                } else {
                    None
                }
            },
        );
        assert_eq!(id, TerminalIdentity::Kitty);
    }

    #[test]
    fn detect_terminal_identity_inside_tmux_falls_back_to_session_env_term_program_ghostty() {
        let id = detect_terminal_identity_with(
            tmux_set_only("/tmp/tmux-1000/default,123,4"),
            no_tmux_client_term,
            |name| {
                if name == "TERM_PROGRAM" {
                    Some("ghostty".to_string())
                } else {
                    None
                }
            },
        );
        assert_eq!(id, TerminalIdentity::Ghostty);
    }

    #[test]
    fn detect_terminal_identity_inside_tmux_returns_other_for_generic_client_termname() {
        let id = detect_terminal_identity_with(
            tmux_set_only("/tmp/tmux-1000/default,123,4"),
            || Some("xterm-256color".to_string()),
            no_tmux_env_lookup,
        );
        assert_eq!(id, TerminalIdentity::Other);
    }

    #[test]
    fn detect_terminal_identity_outside_tmux_skips_tmux_helpers() {
        use std::cell::Cell;

        let client_calls = Cell::new(0u32);
        let env_calls = Cell::new(0u32);
        let id = detect_terminal_identity_with(
            |_| None,
            || {
                client_calls.set(client_calls.get() + 1);
                Some("xterm-kitty".to_string())
            },
            |_| {
                env_calls.set(env_calls.get() + 1);
                Some(String::new())
            },
        );
        assert_eq!(id, TerminalIdentity::Other);
        assert_eq!(client_calls.get(), 0);
        assert_eq!(env_calls.get(), 0);
    }

    #[test]
    fn detect_terminal_identity_direct_env_takes_precedence_over_tmux_lookups() {
        use std::cell::Cell;

        let client_calls = Cell::new(0u32);
        let env_calls = Cell::new(0u32);
        let id = detect_terminal_identity_with(
            |name| match name {
                "TMUX" => Some("/tmp/tmux-1000/default,123,4".to_string()),
                "KITTY_WINDOW_ID" => Some(String::new()),
                _ => None,
            },
            || {
                client_calls.set(client_calls.get() + 1);
                Some("xterm-ghostty".to_string())
            },
            |_| {
                env_calls.set(env_calls.get() + 1);
                None
            },
        );
        assert_eq!(id, TerminalIdentity::Kitty);
        assert_eq!(
            client_calls.get(),
            0,
            "tmux helpers should not run when direct env detection succeeds"
        );
        assert_eq!(env_calls.get(), 0);
    }

    #[test]
    fn parse_show_environment_line_handles_set_unset_and_empty() {
        assert_eq!(
            parse_show_environment_line("KITTY_WINDOW_ID=42\n", "KITTY_WINDOW_ID"),
            Some("42".to_string())
        );
        assert_eq!(
            parse_show_environment_line("KITTY_WINDOW_ID=\n", "KITTY_WINDOW_ID"),
            Some(String::new())
        );
        assert_eq!(
            parse_show_environment_line("-KITTY_WINDOW_ID\n", "KITTY_WINDOW_ID"),
            None
        );
        assert_eq!(parse_show_environment_line("", "KITTY_WINDOW_ID"), None);
        assert_eq!(
            parse_show_environment_line("OTHER=value\nKITTY_WINDOW_ID=7\n", "KITTY_WINDOW_ID"),
            Some("7".to_string())
        );
    }
}
