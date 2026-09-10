use super::*;

fn unique_dir(label: &str) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    std::env::temp_dir().join(format!(
        "elio-scan-test-{label}-{}-{nanos}",
        std::process::id()
    ))
}

// ── applications_for_in_dirs ─────────────────────────────────────

#[test]
fn desktop_scan_finds_app_by_exact_mime_type() {
    use std::fs;
    let dir = unique_dir("flat");
    fs::create_dir_all(&dir).expect("create temp dir");

    fs::write(
        dir.join("testeditor.desktop"),
        "[Desktop Entry]\nName=Test Editor\nExec=testeditor %f\nMimeType=text/plain;\n",
    )
    .expect("write desktop file");

    let apps = applications_for_in_dirs(
        "text/plain",
        Path::new("/tmp/hello.txt"),
        std::slice::from_ref(&dir),
    );
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].display_name, "Test Editor");
    assert_eq!(apps[0].program, "testeditor");
    assert_eq!(apps[0].args, vec!["/tmp/hello.txt"]);
}

#[test]
fn desktop_scan_does_not_find_app_by_inherited_mime_type() {
    use std::fs;
    // Documents that the fallback scan does exact matching only.
    // An app listing text/plain will NOT be found for text/markdown.
    let dir = unique_dir("inherit");
    fs::create_dir_all(&dir).expect("create temp dir");

    fs::write(
        dir.join("plaineditor.desktop"),
        "[Desktop Entry]\nName=Plain Editor\nExec=plaineditor %f\nMimeType=text/plain;\n",
    )
    .expect("write desktop file");

    let apps = applications_for_in_dirs(
        "text/markdown",
        Path::new("/tmp/notes.md"),
        std::slice::from_ref(&dir),
    );
    let _ = fs::remove_dir_all(&dir);

    assert!(
        apps.is_empty(),
        "fallback scan must not infer MIME inheritance — that is gio's job"
    );
}

// ── collect_desktop_entries / desktop-id derivation ───────────────────────

#[test]
fn flat_desktop_file_gets_basename_as_desktop_id() {
    use std::fs;
    let dir = unique_dir("flat-id");
    fs::create_dir_all(&dir).expect("create dir");
    fs::write(dir.join("gedit.desktop"), "").expect("write file");

    let entries = collect_desktop_entries(&dir);
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, "gedit.desktop");
}

#[test]
fn subdirectory_desktop_file_gets_path_joined_with_dash() {
    use std::fs;
    let dir = unique_dir("subdir-id");
    fs::create_dir_all(dir.join("kde")).expect("create subdir");
    fs::write(dir.join("kde/konsole.desktop"), "").expect("write file");

    let entries = collect_desktop_entries(&dir);
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, "kde-konsole.desktop");
}

#[test]
fn deeply_nested_desktop_file_gets_full_path_as_id() {
    use std::fs;
    let dir = unique_dir("deep-id");
    fs::create_dir_all(dir.join("a/b")).expect("create deep dirs");
    fs::write(dir.join("a/b/app.desktop"), "").expect("write file");

    let entries = collect_desktop_entries(&dir);
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, "a-b-app.desktop");
}

#[test]
fn desktop_scan_finds_app_in_subdirectory() {
    use std::fs;
    let dir = unique_dir("subdir-scan");
    fs::create_dir_all(dir.join("kde")).expect("create subdir");

    fs::write(
        dir.join("kde/konsole.desktop"),
        "[Desktop Entry]\nName=Konsole\nExec=konsole %f\nMimeType=text/plain;\n",
    )
    .expect("write desktop file");

    let apps = applications_for_in_dirs(
        "text/plain",
        Path::new("/tmp/hello.txt"),
        std::slice::from_ref(&dir),
    );
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].display_name, "Konsole");
    assert_eq!(
        apps[0].application_id.as_deref(),
        Some("kde-konsole.desktop"),
        "desktop-id must use '-' not '/'"
    );
}

