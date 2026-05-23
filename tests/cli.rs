mod support;

use std::fs;
use support::{elio, temp_path};

#[test]
fn version_prints_package_version() {
    let output = elio()
        .arg("--version")
        .output()
        .expect("failed to run elio --version");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("elio {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn help_prints_usage() {
    let output = elio()
        .arg("--help")
        .output()
        .expect("failed to run elio --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: elio [OPTIONS] [DIRECTORY]"));
    assert!(stdout.contains("Arguments:"));
    assert!(stdout.contains("[DIRECTORY]          Start elio in this directory"));
    assert!(stdout.contains("--cwd-file FILE  Write the final current directory to FILE on exit"));
    assert!(stdout.contains("-h, --help           Print help"));
    assert!(stdout.contains("-V, --version        Print version"));
    assert!(output.stderr.is_empty());
}

#[test]
fn mistyped_version_flag_exits_with_suggestion() {
    let output = elio().arg("--v").output().expect("failed to run elio --v");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error: unexpected argument '--v' found"));
    assert!(stderr.contains("tip: a similar argument exists: '--version'"));
}

#[test]
fn extra_argument_after_version_reports_the_extra_argument() {
    let output = elio()
        .args(["--version", "extra"])
        .output()
        .expect("failed to run elio --version extra");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error: unexpected argument 'extra' found"));
    assert!(!stderr.contains("tip: a similar argument exists"));
}

#[test]
fn missing_directory_argument_exits_with_clear_error() {
    let missing = temp_path("missing");

    let output = elio()
        .arg(missing.to_str().expect("temp path should be valid utf-8"))
        .output()
        .expect("failed to run elio with missing directory");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Cannot open \"{}\": no such file or directory\n",
            missing.display()
        )
    );
}

#[test]
fn file_argument_exits_with_not_a_directory_error() {
    let root = temp_path("file");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let file = root.join("notes.txt");
    fs::write(&file, "hello").expect("temp file should be created");

    let output = elio()
        .arg(file.to_str().expect("temp path should be valid utf-8"))
        .output()
        .expect("failed to run elio with file path");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!("Cannot open \"{}\": not a directory\n", file.display())
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn more_than_one_directory_is_rejected() {
    let first = temp_path("dir-one");
    let second = temp_path("dir-two");
    fs::create_dir_all(&first).expect("first temp directory should be created");
    fs::create_dir_all(&second).expect("second temp directory should be created");

    let output = elio()
        .arg(&first)
        .arg(&second)
        .output()
        .expect("failed to run elio with two directory arguments");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("error: unexpected argument"));

    fs::remove_dir_all(first).expect("first temp directory should be removed");
    fs::remove_dir_all(second).expect("second temp directory should be removed");
}
