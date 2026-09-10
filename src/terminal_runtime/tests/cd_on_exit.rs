use super::write_if_requested;
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-lib-{label}-{unique}"))
}

#[test]
fn cwd_file_is_not_written_when_absent() {
    write_if_requested(None, Path::new("/tmp")).expect("absent cwd file should be a no-op");
}

#[test]
fn cwd_file_writes_path_without_trailing_newline() {
    let root = temp_path("cwd-file");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let cwd_file = root.join("cwd");
    let final_cwd = root.join("nested");
    fs::create_dir_all(&final_cwd).expect("nested temp directory should be created");

    write_if_requested(Some(&cwd_file), &final_cwd).expect("cwd file should be written");

    let bytes = fs::read(&cwd_file).expect("cwd file should be readable");
    assert!(!bytes.ends_with(b"\n"));
    assert_eq!(String::from_utf8_lossy(&bytes), final_cwd.to_string_lossy());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}
