use super::*;
use std::time::UNIX_EPOCH;

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-{label}-{unique}"))
}

fn test_entry(name: &str, kind: EntryKind) -> Entry {
    Entry {
        path: PathBuf::from(name),
        name: name.to_string(),
        name_key: name.to_string(),
        kind,
        size: 10,
        ..Entry::default()
    }
}

#[test]
fn sort_keeps_directories_before_files() {
    let mut entries = vec![
        test_entry("beta.txt", EntryKind::File),
        test_entry("alpha", EntryKind::Directory),
    ];

    sort_entries(&mut entries, SortMode::Name);
    assert!(entries[0].is_dir());
    assert!(!entries[1].is_dir());
}

#[test]
fn sort_uses_natural_numeric_order_for_names() {
    let mut entries = vec![
        test_entry("episode 10.mkv", EntryKind::File),
        test_entry("episode 2.mkv", EntryKind::File),
        test_entry("episode 1.mkv", EntryKind::File),
    ];

    sort_entries(&mut entries, SortMode::Name);
    let names = entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["episode 1.mkv", "episode 2.mkv", "episode 10.mkv"]
    );
}

#[test]
fn sort_uses_natural_numeric_order_with_non_latin_names() {
    let mut entries = vec![
        test_entry("北斗の拳 究極版 10巻.epub", EntryKind::File),
        test_entry("北斗の拳 究極版 2巻.epub", EntryKind::File),
        test_entry("北斗の拳 究極版 1巻.epub", EntryKind::File),
    ];

    sort_entries(&mut entries, SortMode::Name);
    let names = entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "北斗の拳 究極版 1巻.epub",
            "北斗の拳 究極版 2巻.epub",
            "北斗の拳 究極版 10巻.epub",
        ]
    );
}

