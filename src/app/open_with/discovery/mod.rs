mod desktop_file;
mod exec;

#[cfg(all(unix, not(target_os = "macos")))]
mod gio;
#[cfg(all(unix, not(target_os = "macos")))]
mod mime;
#[cfg(all(unix, not(target_os = "macos")))]
mod scan;

use std::path::Path;

use super::super::state::OpenWithApp;

// ── public entry point ────────────────────────────────────────────────────────

pub(super) fn discover_open_with_apps(path: &Path) -> Vec<OpenWithApp> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        discover_xdg(path)
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let _ = path;
        vec![]
    }
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
