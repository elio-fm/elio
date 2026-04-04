pub(super) struct DesktopEntryCandidate {
    pub(super) name: String,
    pub(super) exec: String,
    pub(super) mime_types: Vec<String>,
    pub(super) terminal: bool,
}

/// Returns the ordered list of desktop-ids from the `[Default Applications]`
/// section of a mimeapps.list file for the given MIME type.
pub(super) fn parse_mimeapps_defaults(contents: &str, mime: &str) -> Vec<String> {
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
pub(super) fn parse_desktop_entry(contents: &str) -> Option<DesktopEntryCandidate> {
    let mut in_entry = false;
    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut mime_types: Vec<String> = Vec::new();
    let mut hidden = false;
    let mut no_display = false;
    let mut terminal = false;

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
            "Terminal" => terminal = value.eq_ignore_ascii_case("true"),
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
        terminal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_desktop_entry_marks_terminal_apps() {
        let contents = "\
[Desktop Entry]
Name=Neovim
Exec=nvim %F
MimeType=text/plain;
Terminal=true
";
        let entry = parse_desktop_entry(contents).expect("should parse");
        assert!(entry.terminal, "Terminal=true should be preserved");
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
}
