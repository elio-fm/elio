use serde::Deserialize;
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalAdapter {
    Kitty,
    Ghostty,
    Foot,
    #[serde(rename = "wezterm")]
    WezTerm,
    Alacritty,
    Rio,
    Konsole,
    #[serde(rename = "gnome-terminal")]
    Gnome,
    Xterm,
}

impl TerminalAdapter {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "kitty" => Some(Self::Kitty),
            "ghostty" => Some(Self::Ghostty),
            "foot" => Some(Self::Foot),
            "wezterm" => Some(Self::WezTerm),
            "alacritty" => Some(Self::Alacritty),
            "rio" => Some(Self::Rio),
            "konsole" => Some(Self::Konsole),
            "gnome-terminal" => Some(Self::Gnome),
            "xterm" => Some(Self::Xterm),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::Ghostty => "ghostty",
            Self::Foot => "foot",
            Self::WezTerm => "wezterm",
            Self::Alacritty => "alacritty",
            Self::Rio => "rio",
            Self::Konsole => "konsole",
            Self::Gnome => "gnome-terminal",
            Self::Xterm => "xterm",
        }
    }

    /// Resolve the configured terminal from this service's activation PATH.
    /// D-Bus activation need not inherit an interactive shell environment.
    pub fn resolve_executable(self) -> Option<PathBuf> {
        let name = self.name();
        env::var_os("PATH")
            .as_deref()
            .into_iter()
            .flat_map(env::split_paths)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    }

    /// Build terminal arguments for a portal chooser child. This performs no
    /// process spawning or executable resolution.
    pub fn chooser_args(self, child: &[OsString]) -> Vec<OsString> {
        const TITLE: &str = "elio File Chooser";
        const APP_ID: &str = "io.github.elio_fm.elio.filechooser";

        let mut args = match self {
            Self::Kitty => vec![
                "--title".into(),
                TITLE.into(),
                "--app-id".into(),
                APP_ID.into(),
            ],
            Self::Ghostty => vec![
                // Ghostty configuration overrides require `--key=value`.
                format!("--title={TITLE}").into(),
                format!("--class={APP_ID}").into(),
                "-e".into(),
            ],
            Self::Foot => vec![
                format!("--title={TITLE}").into(),
                format!("--app-id={APP_ID}").into(),
            ],
            Self::WezTerm => vec!["start".into(), "--class".into(), APP_ID.into(), "--".into()],
            Self::Alacritty => vec![
                "--title".into(),
                TITLE.into(),
                "--class".into(),
                APP_ID.into(),
                "--command".into(),
            ],
            Self::Rio => vec![
                "--title-placeholder".into(),
                TITLE.into(),
                "--command".into(),
            ],
            Self::Konsole => vec!["-p".into(), format!("tabtitle={TITLE}").into(), "-e".into()],
            Self::Gnome => vec![
                "--title".into(),
                TITLE.into(),
                format!("--class={APP_ID}").into(),
                "--".into(),
            ],
            Self::Xterm => vec![
                "-T".into(),
                TITLE.into(),
                "-class".into(),
                APP_ID.into(),
                "-e".into(),
            ],
        };
        args.extend_from_slice(child);
        args
    }
}

#[cfg(test)]
#[path = "tests/terminal.rs"]
mod tests;

pub fn detect_with(
    env_lookup: &impl Fn(&str) -> Option<String>,
    ancestors: &[String],
) -> Option<TerminalAdapter> {
    if let Some(terminal) = ancestors
        .iter()
        .find_map(|command| classify_command(command))
    {
        return Some(terminal);
    }

    let has = |name| env_lookup(name).is_some();
    if has("KITTY_WINDOW_ID") {
        return Some(TerminalAdapter::Kitty);
    }
    if has("WEZTERM_PANE") {
        return Some(TerminalAdapter::WezTerm);
    }
    if has("ALACRITTY_SOCKET") {
        return Some(TerminalAdapter::Alacritty);
    }
    if has("KONSOLE_DBUS_SESSION") || has("KONSOLE_DBUS_SERVICE") || has("KONSOLE_DBUS_WINDOW") {
        return Some(TerminalAdapter::Konsole);
    }
    if has("GNOME_TERMINAL_SCREEN") || has("GNOME_TERMINAL_SERVICE") {
        return Some(TerminalAdapter::Gnome);
    }

    let term_program = env_lookup("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if let Some(terminal) = classify_name(&term_program) {
        return Some(terminal);
    }

    let term = env_lookup("TERM").unwrap_or_default().to_ascii_lowercase();
    if term.contains("xterm-kitty") {
        return Some(TerminalAdapter::Kitty);
    }
    if term.contains("ghostty") {
        return Some(TerminalAdapter::Ghostty);
    }
    if term.contains("wezterm") {
        return Some(TerminalAdapter::WezTerm);
    }
    if term.contains("alacritty") {
        return Some(TerminalAdapter::Alacritty);
    }
    if matches!(term.as_str(), "rio" | "xterm-rio") {
        return Some(TerminalAdapter::Rio);
    }
    if matches!(term.as_str(), "foot" | "foot-extra") {
        return Some(TerminalAdapter::Foot);
    }
    if term.starts_with("xterm") {
        return Some(TerminalAdapter::Xterm);
    }

    None
}

pub fn classify_command(command: &str) -> Option<TerminalAdapter> {
    let command = command.split_whitespace().next()?;
    let name = Path::new(command)
        .file_name()?
        .to_string_lossy()
        .to_ascii_lowercase();
    classify_name(&name)
}

fn classify_name(name: &str) -> Option<TerminalAdapter> {
    match name {
        "kitty" => Some(TerminalAdapter::Kitty),
        "ghostty" => Some(TerminalAdapter::Ghostty),
        "foot" | "footclient" => Some(TerminalAdapter::Foot),
        "wezterm" | "wezterm-gui" => Some(TerminalAdapter::WezTerm),
        "alacritty" => Some(TerminalAdapter::Alacritty),
        "rio" => Some(TerminalAdapter::Rio),
        "konsole" => Some(TerminalAdapter::Konsole),
        "gnome-terminal" | "gnome-terminal-server" => Some(TerminalAdapter::Gnome),
        "xterm" => Some(TerminalAdapter::Xterm),
        _ => None,
    }
}
