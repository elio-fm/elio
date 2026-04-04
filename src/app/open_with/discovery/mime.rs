// This module is only compiled on Linux / BSD (gated in discovery/mod.rs).

use std::path::Path;
use std::process::Command;

use crate::preview::process::run_command_capture_stdout_cancellable;

pub(super) fn detect_mime_type(path: &Path, canceled: &impl Fn() -> bool) -> Option<String> {
    // Fast path: look up the file extension in the XDG MIME globs database.
    // This is instant (pure file read), uses the same data source as
    // `xdg-mime`, and covers virtually all files with a recognisable extension.
    if let Some(mime) = mime_from_xdg_database(path) {
        return Some(mime);
    }

    if canceled() {
        return None;
    }

    // Slow path: invoke xdg-mime for extensionless or ambiguous files that
    // need content-based (magic-byte) detection.
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
/// subprocess.  Reads `/usr/share/mime/globs2` (with priority weights) and
/// falls back to `/usr/share/mime/globs`.
fn mime_from_xdg_database(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let target = format!("*.{ext}");

    // globs2: weight:mime/type:glob-pattern  (higher weight wins)
    if let Ok(contents) = std::fs::read_to_string("/usr/share/mime/globs2") {
        let mut best_weight = -1i32;
        let mut best_mime: Option<String> = None;

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
        if best_mime.is_some() {
            return best_mime;
        }
    }

    // globs: mime/type:glob-pattern  (no weights — first match wins)
    if let Ok(contents) = std::fs::read_to_string("/usr/share/mime/globs") {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── mime_from_xdg_database ────────────────────────────────────────────────

    #[test]
    fn mime_from_xdg_database_returns_expected_type_for_common_extensions() {
        // The system globs2/globs database must exist for this test to be
        // meaningful.  Skip gracefully if it does not (e.g. exotic CI image).
        if !std::path::Path::new("/usr/share/mime/globs2").exists()
            && !std::path::Path::new("/usr/share/mime/globs").exists()
        {
            return;
        }
        // .png is universally registered as image/png.
        let result = mime_from_xdg_database(Path::new("/any/path/image.png"));
        assert_eq!(
            result.as_deref(),
            Some("image/png"),
            "expected image/png for .png extension"
        );
    }

    #[test]
    fn mime_from_xdg_database_returns_none_for_unknown_extension() {
        let result = mime_from_xdg_database(Path::new("/tmp/file.xyzzy_unknown_ext"));
        assert!(result.is_none(), "unknown extension should return None");
    }
}