#[test]
fn higher_priority_dir_wins_for_same_desktop_id() {
    use std::fs;
    let dir1 = unique_dir("priority-high");
    let dir2 = unique_dir("priority-low");
    fs::create_dir_all(&dir1).expect("create dir1");
    fs::create_dir_all(&dir2).expect("create dir2");

    fs::write(
        dir1.join("app.desktop"),
        "[Desktop Entry]\nName=High Priority\nExec=app %f\nMimeType=text/plain;\n",
    )
    .expect("write high");
    fs::write(
        dir2.join("app.desktop"),
        "[Desktop Entry]\nName=Low Priority\nExec=app %f\nMimeType=text/plain;\n",
    )
    .expect("write low");

    let apps = applications_for_in_dirs(
        "text/plain",
        Path::new("/tmp/file.txt"),
        &[dir1.clone(), dir2.clone()],
    );
    let _ = fs::remove_dir_all(&dir1);
    let _ = fs::remove_dir_all(&dir2);

    assert_eq!(
        apps.len(),
        1,
        "same desktop-id should not produce duplicates"
    );
    assert_eq!(apps[0].display_name, "High Priority");
}

#[test]
fn removed_associations_are_filtered_out() {
    use std::fs;
    let apps_dir = unique_dir("removed-assoc");
    let mime_dir = unique_dir("removed-assoc-mime");
    fs::create_dir_all(&apps_dir).expect("create apps dir");
    fs::create_dir_all(&mime_dir).expect("create mime dir");

    fs::write(
        apps_dir.join("suppressed.desktop"),
        "[Desktop Entry]\nName=Suppressed\nExec=suppressed %f\nMimeType=text/plain;\n",
    )
    .expect("write suppressed desktop");
    fs::write(
        apps_dir.join("allowed.desktop"),
        "[Desktop Entry]\nName=Allowed\nExec=allowed %f\nMimeType=text/plain;\n",
    )
    .expect("write allowed desktop");
    let mimeapps = mime_dir.join("mimeapps.list");
    fs::write(
        &mimeapps,
        "[Removed Associations]\ntext/plain=suppressed.desktop;\n",
    )
    .expect("write mimeapps.list");

    let apps = applications_for_in_paths(
        "text/plain",
        Path::new("/tmp/file.txt"),
        std::slice::from_ref(&apps_dir),
        std::slice::from_ref(&mimeapps),
    );
    let _ = fs::remove_dir_all(&apps_dir);
    let _ = fs::remove_dir_all(&mime_dir);

    let names: Vec<&str> = apps.iter().map(|a| a.display_name.as_str()).collect();
    assert!(
        !names.contains(&"Suppressed"),
        "suppressed.desktop must be filtered out by [Removed Associations]"
    );
    assert!(
        names.contains(&"Allowed"),
        "allowed.desktop must still appear"
    );
}

#[test]
fn only_first_default_gets_is_default_true() {
    use std::fs;
    let apps_dir = unique_dir("multi-default");
    let mime_dir = unique_dir("multi-default-mime");
    fs::create_dir_all(&apps_dir).expect("create apps dir");
    fs::create_dir_all(&mime_dir).expect("create mime dir");

    for name in &["alpha", "beta", "gamma"] {
        fs::write(
            apps_dir.join(format!("{name}.desktop")),
            format!(
                "[Desktop Entry]\nName={cap}\nExec={name} %f\nMimeType=text/plain;\n",
                cap = name[..1].to_uppercase() + &name[1..]
            ),
        )
        .expect("write desktop");
    }
    let mimeapps = mime_dir.join("mimeapps.list");
    fs::write(
        &mimeapps,
        "[Default Applications]\ntext/plain=alpha.desktop;beta.desktop;gamma.desktop;\n",
    )
    .expect("write mimeapps.list");

    let apps = applications_for_in_paths(
        "text/plain",
        Path::new("/tmp/file.txt"),
        std::slice::from_ref(&apps_dir),
        std::slice::from_ref(&mimeapps),
    );
    let _ = fs::remove_dir_all(&apps_dir);
    let _ = fs::remove_dir_all(&mime_dir);

    assert_eq!(apps.len(), 3);
    let defaults: Vec<_> = apps.iter().filter(|a| a.is_default).collect();
    assert_eq!(
        defaults.len(),
        1,
        "exactly one app should be is_default=true"
    );
    assert_eq!(
        defaults[0].display_name, "Alpha",
        "first declared default wins"
    );
}