#[test]
#[cfg(unix)]
fn symlinked_file_uses_target_size() {
    use std::os::unix::fs::symlink;

    let root = temp_path("symlink-file");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let target = root.join("target.txt");
    fs::write(&target, "hello world").expect("failed to write target file");
    symlink(&target, root.join("linked.txt")).expect("failed to create symlink");

    let entries = read_entries(&root, false, &|| false).expect("failed to read entries");
    let linked = entries
        .iter()
        .find(|entry| entry.name == "linked.txt")
        .expect("linked file should be present");

    assert_eq!(linked.kind, EntryKind::File);
    assert!(linked.symlink.is_some());
    assert_eq!(
        linked
            .symlink
            .as_ref()
            .and_then(|symlink| symlink.target.as_deref()),
        Some(target.as_path())
    );
    assert_eq!(linked.size, 11);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
#[cfg(unix)]
fn symlinked_directory_uses_target_kind() {
    use std::os::unix::fs::symlink;

    let root = temp_path("symlink-dir");
    let target = root.join("target-dir");
    fs::create_dir_all(&target).expect("failed to create target dir");
    symlink(&target, root.join("linked-dir")).expect("failed to create directory symlink");

    let entries = read_entries(&root, false, &|| false).expect("failed to read entries");
    let linked = entries
        .iter()
        .find(|entry| entry.name == "linked-dir")
        .expect("linked dir should be present");

    assert!(linked.is_dir());
    assert!(linked.symlink.is_some());
    assert_eq!(
        linked
            .symlink
            .as_ref()
            .and_then(|symlink| symlink.target_kind),
        Some(EntryKind::Directory)
    );
    assert_eq!(linked.size, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
#[cfg(unix)]
fn broken_symlink_records_target_without_followed_kind() {
    use std::os::unix::fs::symlink;

    let root = temp_path("broken-symlink");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let missing = root.join("missing.txt");
    symlink(&missing, root.join("broken.txt")).expect("failed to create broken symlink");

    let entries = read_entries(&root, false, &|| false).expect("failed to read entries");
    let linked = entries
        .iter()
        .find(|entry| entry.name == "broken.txt")
        .expect("broken link should be present");

    assert_eq!(linked.kind, EntryKind::File);
    assert!(
        linked
            .symlink
            .as_ref()
            .is_some_and(|symlink| symlink.target_kind.is_none())
    );
    assert_eq!(
        linked
            .symlink
            .as_ref()
            .and_then(|symlink| symlink.target.as_deref()),
        Some(missing.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn fingerprint_changes_when_entry_symlink_status_changes() {
    let base = Entry {
        symlink: None,
        ..Entry::default()
    };
    let linked = Entry {
        symlink: Some(SymlinkInfo {
            target: Some(PathBuf::from("target")),
            target_kind: Some(EntryKind::File),
        }),
        ..base.clone()
    };

    assert_ne!(entries_fingerprint(&[base]), entries_fingerprint(&[linked]));
}

#[test]
fn fingerprint_changes_when_visible_directory_entries_change() {
    let root = temp_path("fingerprint");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

    let first = scan_directory_fingerprint(&root, false).expect("failed to fingerprint");
    fs::write(root.join("two.txt"), "world").expect("failed to write second file");
    let second = scan_directory_fingerprint(&root, false).expect("failed to fingerprint");

    assert_ne!(first, second);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn trash_deletion_date_is_parsed_to_unix_timestamp() {
    // 2024-03-15T10:30:00 UTC = 1710498600 seconds since epoch
    let time = parse_trash_deletion_date("2024-03-15T10:30:00").expect("should parse");
    let secs = time
        .duration_since(UNIX_EPOCH)
        .expect("should be after epoch")
        .as_secs();
    assert_eq!(secs, 1_710_498_600);
}

#[test]
fn trash_deletion_date_rejects_invalid_input() {
    assert!(parse_trash_deletion_date("").is_none());
    assert!(parse_trash_deletion_date("not-a-date").is_none());
    assert!(parse_trash_deletion_date("2024-13-01T00:00:00").is_none()); // month 13
    assert!(parse_trash_deletion_date("2024-00-01T00:00:00").is_none()); // month 0
}

#[test]
fn trash_snapshot_uses_deletion_date_from_trashinfo() {
    let root = temp_path("trash-snapshot");
    let files_dir = root.join("files");
    let info_dir = root.join("info");
    fs::create_dir_all(&files_dir).expect("failed to create files dir");
    fs::create_dir_all(&info_dir).expect("failed to create info dir");
    fs::write(files_dir.join("report.pdf"), "dummy").expect("failed to write trashed file");
    fs::write(
        info_dir.join("report.pdf.trashinfo"),
        "[Trash Info]\nPath=/home/user/report.pdf\nDeletionDate=2024-03-15T10:30:00\n",
    )
    .expect("failed to write trashinfo");

    let snapshot = load_directory_snapshot(&files_dir, false, SortMode::Name).expect("should load");
    let entry = snapshot
        .entries
        .iter()
        .find(|e| e.name == "report.pdf")
        .expect("entry should be present");

    let secs = entry
        .modified
        .expect("modified should be set")
        .duration_since(UNIX_EPOCH)
        .expect("should be after epoch")
        .as_secs();
    assert_eq!(secs, 1_710_498_600);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn trash_snapshot_displays_original_name_from_trashinfo() {
    let root = temp_path("trash-display-name");
    let files_dir = root.join("files");
    let info_dir = root.join("info");
    fs::create_dir_all(&files_dir).expect("failed to create files dir");
    fs::create_dir_all(&info_dir).expect("failed to create info dir");

    fs::write(files_dir.join("report.pdf.2"), "dummy")
        .expect("failed to write collision-renamed trashed file");
    fs::write(
        info_dir.join("report.pdf.2.trashinfo"),
        "[Trash Info]\nPath=/home/user/Reports/report%20final.pdf\nDeletionDate=2024-03-15T10:30:00\n",
    )
    .expect("failed to write trashinfo");

    let snapshot = load_directory_snapshot(&files_dir, false, SortMode::Name).expect("should load");
    let entry = snapshot.entries.first().expect("entry should be present");

    assert_eq!(entry.name, "report final.pdf");
    assert_eq!(entry.name_key, "report final.pdf");
    assert_eq!(
        entry.path.file_name().and_then(|n| n.to_str()),
        Some("report.pdf.2")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn trash_snapshot_keeps_stored_name_without_original_path_metadata() {
    let root = temp_path("trash-display-name-fallback");
    let files_dir = root.join("files");
    let info_dir = root.join("info");
    fs::create_dir_all(&files_dir).expect("failed to create files dir");
    fs::create_dir_all(&info_dir).expect("failed to create info dir");

    fs::write(files_dir.join("stored-name.txt.2"), "dummy").expect("failed to write trashed file");
    fs::write(
        info_dir.join("stored-name.txt.2.trashinfo"),
        "[Trash Info]\nDeletionDate=2024-03-15T10:30:00\n",
    )
    .expect("failed to write trashinfo");

    let snapshot = load_directory_snapshot(&files_dir, false, SortMode::Name).expect("should load");
    let entry = snapshot.entries.first().expect("entry should be present");

    assert_eq!(entry.name, "stored-name.txt.2");
    assert_eq!(entry.name_key, "stored-name.txt.2");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn trash_fingerprint_tracks_original_name_from_trashinfo() {
    let root = temp_path("trash-fingerprint-display-name");
    let files_dir = root.join("files");
    let info_dir = root.join("info");
    fs::create_dir_all(&files_dir).expect("failed to create files dir");
    fs::create_dir_all(&info_dir).expect("failed to create info dir");
    fs::write(files_dir.join("photo.jpeg.2"), "dummy").expect("failed to write trashed file");
    let info_path = info_dir.join("photo.jpeg.2.trashinfo");

    fs::write(
        &info_path,
        "[Trash Info]\nPath=/home/user/photo.jpeg\nDeletionDate=2024-03-15T10:30:00\n",
    )
    .expect("failed to write initial trashinfo");
    let first = scan_directory_fingerprint(&files_dir, false).expect("failed to fingerprint");

    fs::write(
        &info_path,
        "[Trash Info]\nPath=/home/user/renamed-photo.jpeg\nDeletionDate=2024-03-15T10:30:00\n",
    )
    .expect("failed to update trashinfo");
    let second = scan_directory_fingerprint(&files_dir, false).expect("failed to fingerprint");

    assert_ne!(first, second);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
