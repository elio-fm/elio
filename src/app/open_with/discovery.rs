use std::path::{Path, PathBuf};

use super::super::state::OpenWithApp;

// ── platform-gated imports ────────────────────────────────────────────────────

#[cfg(all(unix, not(target_os = "macos")))]
use {
    crate::preview::process::run_command_capture_stdout_cancellable,
    std::collections::{HashMap, HashSet},
    std::process::Command,
    std::time::{Duration, Instant},
};

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
    // 3-second budget for subprocess fallbacks; pure-Rust MIME lookup is
    // instant and is tried first, so the timeout rarely matters in practice.
    let deadline = Instant::now() + Duration::from_millis(3000);
    let canceled = || Instant::now() > deadline;

    let Some(mime) = detect_mime_type(path, &canceled) else {
        return vec![];
    };

    // Primary: gio handles MIME inheritance (e.g. text/markdown → text/plain),
    // aliases, and added/removed associations from mimeapps.list.
    if let Some(apps) = discover_via_gio(&mime, path, &canceled)
        && !apps.is_empty()
    {
        return apps;
    }

    // Fallback: manual desktop-file scan with exact MIME match.
    discover_via_desktop_scan(&mime, path)
}

/// Asks `gio mime <mime>` for the full list of registered applications,
/// including those that handle parent MIME types via inheritance.
///
/// Returns `None` if gio is unavailable or was canceled; returns `Some(vec![])`
/// if gio ran successfully but found no applications.
#[cfg(all(unix, not(target_os = "macos")))]
fn discover_via_gio(
    mime: &str,
    path: &Path,
    canceled: &impl Fn() -> bool,
) -> Option<Vec<OpenWithApp>> {
    let mut cmd = Command::new("gio");
    cmd.args(["mime", mime]);
    let output = run_command_capture_stdout_cancellable(cmd, "open-with-gio", canceled)?;
    let text = String::from_utf8_lossy(&output);

    let entries = parse_gio_mime_output(&text);
    if entries.is_empty() {
        return Some(vec![]);
    }

    let dirs = desktop_entry_dirs();
    let mut apps = Vec::new();
    for (desktop_id, is_default) in entries {
        if let Some(app) = read_desktop_entry_for_id(&desktop_id, &dirs, path, is_default) {
            apps.push(app);
        }
    }

    Some(apps)
}

/// Reads and parses the `.desktop` file for `desktop_id` from the first
/// directory in `dirs` that contains it.
///
/// Once the file is found, the result from that directory is final — a
/// higher-priority entry that is hidden or missing fields wins over a
/// lower-priority entry that would be valid.
#[cfg(all(unix, not(target_os = "macos")))]
fn read_desktop_entry_for_id(
    desktop_id: &str,
    dirs: &[PathBuf],
    target: &Path,
    is_default: bool,
) -> Option<OpenWithApp> {
    for dir in dirs {
        let entry_path = dir.join(desktop_id);
        let Ok(contents) = std::fs::read_to_string(&entry_path) else {
            continue;
        };
        // File found in this directory — stop searching regardless of outcome.
        let candidate = parse_desktop_entry(&contents)?;
        let (program, args) = expand_exec_template(&candidate.exec, target)?;
        return Some(OpenWithApp {
            display_name: candidate.name,
            desktop_id: Some(desktop_id.to_string()),
            program,
            args,
            is_default,
        });
    }
    None
}

/// Manual desktop-file scan: walks all desktop entry directories and returns
/// apps that explicitly list `mime` in their `MimeType=` field.
/// Used as a fallback when `gio` is unavailable.
#[cfg(all(unix, not(target_os = "macos")))]
fn discover_via_desktop_scan(mime: &str, path: &Path) -> Vec<OpenWithApp> {
    discover_via_desktop_scan_in_dirs(mime, path, &desktop_entry_dirs())
}

/// Inner scan that accepts an explicit directory list (allows hermetic testing).
#[cfg(all(unix, not(target_os = "macos")))]
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
        });
    }

    apps
}

#[cfg(all(unix, not(target_os = "macos")))]
fn detect_mime_type(path: &Path, canceled: &impl Fn() -> bool) -> Option<String> {
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
#[cfg(all(unix, not(target_os = "macos")))]
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

/// Returns the ordered list of directories to search for `.desktop` files,
/// from highest to lowest priority, following the XDG Base Dir spec.
///
/// Includes Flatpak export paths so apps installed via Flatpak are found.
#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_entry_dirs() -> Vec<PathBuf> {
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

#[cfg(all(unix, not(target_os = "macos")))]
fn mimeapps_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(config) = dirs::config_dir() {
        paths.push(config.join("mimeapps.list"));
    }
    paths.push(PathBuf::from("/usr/local/share/applications/mimeapps.list"));
    paths.push(PathBuf::from("/usr/share/applications/mimeapps.list"));
    paths
}

