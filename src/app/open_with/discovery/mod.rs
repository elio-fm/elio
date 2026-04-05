#[cfg(all(unix, not(target_os = "macos")))]
mod desktop_file;
mod exec;

#[cfg(all(unix, not(target_os = "macos")))]
mod gio;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(all(unix, not(target_os = "macos")))]
mod mime;
#[cfg(all(unix, not(target_os = "macos")))]
mod scan;

use std::path::Path;

use super::super::state::OpenWithApp;

// ── public entry point ────────────────────────────────────────────────────────

pub(super) fn discover_open_with_apps(path: &Path) -> Vec<OpenWithApp> {
    #[cfg(target_os = "macos")]
    {
        macos::discover_via_nsworkspace(path)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        discover_xdg(path)
    }
    #[cfg(not(any(target_os = "macos", all(unix, not(target_os = "macos")))))]
    {
        let _ = path;
        vec![]
    }
}

// ── Shared XDG helpers (Linux / BSD) ─────────────────────────────────────────

/// Returns the ordered list of XDG base data directories:
/// `XDG_DATA_HOME` first, then each entry in `XDG_DATA_DIRS`.
/// Falls back to spec defaults (`~/.local/share` and `/usr/local/share:/usr/share`)
/// when the environment variables are unset.
#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn xdg_data_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();

    let data_home = std::env::var("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join(".local/share"))
                .unwrap_or_default()
        });
    if !data_home.as_os_str().is_empty() {
        dirs.push(data_home);
    }

    for entry in std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string())
        .split(':')
        .filter(|s| !s.is_empty())
    {
        dirs.push(std::path::PathBuf::from(entry));
    }

    dirs
}

/// Returns the desktop names from `$XDG_CURRENT_DESKTOP` (colon-separated,
/// original case).  Empty when the variable is unset or empty.
#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

// ── XDG discovery (Linux / BSD) ───────────────────────────────────────────────

#[cfg(all(unix, not(target_os = "macos")))]
fn discover_xdg(path: &Path) -> Vec<OpenWithApp> {
    use std::time::{Duration, Instant};

    // 3-second budget for subprocess fallbacks; pure-Rust MIME lookup is
    // instant and is tried first, so the timeout rarely matters in practice.
    let deadline = Instant::now() + Duration::from_millis(3000);
    let canceled = || Instant::now() > deadline;

    let Some(mime_type) = mime::detect_mime_type(path, &canceled) else {
        return vec![];
    };

    // Primary: gio handles MIME inheritance (e.g. text/markdown → text/plain),
    // aliases, and added/removed associations from mimeapps.list.
    if let Some(apps) = gio::discover_via_gio(&mime_type, path, &canceled)
        && !apps.is_empty()
    {
        return apps;
    }

    // Fallback: manual desktop-file scan with exact MIME match.
    scan::discover_via_desktop_scan(&mime_type, path)
}
