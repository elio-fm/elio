//! Freedesktop file-clipboard MIME formats shared by the Wayland and X11 backends.

use super::{FileClipboard, FileClipboardOperation};
use std::path::{Path, PathBuf};

pub(super) fn read_file_clipboard(
    mut read_type: impl FnMut(&str) -> Option<Vec<u8>>,
    types: &[&str],
) -> Option<FileClipboard> {
    if types.contains(&"x-special/gnome-copied-files")
        && let Some(payload) = read_type("x-special/gnome-copied-files")
        && let Some(clipboard) = parse_gnome_copied_files(&payload)
    {
        return Some(clipboard);
    }
    if !types.contains(&"text/uri-list") {
        return None;
    }
    let paths = parse_uri_list(&read_type("text/uri-list")?);
    if paths.is_empty() {
        return None;
    }
    let operation = if types.contains(&"application/x-kde-cutselection")
        && read_type("application/x-kde-cutselection").and_then(|value| value.first().copied())
            == Some(b'1')
    {
        FileClipboardOperation::Cut
    } else {
        FileClipboardOperation::Copy
    };
    Some(FileClipboard { paths, operation })
}

pub(super) fn read_text(
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

pub(super) fn file_payloads(paths: &[PathBuf]) -> Vec<(String, Vec<u8>)> {
    let uris = uri_list(paths);
    vec![
        (
            "x-special/gnome-copied-files".to_string(),
            format!("copy\n{}", uris.replace("\r\n", "\n")).into_bytes(),
        ),
        ("text/uri-list".to_string(), uris.clone().into_bytes()),
        (
            "text/plain;charset=utf-8".to_string(),
            paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n")
                .into_bytes(),
        ),
    ]
}

pub(super) fn uri_list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| file_uri(path))
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn parse_gnome_copied_files(payload: &[u8]) -> Option<FileClipboard> {
    let text = std::str::from_utf8(payload).ok()?;
    let (operation, uri_list) = text.split_once('\n')?;
    let operation = match operation.trim().to_ascii_lowercase().as_str() {
        "copy" => FileClipboardOperation::Copy,
        "cut" => FileClipboardOperation::Cut,
        _ => return None,
    };
    let paths = parse_uri_list(uri_list.as_bytes());
    (!paths.is_empty()).then_some(FileClipboard { paths, operation })
}

fn parse_uri_list(payload: &[u8]) -> Vec<PathBuf> {
    let Ok(text) = std::str::from_utf8(payload) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(file_uri_to_path)
        .collect()
}

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

fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            uri.push(byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    uri
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
pub(super) fn parse_gnome_for_test(payload: &[u8]) -> Option<FileClipboard> {
    parse_gnome_copied_files(payload)
}
#[cfg(test)]
pub(super) fn parse_uri_for_test(payload: &[u8]) -> Vec<PathBuf> {
    parse_uri_list(payload)
}
