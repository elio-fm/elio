use super::*;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-paste-{label}-{unique}"))
}

#[test]
fn duplicate_plain_name_uses_underscore_suffixes() {
    let dir = temp_path("plain");
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    fs::write(dir.join("example"), "data").expect("failed to write source");
    assert_eq!(unique_dest(&dir, "example"), dir.join("example_1"));
    fs::remove_dir_all(&dir).expect("failed to remove temp dir");
}

#[test]
fn duplicate_original_skips_to_next_available_suffix() {
    let dir = temp_path("next-available");
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    fs::write(dir.join("example"), "base").expect("failed to write base file");
    fs::write(dir.join("example_1"), "copy").expect("failed to write suffixed file");
    assert_eq!(unique_dest(&dir, "example"), dir.join("example_2"));
    fs::remove_dir_all(&dir).expect("failed to remove temp dir");
}

#[test]
fn duplicate_suffixed_name_stays_literal() {
    let dir = temp_path("nested-suffix");
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    fs::write(dir.join("aur_1"), "report").expect("failed to write source");
    assert_eq!(unique_dest(&dir, "aur_1"), dir.join("aur_1_1"));
    fs::remove_dir_all(&dir).expect("failed to remove temp dir");
}

#[test]
fn duplicate_suffixed_name_with_extension_stays_literal() {
    let dir = temp_path("nested-suffix-ext");
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    fs::write(dir.join("aur_1.txt"), "copy").expect("failed to write source");
    assert_eq!(unique_dest(&dir, "aur_1.txt"), dir.join("aur_1_1.txt"));
    fs::remove_dir_all(&dir).expect("failed to remove temp dir");
}

#[test]
fn copy_recursive_refuses_directory_into_itself() {
    let root = temp_path("copy-into-self");
    let source = root.join("source");
    let dest = source.join("source");
    fs::create_dir_all(&source).expect("failed to create source dir");
    fs::write(source.join("file.txt"), "data").expect("failed to write source file");
    let error = copy_recursive(&source, &dest).expect_err("copy should be rejected");
    assert!(
        error
            .to_string()
            .contains("Cannot paste a folder into itself"),
        "unexpected error: {error}"
    );
    assert!(!dest.exists());
    fs::remove_dir_all(&root).expect("failed to remove temp root");
}

#[test]
fn source_contains_destination_only_blocks_directory_descendants() {
    let root = temp_path("source-dest-check");
    let source_dir = root.join("source");
    let source_file = root.join("file.txt");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    fs::write(&source_file, "data").expect("failed to write source file");
    assert!(source_contains_destination(
        &source_dir,
        &source_dir.join("child")
    ));
    assert!(!source_contains_destination(
        &source_dir,
        &root.join("source_copy")
    ));
    assert!(!source_contains_destination(
        &source_file,
        &source_file.join("child")
    ));
    fs::remove_dir_all(&root).expect("failed to remove temp root");
}
