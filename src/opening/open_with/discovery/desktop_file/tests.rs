use super::*;

// ── parse_mimeapps_removed ────────────────────────────────────────────────

#[test]
fn parse_mimeapps_removed_returns_removed_ids() {
    let contents = "\
[Default Applications]
text/plain=gedit.desktop;

[Removed Associations]
text/plain=vi.desktop;legacy.desktop;
image/png=display.desktop;
";
    let result = parse_mimeapps_removed(contents, "text/plain");
    assert_eq!(result, vec!["vi.desktop", "legacy.desktop"]);
}

#[test]
fn parse_mimeapps_removed_returns_empty_for_unknown_mime() {
    let contents = "\
[Removed Associations]
image/png=display.desktop;
";
    let result = parse_mimeapps_removed(contents, "text/plain");
    assert!(result.is_empty());
}

#[test]
fn parse_mimeapps_removed_ignores_other_sections() {
    let contents = "\
[Added Associations]
text/plain=vi.desktop;

[Default Applications]
text/plain=vi.desktop;
";
    let result = parse_mimeapps_removed(contents, "text/plain");
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

#[test]
fn parse_mimeapps_defaults_skips_file_that_lacks_mime_entry() {
    let user_file = "\
[Default Applications]
image/png=eog.desktop;
";
    let system_file = "\
[Default Applications]
text/plain=gedit.desktop;
";
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

#[test]
fn parse_desktop_entry_parses_only_show_in_and_not_show_in() {
    let contents = "\
[Desktop Entry]
Name=GNOME Tool
Exec=tool %f
MimeType=text/plain;
OnlyShowIn=GNOME;Unity;
NotShowIn=KDE;
";
    let entry = parse_desktop_entry(contents).expect("should parse");
    assert_eq!(entry.only_show_in, vec!["GNOME", "Unity"]);
    assert_eq!(entry.not_show_in, vec!["KDE"]);
}

// ── is_shown_in ───────────────────────────────────────────────────────────

fn make_candidate(only_show_in: &[&str], not_show_in: &[&str]) -> DesktopEntryCandidate {
    DesktopEntryCandidate {
        name: "Test".to_string(),
        exec: "test %f".to_string(),
        mime_types: vec![],
        terminal: false,
        only_show_in: only_show_in.iter().map(|s| s.to_string()).collect(),
        not_show_in: not_show_in.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn is_shown_in_allows_app_with_no_constraints() {
    let c = make_candidate(&[], &[]);
    assert!(c.is_shown_in(&["GNOME".to_string()]));
    assert!(c.is_shown_in(&[]));
}

#[test]
fn is_shown_in_allows_app_when_current_desktop_matches_only_show_in() {
    let c = make_candidate(&["GNOME", "Unity"], &[]);
    assert!(c.is_shown_in(&["GNOME".to_string()]));
    assert!(c.is_shown_in(&["KDE".to_string(), "GNOME".to_string()]));
}

#[test]
fn is_shown_in_blocks_app_when_current_desktop_not_in_only_show_in() {
    let c = make_candidate(&["GNOME"], &[]);
    assert!(!c.is_shown_in(&["KDE".to_string()]));
    assert!(!c.is_shown_in(&["XFCE".to_string(), "LXQt".to_string()]));
}

#[test]
fn is_shown_in_blocks_app_listed_in_not_show_in() {
    let c = make_candidate(&[], &["KDE"]);
    assert!(!c.is_shown_in(&["KDE".to_string()]));
    assert!(!c.is_shown_in(&["GNOME".to_string(), "KDE".to_string()]));
}

#[test]
fn is_shown_in_allows_app_when_not_show_in_does_not_match() {
    let c = make_candidate(&[], &["KDE"]);
    assert!(c.is_shown_in(&["GNOME".to_string()]));
}

#[test]
fn is_shown_in_comparison_is_case_insensitive() {
    let c = make_candidate(&["GNOME"], &["KDE"]);
    assert!(c.is_shown_in(&["gnome".to_string()]));
    assert!(!c.is_shown_in(&["kde".to_string()]));
}

#[test]
fn is_shown_in_allows_all_when_desktop_is_unknown() {
    // When XDG_CURRENT_DESKTOP is unset, desktops is empty.
    // Even an app with OnlyShowIn constraints passes through.
    let c = make_candidate(&["GNOME"], &["KDE"]);
    assert!(c.is_shown_in(&[]));
}
