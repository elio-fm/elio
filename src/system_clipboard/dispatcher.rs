#[cfg(target_os = "macos")]
use super::macos;
#[cfg(windows)]
use super::windows;
use super::{ClipboardError, FileClipboard, PathBuf};
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
use super::{wayland, x11};

/// Reads native file items, if the current system clipboard offers any.
pub(super) fn read_files() -> Option<FileClipboard> {
    #[cfg(target_os = "macos")]
    {
        macos::read_files()
    }
    #[cfg(windows)]
    {
        windows::read_files()
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        if wayland_session()
            && let Some(files) = wayland::read_files()
        {
            return Some(files);
        }
        std::env::var_os("DISPLAY")
            .is_some()
            .then(x11::read_files)
            .flatten()
    }
    #[cfg(not(any(
        target_os = "macos",
        windows,
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )))]
    {
        None
    }
}

/// Reads text from the system clipboard, without interpreting it as files.
pub(super) fn read_text() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos::read_text()
    }
    #[cfg(windows)]
    {
        windows::read_text()
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        if wayland_session()
            && let Some(text) = wayland::read_text()
        {
            return Some(text);
        }
        std::env::var_os("DISPLAY")
            .is_some()
            .then(x11::read_text)
            .flatten()
    }
    #[cfg(not(any(
        target_os = "macos",
        windows,
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )))]
    {
        None
    }
}

/// Publishes local file references in the OS-native file clipboard.
pub(super) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError> {
    #[cfg(target_os = "macos")]
    {
        macos::write_files(paths)
    }
    #[cfg(windows)]
    {
        windows::write_files(paths)
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        if wayland_session() && wayland::write_files(paths).is_ok() {
            return Ok(());
        }
        if std::env::var_os("DISPLAY").is_some()
            || cfg!(test) && std::env::var_os("ELIO_TEST_ITEM_CLIPBOARD_TOOL").is_some()
        {
            return x11::write_files(paths);
        }
        Err(ClipboardError::Unavailable)
    }
    #[cfg(not(any(
        target_os = "macos",
        windows,
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )))]
    {
        let _ = paths;
        Err(ClipboardError::Unavailable)
    }
}

/// Publishes text through the OS clipboard (or the terminal's OSC52 bridge).
pub(super) fn write_text(text: &str) -> Result<(), ClipboardError> {
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        if terminal_supports_osc52() {
            return write_osc52(text);
        }
        if wayland_session() && wayland::write_text(text).is_ok() {
            return Ok(());
        }
        x11::write_text(text)
    }
    #[cfg(target_os = "macos")]
    return macos::write_text(text);
    #[cfg(windows)]
    return windows::write_text(text);
    #[cfg(not(any(unix, windows)))]
    {
        let _ = text;
        Err(ClipboardError::Unavailable)
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
pub(super) fn platform_command(
    program: &str,
    args: &[&str],
    text: &str,
) -> Result<(), ClipboardError> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ClipboardError::HelperMissing
            } else {
                ClipboardError::Unavailable
            }
        })?;
    child
        .stdin
        .take()
        .ok_or(ClipboardError::WriteFailed)?
        .write_all(text.as_bytes())
        .map_err(|_| ClipboardError::WriteFailed)?;
    child
        .wait()
        .map_err(|_| ClipboardError::WriteFailed)?
        .success()
        .then_some(())
        .ok_or(ClipboardError::WriteFailed)
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("XDG_SESSION_TYPE")
            .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("wayland"))
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn terminal_supports_osc52() -> bool {
    if std::env::var_os("TMUX").is_some() && !tmux_accepts_application_osc52() {
        return false;
    }
    #[cfg(test)]
    if std::env::var_os("ELIO_TEST_OSC52_CAPTURE").is_some() {
        return true;
    }
    std::env::var_os("ELIO_CLIPBOARD_OSC52").is_some()
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn tmux_accepts_application_osc52() -> bool {
    #[cfg(test)]
    if let Some(value) = std::env::var_os("ELIO_TEST_TMUX_SET_CLIPBOARD") {
        return value.to_string_lossy().trim().eq_ignore_ascii_case("on");
    }
    std::process::Command::new("tmux")
        .args(["show-options", "-gqv", "set-clipboard"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("on"))
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn write_osc52(text: &str) -> Result<(), ClipboardError> {
    use base64::Engine as _;
    let sequence = format!(
        "\x1b]52;c;{}\x1b\\",
        base64::engine::general_purpose::STANDARD.encode(text)
    );
    #[cfg(test)]
    if let Some(path) = std::env::var_os("ELIO_TEST_OSC52_CAPTURE") {
        return std::fs::write(path, sequence).map_err(|_| ClipboardError::WriteFailed);
    }
    use std::io::{IsTerminal, Write};
    let mut stdout = std::io::stdout();
    if !stdout.is_terminal() {
        return Err(ClipboardError::Unavailable);
    }
    stdout
        .write_all(sequence.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|_| ClipboardError::WriteFailed)
}
