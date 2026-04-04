// This module is only compiled on Linux / BSD (gated in discovery/mod.rs).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::super::super::state::OpenWithApp;
use super::desktop_file::{DesktopEntryCandidate, parse_desktop_entry, parse_mimeapps_defaults};
use super::exec::expand_exec_template;

/// Manual desktop-file scan: walks all desktop entry directories and returns
/// apps that explicitly list `mime` in their `MimeType=` field.
/// Used as a fallback when `gio` is unavailable.
pub(super) fn discover_via_desktop_scan(mime: &str, path: &Path) -> Vec<OpenWithApp> {
    discover_via_desktop_scan_in_dirs(mime, path, &desktop_entry_dirs())
}

/// Inner scan that accepts an explicit directory list (allows hermetic testing).
fn discover_via_desktop_scan_in_dirs(
    mime: &str,
    path: &Path,
    dirs: &[PathBuf],
) -> Vec<OpenWithApp> {
    // Collect all desktop entries that declare this MIME type, keyed by
    // desktop-id.  Higher-priority directories come first, so we skip any id
    // that was already seen.
    let mut candidates: HashMap<String, DesktopEntryCandidate> = HashMap::new();
    for dir in dirs {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut entries: Vec<_> = read_dir.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let entry_path = entry.path();
            if entry_path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let desktop_id = match entry_path.file_name() {
                Some(n) => n.to_string_lossy().into_owned(),
                None => continue,
            };
            if candidates.contains_key(&desktop_id) {
                continue;
            }
            let contents = match std::fs::read_to_string(&entry_path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let Some(candidate) = parse_desktop_entry(&contents) else {
                continue;
            };
            if candidate.mime_types.iter().any(|m| m == mime) {
                candidates.insert(desktop_id, candidate);
            }
        }
    }

    // Determine preferred ordering from mimeapps.list files.  Walk in
    // priority order and stop at the first file that mentions this MIME type.
    let ordered_defaults: Vec<String> = mimeapps_paths()
        .iter()
        .find_map(|p| {
            let contents = std::fs::read_to_string(p).ok()?;
            let defaults = parse_mimeapps_defaults(&contents, mime);
            if defaults.is_empty() {
                None
            } else {
                Some(defaults)
            }
        })
        .unwrap_or_default();

    // Build result: defaults first (in declared order), then the rest by name.
    let mut apps: Vec<OpenWithApp> = Vec::new();

    for desktop_id in &ordered_defaults {
        let Some(candidate) = candidates.remove(desktop_id) else {
            continue;
        };
        let Some((program, args)) = expand_exec_template(&candidate.exec, path) else {
            continue;
        };
        apps.push(OpenWithApp {
            display_name: candidate.name,
            desktop_id: Some(desktop_id.clone()),
            program,
            args,
            is_default: true,
            requires_terminal: candidate.terminal,
        });
    }

    let mut remaining: Vec<_> = candidates.into_iter().collect();
    remaining.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));

    for (desktop_id, candidate) in remaining {
        let Some((program, args)) = expand_exec_template(&candidate.exec, path) else {
            continue;
        };
        apps.push(OpenWithApp {
            display_name: candidate.name,
            desktop_id: Some(desktop_id),
            program,
            args,
            is_default: false,
            requires_terminal: candidate.terminal,
        });
    }

    apps
}

/// Returns the ordered list of directories to search for `.desktop` files,
/// from highest to lowest priority, following the XDG Base Dir spec.
///
/// Includes Flatpak export paths so apps installed via Flatpak are found.
pub(super) fn desktop_entry_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // XDG_DATA_HOME/applications (user apps, highest priority)
    let data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join(".local/share"))
                .unwrap_or_default()
        });
    if !data_home.as_os_str().is_empty() {
        dirs.push(data_home.join("applications"));
    }

    // User Flatpak exports (~/.local/share/flatpak/exports/share/applications)
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    }

    // XDG_DATA_DIRS (system-level; spec default: /usr/local/share:/usr/share)
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for data_dir in xdg_data_dirs.split(':').filter(|s| !s.is_empty()) {
        dirs.push(PathBuf::from(data_dir).join("applications"));
    }

    // System Flatpak exports
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));

    // Deduplicate while preserving priority order
    let mut seen = HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));

    dirs
}

fn mimeapps_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(config) = dirs::config_dir() {
        paths.push(config.join("mimeapps.list"));
    }
    paths.push(PathBuf::from("/usr/local/share/applications/mimeapps.list"));
    paths.push(PathBuf::from("/usr/share/applications/mimeapps.list"));
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── discover_via_desktop_scan_in_dirs (hermetic) ──────────────────────────

    #[test]
    fn desktop_scan_finds_app_by_exact_mime_type() {
        use std::fs;

        let dir =
            std::env::temp_dir().join(format!("elio-test-desktop-scan-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");

        let desktop_content = "\
[Desktop Entry]
Name=Test Editor
Exec=testeditor %f
MimeType=text/plain;
";
        fs::write(dir.join("testeditor.desktop"), desktop_content).expect("write desktop file");

        let target = Path::new("/tmp/hello.txt");
        let apps =
            discover_via_desktop_scan_in_dirs("text/plain", target, std::slice::from_ref(&dir));

        let _ = fs::remove_dir_all(&dir);

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].display_name, "Test Editor");
        assert_eq!(apps[0].program, "testeditor");
        assert_eq!(apps[0].args, vec!["/tmp/hello.txt"]);
    }

    #[test]
    fn desktop_scan_does_not_find_app_by_inherited_mime_type() {
        use std::fs;

        // This test documents the known limitation of the fallback scan:
        // an app that only lists text/plain will NOT be found when scanning
        // for text/markdown (which inherits from text/plain).
        // The gio backend handles this correctly; the desktop scan is the fallback.
        let dir = std::env::temp_dir().join(format!(
            "elio-test-desktop-scan-inherit-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");

        let desktop_content = "\
[Desktop Entry]
Name=Plain Editor
Exec=plaineditor %f
MimeType=text/plain;
";
        fs::write(dir.join("plaineditor.desktop"), desktop_content).expect("write desktop file");

        let target = Path::new("/tmp/notes.md");
        let apps =
            discover_via_desktop_scan_in_dirs("text/markdown", target, std::slice::from_ref(&dir));

        let _ = fs::remove_dir_all(&dir);

        assert!(
            apps.is_empty(),
            "fallback scan must not infer MIME inheritance — that is gio's job"
        );
    }
}