// ── pure parsing helpers (always compiled) ────────────────────────────────────

struct DesktopEntryCandidate {
    name: String,
    exec: String,
    mime_types: Vec<String>,
}

/// Parses the output of `gio mime <mime-type>` into an ordered list of
/// `(desktop_id, is_default)` pairs.
///
/// The default application (if any) is placed first with `is_default = true`.
/// Subsequent entries from the Registered/Recommended sections follow in
/// first-seen order, deduplicated.  If an entry appears in both sections it
/// is emitted once (at the position it was first seen).
fn parse_gio_mime_output(text: &str) -> Vec<(String, bool)> {
    let mut default_app: Option<String> = None;
    let mut ordered: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in text.lines() {
        // gio mime prints the default application line in one of two formats
        // depending on the platform and GLib version:
        //   ASCII quotes:   Default application for 'mime/type': app.desktop
        //   Curly quotes:   Default application for \u{201C}mime/type\u{201D}: app.desktop
        // We strip the known prefix and then find the closing quote + ": " separator,
        // trying the curly-quote form first since that is what GNOME uses.
        if line.starts_with("Default application for ") {
            let desktop_id = line
                .find("\u{201D}: ")
                .map(|i| &line[i + "\u{201D}: ".len()..])
                .or_else(|| line.find("': ").map(|i| &line[i + 3..]))
                .or_else(|| line.find("\": ").map(|i| &line[i + 3..]))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if let Some(id) = desktop_id {
                default_app = Some(id);
            }
            continue;
        }
        // Tab-indented lines are desktop IDs in Registered / Recommended sections.
        if line.starts_with('\t') {
            let desktop_id = line.trim().to_string();
            if !desktop_id.is_empty() && seen.insert(desktop_id.clone()) {
                ordered.push(desktop_id);
            }
        }
    }

    let mut result: Vec<(String, bool)> = Vec::new();
    if let Some(ref def) = default_app {
        result.push((def.clone(), true));
    }
    for id in ordered {
        if default_app.as_deref() != Some(&id) {
            result.push((id, false));
        }
    }

    result
}

/// Returns the ordered list of desktop-ids from the `[Default Applications]`
/// section of a mimeapps.list file for the given MIME type.
fn parse_mimeapps_defaults(contents: &str, mime: &str) -> Vec<String> {
    let mut in_section = false;
    let mut result = Vec::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == "[Default Applications]";
            continue;
        }
        if !in_section || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == mime
        {
            result = value
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
        }
    }

    result
}

/// Parses a .desktop file and returns a `DesktopEntryCandidate` if the entry
/// is visible (not Hidden/NoDisplay) and has both `Name` and `Exec`.
fn parse_desktop_entry(contents: &str) -> Option<DesktopEntryCandidate> {
    let mut in_entry = false;
    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut mime_types: Vec<String> = Vec::new();
    let mut hidden = false;
    let mut no_display = false;

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') || line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            // Only accept the unlocalized Name= (localized keys have the form
            // Name[de]=…, whose key contains '[').
            "Name" => {
                if name.is_none() {
                    name = Some(value.to_string());
                }
            }
            "Exec" => exec = Some(value.to_string()),
            "MimeType" => {
                mime_types = value
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }

    if hidden || no_display {
        return None;
    }

    Some(DesktopEntryCandidate {
        name: name?,
        exec: exec?,
        mime_types,
    })
}

/// Expands the `Exec=` field from a .desktop file into `(program, args)`.
///
/// Supported placeholders: `%f`, `%F`, `%u`, `%U` → replaced with the target
/// file path.  `%i`, `%c`, `%k` are stripped.  Unknown `%x` sequences are
/// dropped.
pub(super) fn expand_exec_template(exec: &str, target: &Path) -> Option<(String, Vec<String>)> {
    let target_str = target.to_str()?;
    let tokens = tokenize_exec(exec);

    let mut expanded: Vec<String> = Vec::new();
    for token in tokens {
        match token.as_str() {
            // Strip deprecated / icon / class / location placeholders.
            "%i" | "%c" | "%k" => {}
            // Standalone file/URL placeholders — replace with the single target.
            "%f" | "%F" | "%u" | "%U" => expanded.push(target_str.to_string()),
            other => {
                // Replace known placeholders embedded inside a larger token
                // (e.g. --file=%f), then strip any remaining unknown %x codes
                // so they are never passed to the child process.
                let replaced = other
                    .replace("%f", target_str)
                    .replace("%F", target_str)
                    .replace("%u", target_str)
                    .replace("%U", target_str)
                    .replace("%i", "")
                    .replace("%c", "")
                    .replace("%k", "");
                let clean = strip_unknown_field_codes(&replaced);
                if !clean.is_empty() {
                    expanded.push(clean);
                }
            }
        }
    }

    if expanded.is_empty() {
        return None;
    }

    let program = expanded.remove(0);
    Some((program, expanded))
}

