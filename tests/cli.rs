use std::process::Command;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn elio() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elio"))
}

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-cli-{label}-{unique}"))
}

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
    assert!(stdout.contains("[DIRECTORY]  Start elio in this directory"));
    assert!(stdout.contains("-h, --help"));
    assert!(stdout.contains("-V, --version"));
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
