use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-{label}-{unique}"))
}

/// Builds a minimal FreeDesktop trash layout under `root`:
///   root/
///     files/<name>  ← the trashed item (a regular file)
///     info/<name>.trashinfo
///
/// Returns `(trash_files_dir, trash_info_dir, item_path)`.
#[cfg(unix)]
fn make_freedesktop_trash(root: &Path, name: &str, original: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let files_dir = root.join("files");
    let info_dir = root.join("info");
    fs::create_dir_all(&files_dir).expect("failed to create trash files dir");
    fs::create_dir_all(&info_dir).expect("failed to create trash info dir");
    let item_path = files_dir.join(name);
    fs::write(&item_path, b"trashed content").expect("failed to write trashed item");
    let trashinfo = format!(
        "[Trash Info]\nPath={}\nDeletionDate=2024-01-01T00:00:00\n",
        original.to_str().unwrap()
    );
    fs::write(info_dir.join(format!("{name}.trashinfo")), trashinfo)
        .expect("failed to write trashinfo");
    (files_dir, info_dir, item_path)
}

#[test]
#[cfg(unix)]
fn restore_freedesktop_moves_item_to_original_path_and_removes_trashinfo() {
    let root = temp_path("restore-fd-ok");
    let restore_target = temp_path("restore-fd-ok-dest");
    fs::create_dir_all(&root).expect("failed to create trash root");
    fs::create_dir_all(&restore_target).expect("failed to create restore target dir");

    let original = restore_target.join("report.pdf");
    let (_, info_dir, item_path) = make_freedesktop_trash(&root, "report.pdf", &original);

    let result = restore_trash_item(&item_path);
    assert!(result.is_ok(), "restore should succeed: {:?}", result);
    assert!(original.exists(), "file should be at original location");
    assert!(!item_path.exists(), "trashed item should be gone");
    assert!(
        !info_dir.join("report.pdf.trashinfo").exists(),
        "trashinfo should be removed"
    );

    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&restore_target).ok();
}

#[test]
#[cfg(unix)]
fn restore_freedesktop_fails_when_destination_already_exists() {
    let root = temp_path("restore-fd-conflict");
    let restore_target = temp_path("restore-fd-conflict-dest");
    fs::create_dir_all(&root).expect("failed to create trash root");
    fs::create_dir_all(&restore_target).expect("failed to create restore target dir");

    let original = restore_target.join("conflict.txt");
    fs::write(&original, b"already here").expect("failed to write blocking file");

    let (_, _, item_path) = make_freedesktop_trash(&root, "conflict.txt", &original);

    let err = restore_trash_item(&item_path).unwrap_err();
    assert!(
        err.to_string().contains("destination already exists"),
        "unexpected error: {err}"
    );

    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&restore_target).ok();
}