/// Removes any `%x` field codes that were not already handled, so they are
/// never forwarded to the child process.  `%%` is converted to a literal `%`.
fn strip_unknown_field_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            match chars.peek() {
                Some('%') => {
                    chars.next();
                    result.push('%');
                }
                Some(_) => {
                    chars.next(); // drop %x
                }
                None => {} // trailing bare % — drop it
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Splits a desktop-spec Exec string into tokens, respecting double-quoted
/// strings and backslash escapes.
fn tokenize_exec(exec: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = exec.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── parse_gio_mime_output ─────────────────────────────────────────────────

    #[test]
    fn parse_gio_mime_output_extracts_default_and_registered_curly_quotes() {
        // GNOME gio uses Unicode curly double-quotes (U+201C / U+201D) around
        // the MIME type in the Default application line.
        let output = "Default application for \u{201C}text/markdown\u{201D}: org.gnome.TextEditor.desktop\nRegistered applications:\n\tcode.desktop\n\torg.gnome.TextEditor.desktop\nRecommended applications:\n\tcode.desktop\n\torg.gnome.TextEditor.desktop\n";
        let result = parse_gio_mime_output(output);

        // Default must come first, marked as default.
        assert_eq!(
            result[0],
            ("org.gnome.TextEditor.desktop".to_string(), true)
        );
        // code.desktop appears in Registered (first), not again from Recommended.
        assert_eq!(result[1], ("code.desktop".to_string(), false));
        assert_eq!(result.len(), 2, "default + one non-default, no duplicates");
    }

    #[test]
    fn parse_gio_mime_output_extracts_default_and_registered_ascii_quotes() {
        // Older gio / non-GNOME builds may use ASCII single quotes.
        let output = "\
Default application for 'text/markdown': org.gnome.TextEditor.desktop
Registered applications:
\tcode.desktop
\torg.gnome.TextEditor.desktop
";
        let result = parse_gio_mime_output(output);
        assert_eq!(
            result[0],
            ("org.gnome.TextEditor.desktop".to_string(), true)
        );
        assert_eq!(result[1], ("code.desktop".to_string(), false));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_gio_mime_output_no_default_returns_registered_only() {
        let output = "No default applications for \u{201C}application/octet-stream\u{201D}\nRegistered applications:\n\tfoo.desktop\n\tbar.desktop\n";
        let result = parse_gio_mime_output(output);
        assert_eq!(
            result,
            vec![
                ("foo.desktop".to_string(), false),
                ("bar.desktop".to_string(), false),
            ]
        );
    }

    #[test]
    fn parse_gio_mime_output_empty_when_no_apps() {
        let output = "No default applications for \u{201C}application/x-unknown\u{201D}\nNo registered applications\nNo recommended applications\n";
        let result = parse_gio_mime_output(output);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_gio_mime_output_deduplicates_across_sections() {
        // code.desktop appears in both Registered and Recommended — should appear once.
        // kate.desktop appears only in Recommended — should still be included.
        let output = "Default application for \u{201C}text/plain\u{201D}: gedit.desktop\nRegistered applications:\n\tgedit.desktop\n\tcode.desktop\nRecommended applications:\n\tcode.desktop\n\tkate.desktop\n";
        let result = parse_gio_mime_output(output);

        assert_eq!(result[0], ("gedit.desktop".to_string(), true));

        let ids: Vec<&str> = result.iter().map(|(id, _)| id.as_str()).collect();
        assert!(
            ids.contains(&"code.desktop"),
            "code.desktop should be present"
        );
        assert!(
            ids.contains(&"kate.desktop"),
            "kate.desktop should be present"
        );
        assert_eq!(
            result.len(),
            3,
            "gedit(default) + code + kate, no duplicates"
        );

        // Verify none are marked is_default except the first.
        for (_, is_default) in &result[1..] {
            assert!(
                !is_default,
                "only the default entry should have is_default=true"
            );
        }
    }

    #[test]
    fn parse_gio_mime_output_default_not_in_registered_section() {
        // The default app is listed only in the "Default application" line,
        // not in Registered/Recommended.  It must still appear in results.
        let output = "Default application for \u{201C}image/png\u{201D}: eog.desktop\nRegistered applications:\n\tfeh.desktop\n";
        let result = parse_gio_mime_output(output);
        assert_eq!(result[0], ("eog.desktop".to_string(), true));
        assert_eq!(result[1], ("feh.desktop".to_string(), false));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_gio_mime_output_handles_empty_input() {
        let result = parse_gio_mime_output("");
        assert!(result.is_empty());
    }

    // ── parse_mimeapps_defaults ───────────────────────────────────────────────

    #[test]
    fn parse_mimeapps_defaults_picks_matching_section_entries() {
        let contents = "\
[Added Associations]
text/plain=kate.desktop;

[Default Applications]
image/png=eog.desktop;feh.desktop;
text/plain=gedit.desktop;nano.desktop;

[Removed Associations]
text/plain=vi.desktop;
";
        let result = parse_mimeapps_defaults(contents, "text/plain");
        assert_eq!(result, vec!["gedit.desktop", "nano.desktop"]);
    }

    #[test]
    fn parse_mimeapps_defaults_returns_empty_for_unknown_mime() {
        let contents = "\
[Default Applications]
image/png=eog.desktop;
";
        let result = parse_mimeapps_defaults(contents, "text/plain");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_mimeapps_defaults_ignores_other_sections() {
        // text/plain appears in [Added Associations] but NOT [Default Applications].
        let contents = "\
[Added Associations]
text/plain=kate.desktop;

[Default Applications]
image/png=eog.desktop;
";
        let result = parse_mimeapps_defaults(contents, "text/plain");
        assert!(result.is_empty());
    }

    // ── parse_desktop_entry ───────────────────────────────────────────────────

    #[test]
    fn parse_desktop_entry_returns_valid_entry() {
        let contents = "\
[Desktop Entry]
Name=Text Editor
Exec=gedit %f
MimeType=text/plain;text/x-readme;
";
        let entry = parse_desktop_entry(contents).expect("should parse");
        assert_eq!(entry.name, "Text Editor");
        assert_eq!(entry.exec, "gedit %f");
        assert!(entry.mime_types.contains(&"text/plain".to_string()));
    }

    #[test]
    fn parse_desktop_entry_skips_hidden_and_nodisplay() {
        let hidden = "\
[Desktop Entry]
Name=Hidden App
Exec=hidden %f
MimeType=text/plain;
Hidden=true
";
        assert!(
            parse_desktop_entry(hidden).is_none(),
            "Hidden=true should be skipped"
        );

        let no_display = "\
[Desktop Entry]
Name=Background Tool
Exec=tool %f
MimeType=text/plain;
NoDisplay=true
";
        assert!(
            parse_desktop_entry(no_display).is_none(),
            "NoDisplay=true should be skipped"
        );
    }

    #[test]
    fn parse_desktop_entry_ignores_localized_name() {
        let contents = "\
[Desktop Entry]
Name=Plain Name
Name[de]=Deutsch Name
Exec=app %f
MimeType=text/plain;
";
        let entry = parse_desktop_entry(contents).expect("should parse");
        assert_eq!(entry.name, "Plain Name");
    }

    #[test]
    fn parse_desktop_entry_returns_none_without_exec() {
        let contents = "\
[Desktop Entry]
Name=Broken App
MimeType=text/plain;
";
        assert!(parse_desktop_entry(contents).is_none());
    }

    #[test]
    fn parse_desktop_entry_returns_none_without_name() {
        let contents = "\
[Desktop Entry]
Exec=app %f
MimeType=text/plain;
";
        assert!(parse_desktop_entry(contents).is_none());
    }

    // ── expand_exec_template ──────────────────────────────────────────────────

    #[test]
    fn expand_exec_template_supports_percent_f_and_percent_u() {
        let path = Path::new("/home/user/doc.txt");

        let (prog, args) = expand_exec_template("gedit %f", path).expect("should expand");
        assert_eq!(prog, "gedit");
        assert_eq!(args, vec!["/home/user/doc.txt"]);

        let (prog, args) = expand_exec_template("vlc %u", path).expect("should expand");
        assert_eq!(prog, "vlc");
        assert_eq!(args, vec!["/home/user/doc.txt"]);
    }

    #[test]
    fn expand_exec_template_supports_uppercase_percent_f_and_percent_u() {
        let path = Path::new("/tmp/file.png");

        let (prog, args) = expand_exec_template("eog %F", path).expect("should expand");
        assert_eq!(prog, "eog");
        assert_eq!(args, vec!["/tmp/file.png"]);

        let (prog, args) = expand_exec_template("vlc %U", path).expect("should expand");
        assert_eq!(prog, "vlc");
        assert_eq!(args, vec!["/tmp/file.png"]);
    }

    #[test]
    fn expand_exec_template_strips_percent_i_percent_c_percent_k() {
        let path = Path::new("/tmp/x.txt");

        // %i, %c, %k as standalone tokens — should all be dropped.
        let (prog, args) = expand_exec_template("nano %i %c %k %f", path).expect("should expand");
        assert_eq!(prog, "nano");
        assert_eq!(args, vec!["/tmp/x.txt"]);
    }

    #[test]
    fn expand_exec_template_handles_embedded_placeholder() {
        let path = Path::new("/tmp/image.png");

        let (prog, args) =
            expand_exec_template("viewer --file=%f --quality=90", path).expect("should expand");
        assert_eq!(prog, "viewer");
        assert_eq!(args, vec!["--file=/tmp/image.png", "--quality=90"]);
    }

    #[test]
    fn expand_exec_template_handles_quoted_program() {
        let path = Path::new("/tmp/doc.txt");

        let (prog, args) = expand_exec_template(r#""my editor" %f"#, path).expect("should expand");
        assert_eq!(prog, "my editor");
        assert_eq!(args, vec!["/tmp/doc.txt"]);
    }

    #[test]
    fn expand_exec_template_returns_none_for_empty_after_strip() {
        let path = Path::new("/tmp/x");
        // Only stripped placeholders — nothing left.
        let result = expand_exec_template("%i %c %k", path);
        assert!(result.is_none());
    }

    #[test]
    fn expand_exec_template_drops_unknown_placeholders() {
        let path = Path::new("/tmp/doc.txt");

        // %d, %n, %D, %v, %m are deprecated/unknown — must not pass through.
        let (prog, args) =
            expand_exec_template("app %d %n %f", path).expect("should expand with file arg");
        assert_eq!(prog, "app");
        assert_eq!(args, vec!["/tmp/doc.txt"]);
    }

    #[test]
    fn expand_exec_template_handles_embedded_unknown_placeholder() {
        let path = Path::new("/tmp/img.png");

        // An embedded unknown code like %v inside an option should be stripped,
        // not forwarded to the program.
        let (prog, args) = expand_exec_template("viewer --opt=%v %f", path).expect("should expand");
        assert_eq!(prog, "viewer");
        // "--opt=" is not empty so it remains; file arg is expanded normally.
        assert_eq!(args, vec!["--opt=", "/tmp/img.png"]);
    }

    #[test]
    fn expand_exec_template_converts_double_percent_to_literal() {
        let path = Path::new("/tmp/file");

        let (prog, args) =
            expand_exec_template("app --label=100%% %f", path).expect("should expand");
        assert_eq!(prog, "app");
        assert_eq!(args, vec!["--label=100%", "/tmp/file"]);
    }

    // ── parse_mimeapps_defaults ordering ─────────────────────────────────────

    #[test]
    fn parse_mimeapps_defaults_skips_file_that_lacks_mime_entry() {
        // Simulate ~/.config/mimeapps.list that only overrides image/png.
        let user_file = "\
[Default Applications]
image/png=eog.desktop;
";
        // Simulate /usr/share/applications/mimeapps.list with text/plain.
        let system_file = "\
[Default Applications]
text/plain=gedit.desktop;
";
        // The bug: if we just find_map on readable files, the user file is
        // returned immediately (it's readable) even though it has no entry for
        // text/plain.  The fix returns None for files with no matching entry,
        // so the search continues to the system file.
        let result_user = parse_mimeapps_defaults(user_file, "text/plain");
        assert!(
            result_user.is_empty(),
            "user file has no text/plain entry — should return empty"
        );

        let result_system = parse_mimeapps_defaults(system_file, "text/plain");
        assert_eq!(result_system, vec!["gedit.desktop"]);
    }

    // ── mime_from_xdg_database ────────────────────────────────────────────────
    // These tests run only on supported platforms since the helper is gated.

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn mime_from_xdg_database_returns_expected_type_for_common_extensions() {
        use std::path::Path;
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

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn mime_from_xdg_database_returns_none_for_unknown_extension() {
        let result = mime_from_xdg_database(std::path::Path::new("/tmp/file.xyzzy_unknown_ext"));
        assert!(result.is_none(), "unknown extension should return None");
    }

    // ── discover_via_desktop_scan_in_dirs (hermetic) ──────────────────────────

    #[cfg(all(unix, not(target_os = "macos")))]
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

    #[cfg(all(unix, not(target_os = "macos")))]
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
