use super::{ResolvedPath, resolve_path};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-cli-{label}-{unique}"))
}

#[test]
fn existing_directory_resolves_without_focus() {
    let root = temp_path("directory");
    fs::create_dir_all(&root).expect("temp directory should be created");

    let resolved = resolve_path(root.to_str().expect("temp path should be utf-8"))
        .expect("existing directory should resolve");

    assert_eq!(
        resolved,
        ResolvedPath {
            directory: root
                .canonicalize()
                .expect("temp directory should canonicalize successfully"),
            focused_entry: None,
            reveal_hidden: false,
        }
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn existing_file_resolves_to_parent_with_focus() {
    let root = temp_path("file");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let file = root.join("notes.txt");
    fs::write(&file, "hello").expect("temp file should be created");

    let resolved = resolve_path(file.to_str().expect("temp path should be valid utf-8"))
        .expect("file path should resolve");
    let canonical_root = root
        .canonicalize()
        .expect("temp directory should canonicalize successfully");

    assert_eq!(
        resolved,
        ResolvedPath {
            directory: canonical_root.clone(),
            focused_entry: Some(canonical_root.join("notes.txt")),
            reveal_hidden: false,
        }
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn hidden_file_is_revealed() {
    let root = temp_path("hidden-file");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let file = root.join(".env");
    fs::write(&file, "secret").expect("temp file should be created");

    let resolved = resolve_path(file.to_str().expect("temp path should be valid utf-8"))
        .expect("hidden file path should resolve");
    let canonical_root = root
        .canonicalize()
        .expect("temp directory should canonicalize successfully");

    assert_eq!(
        resolved,
        ResolvedPath {
            directory: canonical_root.clone(),
            focused_entry: Some(canonical_root.join(".env")),
            reveal_hidden: true,
        }
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn file_symlink_focuses_link_itself() {
    use std::os::unix::fs::symlink;

    let root = temp_path("file-symlink");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let target = root.join("target.txt");
    let link = root.join("link.txt");
    fs::write(&target, "target").expect("target file should be created");
    symlink(&target, &link).expect("file symlink should be created");

    let resolved = resolve_path(link.to_str().expect("temp path should be valid utf-8"))
        .expect("file symlink should resolve");
    let canonical_root = root
        .canonicalize()
        .expect("temp directory should canonicalize successfully");

    assert_eq!(
        resolved,
        ResolvedPath {
            directory: canonical_root.clone(),
            focused_entry: Some(canonical_root.join("link.txt")),
            reveal_hidden: false,
        }
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn broken_symlink_focuses_link_itself() {
    use std::os::unix::fs::symlink;

    let root = temp_path("broken-symlink");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let missing_target = root.join("missing.txt");
    let link = root.join("broken.txt");
    symlink(&missing_target, &link).expect("broken symlink should be created");

    let resolved = resolve_path(link.to_str().expect("temp path should be valid utf-8"))
        .expect("broken symlink should resolve");
    let canonical_root = root
        .canonicalize()
        .expect("temp directory should canonicalize successfully");

    assert_eq!(
        resolved,
        ResolvedPath {
            directory: canonical_root.clone(),
            focused_entry: Some(canonical_root.join("broken.txt")),
            reveal_hidden: false,
        }
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}