#[test]
fn non_desktop_files_in_scan_dirs_are_ignored() {
    use std::fs;
    let dir = unique_dir("non-desktop");
    fs::create_dir_all(&dir).expect("create dir");
    fs::write(dir.join("README.md"), "not a desktop file").expect("write readme");
    fs::write(dir.join("app.txt"), "also not a desktop file").expect("write txt");
    fs::write(
        dir.join("real.desktop"),
        "[Desktop Entry]\nName=Real\nExec=real %f\nMimeType=text/plain;\n",
    )
    .expect("write desktop");

    let apps = applications_for_in_dirs(
        "text/plain",
        Path::new("/tmp/file.txt"),
        std::slice::from_ref(&dir),
    );
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].display_name, "Real");
}

#[test]
fn parse_mimeapps_removed_returns_removed_ids() {
    let contents = "\
[Default Applications]
text/plain=gedit.desktop;

[Removed Associations]
text/plain=vi.desktop;legacy.desktop;
image/png=display.desktop;
";
    assert_eq!(
        parse_mimeapps_removed(contents, "text/plain"),
        vec!["vi.desktop", "legacy.desktop"]
    );
}

#[test]
fn parse_mimeapps_removed_returns_empty_for_unknown_mime() {
    let contents = "\
[Removed Associations]
image/png=display.desktop;
";
    assert!(parse_mimeapps_removed(contents, "text/plain").is_empty());
}

#[test]
fn parse_mimeapps_removed_ignores_other_sections() {
    let contents = "\
[Added Associations]
text/plain=vi.desktop;

[Default Applications]
text/plain=vi.desktop;
";
    assert!(parse_mimeapps_removed(contents, "text/plain").is_empty());
}

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
    assert_eq!(
        parse_mimeapps_defaults(contents, "text/plain"),
        vec!["gedit.desktop", "nano.desktop"]
    );
}

#[test]
fn parse_mimeapps_defaults_returns_empty_for_unknown_mime() {
    let contents = "\
[Default Applications]
image/png=eog.desktop;
";
    assert!(parse_mimeapps_defaults(contents, "text/plain").is_empty());
}

#[test]
fn parse_mimeapps_defaults_ignores_other_sections() {
    let contents = "\
[Added Associations]
text/plain=kate.desktop;

[Default Applications]
image/png=eog.desktop;
";
    assert!(parse_mimeapps_defaults(contents, "text/plain").is_empty());
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
    assert!(parse_mimeapps_defaults(user_file, "text/plain").is_empty());
    assert_eq!(
        parse_mimeapps_defaults(system_file, "text/plain"),
        vec!["gedit.desktop"]
    );
}

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
    assert!(entry.terminal);
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
    assert!(parse_desktop_entry(hidden).is_none());

    let no_display = "\
[Desktop Entry]
Name=Background Tool
Exec=tool %f
MimeType=text/plain;
NoDisplay=true
";
    assert!(parse_desktop_entry(no_display).is_none());
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

fn candidate(only_show_in: &[&str], not_show_in: &[&str]) -> DesktopEntryCandidate {
    DesktopEntryCandidate {
        name: "Test".to_string(),
        exec: "test %f".to_string(),
        mime_types: vec![],
        terminal: false,
        only_show_in: only_show_in.iter().map(|value| value.to_string()).collect(),
        not_show_in: not_show_in.iter().map(|value| value.to_string()).collect(),
    }
}

#[test]
fn desktop_constraints_allow_unconstrained_applications() {
    let application = candidate(&[], &[]);
    assert!(application.is_shown_in(&["GNOME".to_string()]));
    assert!(application.is_shown_in(&[]));
}

#[test]
fn desktop_constraints_allow_matching_only_show_in() {
    let application = candidate(&["GNOME", "Unity"], &[]);
    assert!(application.is_shown_in(&["GNOME".to_string()]));
    assert!(application.is_shown_in(&["KDE".to_string(), "GNOME".to_string()]));
}

