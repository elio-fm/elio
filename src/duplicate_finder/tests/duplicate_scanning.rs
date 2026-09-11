use super::*;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "elio-duplicates-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn duplicate_group(id: u64, size: u64, names: &[&str]) -> DuplicateGroup {
    DuplicateGroup {
        id,
        size,
        files: names
            .iter()
            .map(|name| DuplicateFile {
                path: PathBuf::from(name),
                name: (*name).to_string(),
                relative: (*name).to_string(),
                size,
                modified: None,
            })
            .collect(),
    }
}

#[test]
fn duplicate_group_order_prefers_larger_files_before_larger_reclaimable_groups() {
    let mut groups = [
        duplicate_group(1, 700 * 1024, &["small-a", "small-b", "small-c", "small-d"]),
        duplicate_group(2, 20 * 1024 * 1024, &["medium-a", "medium-b"]),
        duplicate_group(
            3,
            20 * 1024 * 1024,
            &["medium-more-a", "medium-more-b", "medium-more-c"],
        ),
        duplicate_group(4, 460 * 1024 * 1024, &["large-a", "large-b"]),
    ];

    groups.sort_by(compare_groups);

    assert_eq!(
        groups.iter().map(|group| group.id).collect::<Vec<_>>(),
        vec![4, 3, 2, 1]
    );
}

#[test]
fn scan_groups_exact_duplicate_files_by_content_not_name() {
    let root = temp_path("exact");
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("a.txt"), b"same").unwrap();
    fs::write(root.join("nested/renamed.bin"), b"same").unwrap();
    fs::write(root.join("different.txt"), b"diff").unwrap();

    let result = scan_duplicates_streaming(&root, true, || false, |_| true).unwrap();

    assert_eq!(result.groups.len(), 1);
    assert_eq!(result.groups[0].files.len(), 2);
    assert_eq!(result.groups[0].duplicate_bytes(), 4);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_coalesces_small_duplicate_batches() {
    let root = temp_path("coalesced-batches");
    fs::create_dir_all(&root).unwrap();
    for (left, right, content) in [
        ("a1.txt", "a2.txt", b"aa".as_slice()),
        ("b1.txt", "b2.txt", b"bbb".as_slice()),
        ("c1.txt", "c2.txt", b"cccc".as_slice()),
    ] {
        fs::write(root.join(left), content).unwrap();
        fs::write(root.join(right), content).unwrap();
    }

    let mut batches = Vec::new();
    let result = scan_duplicates_streaming(
        &root,
        true,
        || false,
        |batch| {
            batches.push(batch);
            true
        },
    )
    .unwrap();

    assert_eq!(result.groups.len(), 3);
    let group_batches = batches
        .iter()
        .filter(|batch| !batch.groups.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(group_batches.len(), 1);
    assert_eq!(group_batches[0].groups.len(), 3);
    assert_eq!(result.stats.phase, DuplicateScanPhase::Complete);
    let phases = batches
        .iter()
        .map(|batch| batch.stats.phase)
        .collect::<Vec<_>>();
    assert!(phases.contains(&DuplicateScanPhase::SizeGrouping));
    assert!(phases.contains(&DuplicateScanPhase::ContentChecking));
    assert_eq!(result.stats.candidate_files, 6);
    assert_eq!(result.stats.checked_candidates, 6);
    assert_eq!(result.stats.hashed_files, 6);
    assert!(result.stats.processed_bytes > 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_respects_hidden_file_setting_for_directories() {
    let root = temp_path("hidden-dirs");
    fs::create_dir_all(root.join("visible")).unwrap();
    fs::create_dir_all(root.join(".cache")).unwrap();
    fs::write(root.join("visible/a.txt"), b"visible duplicate").unwrap();
    fs::write(root.join("visible/b.txt"), b"visible duplicate").unwrap();
    fs::write(root.join(".cache/a.txt"), b"hidden duplicate").unwrap();
    fs::write(root.join(".cache/b.txt"), b"hidden duplicate").unwrap();

    let hidden_off = scan_duplicates_streaming(&root, false, || false, |_| true).unwrap();
    assert_eq!(hidden_off.groups.len(), 1);
    assert!(
        hidden_off.groups[0]
            .files
            .iter()
            .all(|file| file.relative.starts_with("visible/"))
    );

    let hidden_on = scan_duplicates_streaming(&root, true, || false, |_| true).unwrap();
    assert_eq!(hidden_on.groups.len(), 2);
    assert!(hidden_on.groups.iter().any(|group| {
        group
            .files
            .iter()
            .all(|file| file.relative.starts_with(".cache/"))
    }));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_ignores_zero_byte_files() {
    let root = temp_path("zero");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a"), b"").unwrap();
    fs::write(root.join("b"), b"").unwrap();

    let result = scan_duplicates_streaming(&root, true, || false, |_| true).unwrap();

    assert!(result.groups.is_empty());
    fs::remove_dir_all(root).unwrap();
}
