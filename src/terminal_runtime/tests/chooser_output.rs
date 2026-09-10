use super::{file_is_stdout, write_if_requested};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-lib-{label}-{unique}"))
}

#[test]
fn chooser_file_is_not_written_when_absent() {
    write_if_requested(None, &[PathBuf::from("/tmp/example")])
        .expect("absent chooser file should be a no-op");
}

#[test]
fn chooser_file_hyphen_and_dev_stdout_target_stdout() {
    assert!(file_is_stdout(Path::new("-")));
    #[cfg(unix)]
    assert!(file_is_stdout(Path::new("/dev/stdout")));
    #[cfg(not(unix))]
    assert!(!file_is_stdout(Path::new("/dev/stdout")));
    assert!(!file_is_stdout(Path::new("./-")));
}

#[test]
fn chooser_file_writes_paths_with_trailing_newline() {
    let root = temp_path("chooser-file");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let chooser_file = root.join("selection");
    let alpha = root.join("alpha.txt");
    let beta = root.join("beta.txt");

    write_if_requested(Some(&chooser_file), &[alpha.clone(), beta.clone()])
        .expect("chooser file should be written");

    let bytes = fs::read(&chooser_file).expect("chooser file should be readable");
    let expected = format!("{}\n{}\n", alpha.to_string_lossy(), beta.to_string_lossy());
    assert_eq!(String::from_utf8_lossy(&bytes), expected);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn chooser_file_truncates_on_empty_confirmation() {
    let root = temp_path("chooser-empty");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let chooser_file = root.join("selection");
    fs::write(&chooser_file, "stale\n").expect("chooser file should be primed");

    write_if_requested(Some(&chooser_file), &[])
        .expect("empty chooser confirmation should be written");

    let bytes = fs::read(&chooser_file).expect("chooser file should be readable");
    assert!(bytes.is_empty());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}
