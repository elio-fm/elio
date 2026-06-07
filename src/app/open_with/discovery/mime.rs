// This module is only compiled on Linux / BSD (gated in discovery/mod.rs).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::preview::process::run_command_capture_stdout_cancellable;

pub(super) fn detect_mime_type_with_name(
    path: &Path,
    display_name: Option<&str>,
    canceled: &impl Fn() -> bool,
) -> Option<String> {
    // Fast path: look up the file extension in the XDG MIME globs database.
    // This is instant (pure file read), covers virtually all files with a
    // recognisable extension, and works correctly on both Linux and BSD because
    // it searches the full XDG data dir chain rather than a hardcoded path.
    if let Some(mime) = mime_from_data_dirs_with_name(path, display_name, &super::xdg_data_dirs()) {
        return Some(mime);
    }

    if canceled() {
        return None;
    }

    // Slow path: gio info uses GLib's MIME detection, which agrees with gio open
    // and handles extensionless or ambiguous files more consistently than
    // xdg-mime's generic text fallback.
    let mut cmd = Command::new("gio");
    cmd.args(["info", "-a", "standard::content-type"]).arg(path);
    if let Some(out) = run_command_capture_stdout_cancellable(cmd, "open-with-mime-gio", canceled) {
        let text = String::from_utf8_lossy(&out);
        if let Some(mime) = parse_gio_content_type(&text) {
            return Some(mime);
        }
    }

    if canceled() {
        return None;
    }

    // Fallback: xdg-mime for systems without gio.
    let mut cmd = Command::new("xdg-mime");
    cmd.args(["query", "filetype"]).arg(path);
    if let Some(out) = run_command_capture_stdout_cancellable(cmd, "open-with-mime", canceled) {
        let s = String::from_utf8_lossy(&out).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }

    if canceled() {
        return None;
    }

    // Last resort: file(1).
    let mut cmd = Command::new("file");
    cmd.args(["--mime-type", "-b"]).arg(path);
    if let Some(out) = run_command_capture_stdout_cancellable(cmd, "open-with-mime-fb", canceled) {
        let s = String::from_utf8_lossy(&out).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }

    None
}

/// Looks up MIME type from the XDG MIME globs database without spawning any
/// subprocess.  Searches each XDG data directory for `mime/globs2` (weighted)
/// and falls back to `mime/globs` (unweighted).
///
/// This correctly handles BSD systems where `shared-mime-info` installs to
/// `/usr/local/share/mime/` rather than `/usr/share/mime/`.
fn mime_from_data_dirs_with_name(
    path: &Path,
    display_name: Option<&str>,
    data_dirs: &[PathBuf],
) -> Option<String> {
    display_name
        .and_then(|name| mime_from_data_dirs(Path::new(name), data_dirs))
        .or_else(|| mime_from_data_dirs(path, data_dirs))
}

/// Inner implementation that accepts an explicit data-dir list for testing.
/// Searches `{dir}/mime/globs2` (highest weight wins across all dirs), then
/// `{dir}/mime/globs` (first match across dirs in priority order).
pub(super) fn mime_from_data_dirs(path: &Path, data_dirs: &[PathBuf]) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let target = format!("*.{ext}");

    // ── globs2 pass: weight:mime/type:glob-pattern ────────────────────────────
    // Collect the highest-weight match across all data dirs.  This mirrors how
    // shared-mime-info merges databases from multiple XDG data directories.
    let mut best_weight = -1i32;
    let mut best_mime: Option<String> = None;

    for dir in data_dirs {
        let globs2 = dir.join("mime/globs2");
        let Ok(contents) = std::fs::read_to_string(&globs2) else {
            continue;
        };
        for line in contents.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(3, ':');
            let (Some(w_str), Some(mime), Some(pattern)) =
                (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if pattern != target {
                continue;
            }
            let weight: i32 = w_str.parse().unwrap_or(50);
            if weight > best_weight {
                best_weight = weight;
                best_mime = Some(mime.to_string());
            }
        }
    }
    if best_mime.is_some() {
        return best_mime;
    }

    // ── globs pass: mime/type:glob-pattern ────────────────────────────────────
    // No weights — first match in priority order wins.
    for dir in data_dirs {
        let globs = dir.join("mime/globs");
        let Ok(contents) = std::fs::read_to_string(&globs) else {
            continue;
        };
        for line in contents.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let Some((mime, pattern)) = line.split_once(':') else {
                continue;
            };
            if pattern == target {
                return Some(mime.to_string());
            }
        }
    }

    None
}

fn parse_gio_content_type(output: &str) -> Option<String> {
    for line in output.lines() {
        if let Some(rest) = line.trim().strip_prefix("standard::content-type:") {
            let mime = rest.trim().to_string();
            if !mime.is_empty() {
                return Some(mime);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
