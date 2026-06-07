use super::*;
use std::fs;

// ── helpers ───────────────────────────────────────────────────────────────

fn unique_dir(label: &str) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    std::env::temp_dir().join(format!(
        "elio-mime-test-{label}-{}-{nanos}",
        std::process::id()
    ))
}

fn write_globs2(dir: &Path, content: &str) {
    let mime_dir = dir.join("mime");
    fs::create_dir_all(&mime_dir).expect("create mime dir");
    fs::write(mime_dir.join("globs2"), content).expect("write globs2");
}

fn write_globs(dir: &Path, content: &str) {
    let mime_dir = dir.join("mime");
    fs::create_dir_all(&mime_dir).expect("create mime dir");
    fs::write(mime_dir.join("globs"), content).expect("write globs");
}

// ── mime_from_data_dirs ───────────────────────────────────────────────────

#[test]
fn finds_mime_type_from_globs2() {
    let dir = unique_dir("globs2-basic");
    write_globs2(&dir, "50:text/markdown:*.md\n");

    let result = mime_from_data_dirs(Path::new("/any/file.md"), std::slice::from_ref(&dir));
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(result.as_deref(), Some("text/markdown"));
}

#[test]
fn falls_back_to_globs_when_globs2_absent() {
    let dir = unique_dir("globs-fallback");
    write_globs(&dir, "text/plain:*.txt\n");

    let result = mime_from_data_dirs(Path::new("/any/file.txt"), std::slice::from_ref(&dir));
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(result.as_deref(), Some("text/plain"));
}

#[test]
fn globs2_higher_weight_wins_over_lower_weight() {
    let dir = unique_dir("globs2-weight");
    // Two entries for *.md — higher weight (60) should win.
    write_globs2(
        &dir,
        "40:text/x-markdown:*.md\n\
         60:text/markdown:*.md\n\
         50:text/plain:*.txt\n",
    );

    let result = mime_from_data_dirs(Path::new("/docs/readme.md"), std::slice::from_ref(&dir));
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(result.as_deref(), Some("text/markdown"));
}

#[test]
fn globs2_weights_compared_across_multiple_data_dirs() {
    // dir1 has a low-weight match; dir2 has a higher-weight match.
    // The higher weight should win regardless of dir priority.
    let dir1 = unique_dir("multi-dir-low");
    let dir2 = unique_dir("multi-dir-high");
    write_globs2(&dir1, "30:text/x-markdown:*.md\n");
    write_globs2(&dir2, "70:text/markdown:*.md\n");

    let result = mime_from_data_dirs(Path::new("/docs/readme.md"), &[dir1.clone(), dir2.clone()]);
    let _ = fs::remove_dir_all(&dir1);
    let _ = fs::remove_dir_all(&dir2);

    assert_eq!(result.as_deref(), Some("text/markdown"));
}

#[test]
fn globs_first_match_in_priority_order_wins() {
    // dir1 (higher priority) has text/plain for *.txt;
    // dir2 (lower priority) has text/x-log for *.txt.
    // dir1's entry should win.
    let dir1 = unique_dir("globs-priority-high");
    let dir2 = unique_dir("globs-priority-low");
    write_globs(&dir1, "text/plain:*.txt\n");
    write_globs(&dir2, "text/x-log:*.txt\n");

    let result = mime_from_data_dirs(Path::new("/var/log/app.txt"), &[dir1.clone(), dir2.clone()]);
    let _ = fs::remove_dir_all(&dir1);
    let _ = fs::remove_dir_all(&dir2);

    assert_eq!(result.as_deref(), Some("text/plain"));
}

#[test]
fn returns_none_for_unknown_extension() {
    let dir = unique_dir("no-match");
    write_globs2(&dir, "50:text/plain:*.txt\n");

    let result = mime_from_data_dirs(
        Path::new("/tmp/file.xyzzy_elio_test"),
        std::slice::from_ref(&dir),
    );
    let _ = fs::remove_dir_all(&dir);

    assert!(result.is_none());
}

#[test]
fn returns_none_when_data_dirs_empty() {
    let result = mime_from_data_dirs(Path::new("/tmp/file.txt"), &[]);
    assert!(result.is_none());
}

#[test]
fn returns_none_for_path_with_no_extension() {
    let dir = unique_dir("no-ext");
    write_globs2(&dir, "50:text/plain:*.txt\n");

    let result = mime_from_data_dirs(Path::new("/usr/bin/ls"), std::slice::from_ref(&dir));
    let _ = fs::remove_dir_all(&dir);

    assert!(result.is_none());
}

#[test]
fn display_name_extension_can_override_collision_suffixed_storage_path() {
    let dir = unique_dir("display-name-extension");
    write_globs2(&dir, "50:image/jpeg:*.jpeg\n");

    let result = mime_from_data_dirs_with_name(
        Path::new("/trash/files/photo.jpeg.2"),
        Some("photo.jpeg"),
        std::slice::from_ref(&dir),
    );
    let raw_storage_result = mime_from_data_dirs(
        Path::new("/trash/files/photo.jpeg.2"),
        std::slice::from_ref(&dir),
    );
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(result.as_deref(), Some("image/jpeg"));
    assert!(raw_storage_result.is_none());
}

// ── mime_from_xdg_database (system integration) ───────────────────────────
// Uses the real XDG data dirs.  Skips gracefully if no MIME database is
// present (e.g. minimal CI image).

#[test]
fn mime_from_xdg_database_returns_expected_type_for_common_extensions() {
    let has_db = super::super::xdg_data_dirs()
        .iter()
        .any(|d| d.join("mime/globs2").exists() || d.join("mime/globs").exists());
    if !has_db {
        return;
    }
    // .png is universally registered as image/png.
    let result = mime_from_data_dirs_with_name(
        Path::new("/any/path/image.png"),
        None,
        &super::super::xdg_data_dirs(),
    );
    assert_eq!(
        result.as_deref(),
        Some("image/png"),
        "expected image/png for .png extension"
    );
}

// ── parse_gio_content_type ────────────────────────────────────────────────

#[test]
fn parse_gio_content_type_extracts_mime_from_gio_info_output() {
    let output = "uri: file:///home/user/Makefile\n\
                  local path: /home/user/Makefile\n\
                  attributes:\n\
                    standard::content-type: text/x-makefile\n";
    assert_eq!(
        super::parse_gio_content_type(output).as_deref(),
        Some("text/x-makefile")
    );
}

#[test]
fn parse_gio_content_type_returns_none_for_missing_attribute() {
    let output = "uri: file:///home/user/unknown\nattributes:\n";
    assert!(super::parse_gio_content_type(output).is_none());
}

#[test]
fn parse_gio_content_type_handles_empty_output() {
    assert!(super::parse_gio_content_type("").is_none());
}
