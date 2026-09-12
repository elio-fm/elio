use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::symlink;

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-directory-stats-{label}-{unique}"))
}

#[test]
fn recursive_directory_stats_include_nested_entries_and_sizes() {
    let root = temp_path("recursive");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).expect("failed to create nested dir");
    fs::write(root.join("a.txt"), vec![b'a'; 500]).expect("failed to write file");
    fs::write(nested.join("b.txt"), vec![b'b'; 700]).expect("failed to write nested file");

    let result = scan_directory_stats(&root, &|| false);

    assert_eq!(
        result,
        DirectoryStatsScanResult::Complete(DirectoryStats {
            item_count: 3,
            folder_count: 1,
            file_count: 2,
            total_size_bytes: 1_200,
        })
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn recursive_directory_stats_do_not_follow_symlinked_directories() {
    let root = temp_path("symlink-dir");
    let nested = root.join("nested");
    let linked = root.join("linked");
    fs::create_dir_all(&nested).expect("failed to create nested dir");
    fs::write(nested.join("inside.txt"), vec![b'x'; 900]).expect("failed to write file");
    symlink(&nested, &linked).expect("failed to create symlink");

    let result = scan_directory_stats(&root, &|| false);
    let symlink_size = fs::symlink_metadata(&linked)
        .expect("failed to stat symlink")
        .len();

    assert_eq!(
        result,
        DirectoryStatsScanResult::Complete(DirectoryStats {
            item_count: 3,
            folder_count: 1,
            file_count: 2,
            total_size_bytes: 900 + symlink_size,
        })
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
