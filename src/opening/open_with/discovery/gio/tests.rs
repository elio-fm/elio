use super::*;

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

// ── candidate_paths_for_desktop_id ────────────────────────────────────────

#[test]
fn candidate_paths_no_dash_returns_flat_path() {
    let base = Path::new("/usr/share/applications");
    let paths = candidate_paths_for_desktop_id(base, "gedit.desktop");
    assert_eq!(paths, vec![base.join("gedit.desktop")]);
}

#[test]
fn candidate_paths_one_dash_returns_flat_then_nested() {
    let base = Path::new("/usr/share/applications");
    let paths = candidate_paths_for_desktop_id(base, "kde-konsole.desktop");
    assert_eq!(
        paths,
        vec![
            base.join("kde-konsole.desktop"),
            base.join("kde/konsole.desktop"),
        ]
    );
}

#[test]
fn candidate_paths_two_dashes_returns_all_splits() {
    let base = Path::new("/usr/share/applications");
    let paths = candidate_paths_for_desktop_id(base, "org-kde-konsole.desktop");
    assert_eq!(
        paths,
        vec![
            base.join("org-kde-konsole.desktop"),
            base.join("org/kde-konsole.desktop"),
            base.join("org/kde/konsole.desktop"),
        ]
    );
}

// ── read_desktop_entry_for_id (nested path resolution) ───────────────────

#[test]
fn reads_nested_desktop_file_via_hyphenated_id() {
    use std::fs;

    // Build a temp applications dir with kde/konsole.desktop at the
    // nested path — simulating how packages like kde-konsole install.
    let base = std::env::temp_dir().join(format!("elio-gio-nest-test-{}", std::process::id()));
    let nested_dir = base.join("kde");
    fs::create_dir_all(&nested_dir).unwrap();
    fs::write(
        nested_dir.join("konsole.desktop"),
        "[Desktop Entry]\nName=Konsole\nExec=konsole %u\nMimeType=text/plain;\n",
    )
    .unwrap();

    let result = read_desktop_entry_for_id(
        "kde-konsole.desktop",
        std::slice::from_ref(&base),
        Path::new("/tmp/test.txt"),
        false,
        &[],
    );
    let _ = fs::remove_dir_all(&base);

    let app = result.expect("should find kde/konsole.desktop via kde-konsole.desktop id");
    assert_eq!(app.display_name, "Konsole");
    assert_eq!(app.desktop_id.as_deref(), Some("kde-konsole.desktop"));
}
