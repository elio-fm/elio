use super::ClipOp;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExternalFileClipboard {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) op: ClipOp,
}

pub(crate) fn read_external_file_clipboard() -> Option<ExternalFileClipboard> {
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        if wayland_session()
            && let Some(clipboard) = read_wayland_clipboard()
        {
            return Some(clipboard);
        }
        if std::env::var_os("DISPLAY").is_some() {
            return read_x11_clipboard();
        }
    }

    None
}

pub(crate) fn read_external_text_clipboard() -> Option<String> {
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        if wayland_session()
            && let Some(text) = read_wayland_text_clipboard()
        {
            return Some(text);
        }
        if std::env::var_os("DISPLAY").is_some() {
            return read_x11_text_clipboard();
        }
    }

    None
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn read_wayland_text_clipboard() -> Option<String> {
    let output = run_command("wl-paste", &["--list-types"])?;
    if !output.status.success() {
        return None;
    }
    let types = String::from_utf8_lossy(&output.stdout);
    let types = types.lines().map(str::trim).collect::<Vec<_>>();
    read_text_clipboard_types(
        |mime| {
            let output = run_command("wl-paste", &["--no-newline", "--type", mime])?;
            output.status.success().then_some(output.stdout)
        },
        &types,
    )
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn read_x11_text_clipboard() -> Option<String> {
    let output = run_command("xclip", &["-selection", "clipboard", "-t", "TARGETS", "-o"])?;
    if !output.status.success() {
        return None;
    }
    let targets = String::from_utf8_lossy(&output.stdout);
    let targets = targets.lines().map(str::trim).collect::<Vec<_>>();
    read_text_clipboard_types(
        |mime| {
            let output = run_command("xclip", &["-selection", "clipboard", "-t", mime, "-o"])?;
            output.status.success().then_some(output.stdout)
        },
        &targets,
    )
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn read_text_clipboard_types(
    mut read_type: impl FnMut(&str) -> Option<Vec<u8>>,
    types: &[&str],
) -> Option<String> {
    let mime = types.iter().copied().find(|mime| {
        let mime = mime.to_ascii_lowercase();
        mime == "text/plain"
            || mime.starts_with("text/plain;")
            || matches!(mime.as_str(), "utf8_string" | "string" | "text")
    })?;
    String::from_utf8(read_type(mime)?).ok()
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn read_wayland_clipboard() -> Option<ExternalFileClipboard> {
    let types = run_command("wl-paste", &["--list-types"])?;
    if !types.status.success() {
        return None;
    }
    let types = String::from_utf8_lossy(&types.stdout);
    let types = types.lines().map(str::trim).collect::<Vec<_>>();
    read_clipboard_types(
        |mime| {
            let output = run_command("wl-paste", &["--no-newline", "--type", mime])?;
            output.status.success().then_some(output.stdout)
        },
        &types,
    )
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn read_x11_clipboard() -> Option<ExternalFileClipboard> {
    let targets = run_command("xclip", &["-selection", "clipboard", "-t", "TARGETS", "-o"])?;
    if !targets.status.success() {
        return None;
    }
    let targets = String::from_utf8_lossy(&targets.stdout);
    let targets = targets.lines().map(str::trim).collect::<Vec<_>>();
    read_clipboard_types(
        |mime| {
            let output = run_command("xclip", &["-selection", "clipboard", "-t", mime, "-o"])?;
            output.status.success().then_some(output.stdout)
        },
        &targets,
    )
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn run_command(program: &str, args: &[&str]) -> Option<std::process::Output> {
    std::process::Command::new(program).args(args).output().ok()
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
pub(super) fn read_clipboard_types(
    mut read_type: impl FnMut(&str) -> Option<Vec<u8>>,
    types: &[&str],
) -> Option<ExternalFileClipboard> {
    if types.contains(&"x-special/gnome-copied-files")
        && let Some(payload) = read_type("x-special/gnome-copied-files")
        && let Some(clipboard) = parse_gnome_copied_files(&payload)
    {
        return Some(clipboard);
    }

    if !types.contains(&"text/uri-list") {
        return None;
    }
    let payload = read_type("text/uri-list")?;
    let paths = parse_uri_list(&payload);
    if paths.is_empty() {
        return None;
    }

    let op = if types.contains(&"application/x-kde-cutselection")
        && read_type("application/x-kde-cutselection")
            .and_then(|value| value.first().copied())
            .is_some_and(|value| value == b'1')
    {
        ClipOp::Cut
    } else {
        ClipOp::Yank
    };

    Some(ExternalFileClipboard { paths, op })
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
pub(super) fn parse_gnome_copied_files(payload: &[u8]) -> Option<ExternalFileClipboard> {
    let text = std::str::from_utf8(payload).ok()?;
    let (operation, uri_list) = text.split_once('\n')?;
    let op = match operation.trim().to_ascii_lowercase().as_str() {
        "copy" => ClipOp::Yank,
        "cut" => ClipOp::Cut,
        _ => return None,
    };
    let paths = parse_uri_list(uri_list.as_bytes());
    (!paths.is_empty()).then_some(ExternalFileClipboard { paths, op })
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
pub(super) fn parse_uri_list(payload: &[u8]) -> Vec<PathBuf> {
    let Ok(text) = std::str::from_utf8(payload) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(file_uri_to_path)
        .collect()
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let path = if let Some(local) = rest.strip_prefix("localhost/") {
        format!("/{local}")
    } else if rest.starts_with('/') {
        rest.to_string()
    } else {
        return None;
    };
    Some(PathBuf::from(percent_decode(&path)?))
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