#[test]
#[cfg(unix)]
fn restore_freedesktop_fails_when_trashinfo_is_missing() {
    let root = temp_path("restore-fd-no-info");
    let files_dir = root.join("files");
    let info_dir = root.join("info");
    fs::create_dir_all(&files_dir).expect("failed to create files dir");
    fs::create_dir_all(&info_dir).expect("failed to create info dir");

    let item_path = files_dir.join("orphan.txt");
    fs::write(&item_path, b"no metadata").expect("failed to write orphan item");
    // Deliberately do NOT write a .trashinfo file.

    let err = restore_trash_item(&item_path).unwrap_err();
    assert!(
        err.to_string().contains("orphan.txt.trashinfo"),
        "error should mention the missing trashinfo, got: {err}"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn restore_fails_for_path_outside_any_known_trash_layout() {
    let tmp = temp_path("restore-unsupported");
    fs::create_dir_all(&tmp).expect("failed to create temp dir");
    let fake_item = tmp.join("item.txt");
    fs::write(&fake_item, b"content").expect("failed to write item");

    #[cfg(not(target_os = "macos"))]
    {
        let err = restore_trash_item(&fake_item).unwrap_err();
        assert!(
            err.to_string().contains("not supported"),
            "unexpected error: {err}"
        );
    }

    fs::remove_dir_all(&tmp).ok();
}

/// Regression test for false-positive FreeDesktop detection.
///
/// On macOS, `~/.Trash/foo` computes `~/info` as the candidate info dir.
/// If the user happens to have a `~/info` directory, the old code would
/// take the FreeDesktop path and then fail looking for a `.trashinfo` file
/// instead of falling through to the Finder backend.
///
/// The fix requires the entry's immediate parent to be named `files` before
/// treating the layout as FreeDesktop, so a `~/.Trash`-style path is never
/// misidentified even when a coincidental `info/` exists nearby.
#[test]
#[cfg(not(target_os = "macos"))]
fn restore_does_not_misdetect_freedesktop_when_info_dir_exists_at_wrong_level() {
    let root = temp_path("restore-false-positive");
    let trash_dir = root.join("Trash");
    let decoy_info = root.join("info");
    fs::create_dir_all(&trash_dir).expect("failed to create trash dir");
    fs::create_dir_all(&decoy_info).expect("failed to create decoy info dir");

    let item_path = trash_dir.join("foo.txt");
    fs::write(&item_path, b"content").expect("failed to write item");

    let err = restore_trash_item(&item_path).unwrap_err();
    assert!(
        err.to_string().contains("not supported"),
        "should bail as unsupported, not attempt FreeDesktop restore: {err}"
    );

    fs::remove_dir_all(&root).ok();
}

// ── macOS DS_Store restore helpers ────────────────────────────────────────

#[test]
#[cfg(target_os = "macos")]
fn decode_utf16be_decodes_ascii_string() {
    let bytes = b"\x00H\x00i";
    assert_eq!(decode_utf16be(bytes), Some("Hi".to_string()));
}

#[test]
#[cfg(target_os = "macos")]
fn decode_utf16be_decodes_non_ascii() {
    let bytes = b"\x00\xe9";
    assert_eq!(decode_utf16be(bytes), Some("é".to_string()));
}

#[test]
#[cfg(target_os = "macos")]
fn decode_utf16be_rejects_odd_byte_count() {
    assert_eq!(decode_utf16be(b"\x00H\x00"), None);
}

#[test]
#[cfg(target_os = "macos")]
fn decode_utf16be_empty_slice_gives_empty_string() {
    assert_eq!(decode_utf16be(b""), Some(String::new()));
}

// ── remove_from_origins_map ───────────────────────────────────────────────

#[test]
#[cfg(target_os = "macos")]
fn remove_from_origins_map_removes_exact_match() {
    let mut map = std::collections::HashMap::from([
        (
            "report.pdf".to_string(),
            "/home/user/report.pdf".to_string(),
        ),
        ("notes.txt".to_string(), "/home/user/notes.txt".to_string()),
    ]);
    let changed = remove_from_origins_map(&mut map, &["report.pdf"]);
    assert!(changed);
    assert!(
        !map.contains_key("report.pdf"),
        "target entry should be removed"
    );
    assert!(
        map.contains_key("notes.txt"),
        "unrelated entry should be untouched"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn remove_from_origins_map_handles_collision_suffix_with_extension() {
    // "report.pdf" was stored as the key but macOS renamed it "report 2.pdf"
    // in the trash due to a collision.
    let mut map = std::collections::HashMap::from([(
        "report.pdf".to_string(),
        "/home/user/report.pdf".to_string(),
    )]);
    let changed = remove_from_origins_map(&mut map, &["report 2.pdf"]);
    assert!(changed);
    assert!(
        map.is_empty(),
        "collision-suffixed name should strip and remove base key"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn remove_from_origins_map_handles_collision_suffix_without_extension() {
    let mut map =
        std::collections::HashMap::from([("notes".to_string(), "/home/user/notes".to_string())]);
    let changed = remove_from_origins_map(&mut map, &["notes 2"]);
    assert!(changed);
    assert!(map.is_empty());
}

#[test]
#[cfg(target_os = "macos")]
fn remove_from_origins_map_returns_false_when_key_not_found() {
    let mut map = std::collections::HashMap::from([(
        "other.txt".to_string(),
        "/home/user/other.txt".to_string(),
    )]);
    let changed = remove_from_origins_map(&mut map, &["missing.txt"]);
    assert!(
        !changed,
        "no match should return false and leave map untouched"
    );
    assert_eq!(map.len(), 1);
}

#[test]
#[cfg(target_os = "macos")]
fn remove_from_origins_map_removes_multiple_names() {
    let mut map = std::collections::HashMap::from([
        ("a.txt".to_string(), "/home/user/a.txt".to_string()),
        ("b.txt".to_string(), "/home/user/b.txt".to_string()),
        ("c.txt".to_string(), "/home/user/c.txt".to_string()),
    ]);
    let changed = remove_from_origins_map(&mut map, &["a.txt", "c.txt"]);
    assert!(changed);
    assert!(!map.contains_key("a.txt"));
    assert!(map.contains_key("b.txt"), "untargeted entry must survive");
    assert!(!map.contains_key("c.txt"));
}

#[test]
#[cfg(target_os = "macos")]
fn remove_from_origins_map_no_op_on_empty_map() {
    let mut map = std::collections::HashMap::new();
    let changed = remove_from_origins_map(&mut map, &["foo.txt"]);
    assert!(!changed);
}
