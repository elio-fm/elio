use std::{env, process::Command};

const MIN_KITTY_DND_VERSION: KittyVersion = KittyVersion {
    major: 0,
    minor: 47,
    patch: 0,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_runtime) struct KittyDndRuntime {
    enabled: bool,
    machine_id: Option<String>,
}

impl KittyDndRuntime {
    pub(super) const fn disabled() -> Self {
        Self {
            enabled: false,
            machine_id: None,
        }
    }

    pub(super) fn enabled(machine_id: Option<String>) -> Self {
        Self {
            enabled: true,
            machine_id,
        }
    }

    pub(in crate::terminal_runtime) const fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(in crate::terminal_runtime) fn drag_machine_id(&self) -> Option<&str> {
        self.machine_id.as_deref()
    }
}

pub(in crate::terminal_runtime) fn detect_kitty_dnd_runtime() -> KittyDndRuntime {
    let env = RuntimeEnv::read();
    detect_from_env(&env, query_kitty_version, local_machine_id)
}

#[derive(Debug, Default)]
struct RuntimeEnv {
    term: Option<String>,
    term_program: Option<String>,
    kitty_window_id: bool,
    tmux: bool,
    zellij: bool,
    ssh_connection: bool,
    ssh_tty: bool,
}

impl RuntimeEnv {
    fn read() -> Self {
        Self {
            term: env::var("TERM").ok(),
            term_program: env::var("TERM_PROGRAM").ok(),
            kitty_window_id: env::var_os("KITTY_WINDOW_ID").is_some(),
            tmux: env::var_os("TMUX").is_some(),
            zellij: env::var_os("ZELLIJ").is_some(),
            ssh_connection: env::var_os("SSH_CONNECTION").is_some(),
            ssh_tty: env::var_os("SSH_TTY").is_some(),
        }
    }
}

fn detect_from_env(
    env: &RuntimeEnv,
    query_version: impl FnOnce() -> Option<KittyVersion>,
    _read_machine_id: impl FnOnce() -> Option<String>,
) -> KittyDndRuntime {
    if !is_kitty_env(env) || env.tmux || env.zellij || env.ssh_connection || env.ssh_tty {
        return KittyDndRuntime::disabled();
    }

    match query_version() {
        Some(version) if version >= MIN_KITTY_DND_VERSION => KittyDndRuntime::enabled(None),
        _ => KittyDndRuntime::disabled(),
    }
}

fn is_kitty_env(env: &RuntimeEnv) -> bool {
    env.term
        .as_deref()
        .is_some_and(|term| term.to_ascii_lowercase().contains("xterm-kitty"))
        || env
            .term_program
            .as_deref()
            .is_some_and(|program| program.eq_ignore_ascii_case("kitty"))
        || env.kitty_window_id
}

fn query_kitty_version() -> Option<KittyVersion> {
    let output = Command::new("kitty").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_kitty_version(&String::from_utf8_lossy(&output.stdout))
        .or_else(|| parse_kitty_version(&String::from_utf8_lossy(&output.stderr)))
}

fn local_machine_id() -> Option<String> {
    None
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct KittyVersion {
    major: u16,
    minor: u16,
    patch: u16,
}

fn parse_kitty_version(output: &str) -> Option<KittyVersion> {
    let version = output
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = version.split('.');
    Some(KittyVersion {
        major: parts.next()?.parse().ok()?,
        minor: parts.next()?.parse().ok()?,
        patch: parts
            .next()
            .and_then(|patch| {
                patch
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .unwrap_or(0),
    })
}

#[cfg(test)]
#[path = "tests/support.rs"]
mod tests;
