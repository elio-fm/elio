#[cfg(all(unix, not(target_os = "macos")))]
mod desktop_file;
#[cfg(all(unix, not(target_os = "macos")))]
mod editor;
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
use crate::core::Entry;

// ── public entry point ────────────────────────────────────────────────────────

pub(super) fn discover_open_with_apps_for_entry(entry: &Entry) -> Vec<OpenWithApp> {
    discover_open_with_apps_inner(&entry.path, Some(entry.name.as_str()), true)
}

#[cfg(all(unix, not(target_os = "macos")))]
#[cfg_attr(test, allow(dead_code))]
pub(super) fn discover_desktop_apps_for_entry(entry: &Entry) -> Vec<OpenWithApp> {
    discover_open_with_apps_inner(&entry.path, Some(entry.name.as_str()), false)
}

#[cfg(all(unix, not(target_os = "macos")))]
#[cfg_attr(test, allow(dead_code))]
pub(super) fn editor_fallback_app_for_entry(entry: &Entry) -> Option<OpenWithApp> {
    editor::editor_fallback_for_path(&entry.path)
}

fn discover_open_with_apps_inner(
    path: &Path,
    display_name: Option<&str>,
    include_editor_fallback: bool,
) -> Vec<OpenWithApp> {
    #[cfg(target_os = "macos")]
    {
        let _ = display_name;
        let _ = include_editor_fallback;
        macos::discover_via_nsworkspace(path)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if path.is_dir() {
            discover_xdg_for_mime("inode/directory", path, true, false)
        } else {
            discover_xdg(path, display_name, include_editor_fallback)
        }
    }
    #[cfg(not(any(target_os = "macos", all(unix, not(target_os = "macos")))))]
    {
        let _ = path;
        let _ = display_name;
        let _ = include_editor_fallback;
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
fn discover_xdg(
    path: &Path,
    display_name: Option<&str>,
    include_editor_fallback: bool,
) -> Vec<OpenWithApp> {
    use std::time::{Duration, Instant};

    // 3-second budget for subprocess fallbacks; pure-Rust MIME lookup is
    // instant and is tried first, so the timeout rarely matters in practice.
    let deadline = Instant::now() + Duration::from_millis(3000);
    let canceled = || Instant::now() > deadline;

    let Some(mime_type) = mime::detect_mime_type_with_name(path, display_name, &canceled) else {
        return vec![];
    };

    discover_xdg_for_mime(&mime_type, path, include_editor_fallback, true)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn discover_xdg_for_mime(
    mime_type: &str,
    path: &Path,
    include_env_editor_fallback: bool,
    require_text_like_editor: bool,
) -> Vec<OpenWithApp> {
    use std::time::{Duration, Instant};

    let deadline = Instant::now() + Duration::from_millis(3000);
    let canceled = || Instant::now() > deadline;

    // Primary: gio handles MIME inheritance (e.g. text/markdown → text/plain),
    // aliases, and added/removed associations from mimeapps.list.
    let mut apps = match gio::discover_via_gio(mime_type, path, &canceled) {
        Some(apps) if !apps.is_empty() => apps,
        _ => {
            // Fallback: manual desktop-file scan with exact MIME match.
            scan::discover_via_desktop_scan(mime_type, path)
        }
    };

    if include_env_editor_fallback {
        editor::append_editor_fallback(&mut apps, path, require_text_like_editor);
    }
    apps
}