#[test]
fn desktop_constraints_block_nonmatching_only_show_in() {
    let application = candidate(&["GNOME"], &[]);
    assert!(!application.is_shown_in(&["KDE".to_string()]));
    assert!(!application.is_shown_in(&["XFCE".to_string(), "LXQt".to_string()]));
}

#[test]
fn desktop_constraints_block_matching_not_show_in() {
    let application = candidate(&[], &["KDE"]);
    assert!(!application.is_shown_in(&["KDE".to_string()]));
    assert!(!application.is_shown_in(&["GNOME".to_string(), "KDE".to_string()]));
}

#[test]
fn desktop_constraints_allow_nonmatching_not_show_in() {
    let application = candidate(&[], &["KDE"]);
    assert!(application.is_shown_in(&["GNOME".to_string()]));
}

#[test]
fn desktop_constraints_are_case_insensitive() {
    let application = candidate(&["GNOME"], &["KDE"]);
    assert!(application.is_shown_in(&["gnome".to_string()]));
    assert!(!application.is_shown_in(&["kde".to_string()]));
}

#[test]
fn desktop_constraints_allow_unknown_desktops() {
    assert!(candidate(&["GNOME"], &["KDE"]).is_shown_in(&[]));
}

#[test]
fn exec_expansion_supports_file_and_url_fields() {
    let path = Path::new("/home/user/doc.txt");
    let (program, args) = expand_exec_template("gedit %f", path).expect("expand %f");
    assert_eq!(program, "gedit");
    assert_eq!(args, vec!["/home/user/doc.txt"]);
    let (program, args) = expand_exec_template("vlc %u", path).expect("expand %u");
    assert_eq!(program, "vlc");
    assert_eq!(args, vec!["/home/user/doc.txt"]);
}

#[test]
fn exec_expansion_supports_uppercase_file_and_url_fields() {
    let path = Path::new("/tmp/file.png");
    assert_eq!(
        expand_exec_template("eog %F", path),
        Some(("eog".to_string(), vec!["/tmp/file.png".to_string()]))
    );
    assert_eq!(
        expand_exec_template("vlc %U", path),
        Some(("vlc".to_string(), vec!["/tmp/file.png".to_string()]))
    );
}

#[test]
fn exec_expansion_strips_icon_class_and_location_fields() {
    assert_eq!(
        expand_exec_template("nano %i %c %k %f", Path::new("/tmp/x.txt")),
        Some(("nano".to_string(), vec!["/tmp/x.txt".to_string()]))
    );
}

#[test]
fn exec_expansion_handles_embedded_fields() {
    assert_eq!(
        expand_exec_template("viewer --file=%f --quality=90", Path::new("/tmp/image.png")),
        Some((
            "viewer".to_string(),
            vec![
                "--file=/tmp/image.png".to_string(),
                "--quality=90".to_string()
            ]
        ))
    );
}

#[test]
fn exec_expansion_handles_quoted_programs() {
    assert_eq!(
        expand_exec_template(r#""my editor" %f"#, Path::new("/tmp/doc.txt")),
        Some(("my editor".to_string(), vec!["/tmp/doc.txt".to_string()]))
    );
}

#[test]
fn exec_expansion_returns_none_when_every_field_is_stripped() {
    assert!(expand_exec_template("%i %c %k", Path::new("/tmp/x")).is_none());
}

#[test]
fn exec_expansion_drops_unknown_fields() {
    assert_eq!(
        expand_exec_template("app %d %n %f", Path::new("/tmp/doc.txt")),
        Some(("app".to_string(), vec!["/tmp/doc.txt".to_string()]))
    );
}

#[test]
fn exec_expansion_handles_embedded_unknown_fields() {
    assert_eq!(
        expand_exec_template("viewer --opt=%v %f", Path::new("/tmp/img.png")),
        Some((
            "viewer".to_string(),
            vec!["--opt=".to_string(), "/tmp/img.png".to_string()]
        ))
    );
}

#[test]
fn exec_expansion_converts_double_percent_to_literal() {
    assert_eq!(
        expand_exec_template("app --label=100%% %f", Path::new("/tmp/file")),
        Some((
            "app".to_string(),
            vec!["--label=100%".to_string(), "/tmp/file".to_string()]
        ))
    );
}
