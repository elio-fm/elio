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

// ── discover_via_desktop_scan_in_dirs ─────────────────────────────────────

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

    let apps = discover_via_desktop_scan_in_dirs(
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

    let apps = discover_via_desktop_scan_in_dirs(
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

    let apps = discover_via_desktop_scan_in_dirs(
        "text/plain",
        Path::new("/tmp/hello.txt"),
        std::slice::from_ref(&dir),
    );
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].display_name, "Konsole");
    assert_eq!(
        apps[0].desktop_id.as_deref(),
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

    let apps = discover_via_desktop_scan_in_dirs(
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

    let apps = discover_via_desktop_scan_inner(
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

    let apps = discover_via_desktop_scan_inner(
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

    let apps = discover_via_desktop_scan_in_dirs(
        "text/plain",
        Path::new("/tmp/file.txt"),
        std::slice::from_ref(&dir),
    );
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].display_name, "Real");
}
