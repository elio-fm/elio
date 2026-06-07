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
    assert!(stdout.contains("Usage: elio [OPTIONS] [PATH]"));
    assert!(stdout.contains("       elio shell init <SHELL>"));
    assert!(stdout.contains("       elio shell uninstall [SHELL]"));
    assert!(stdout.contains("Arguments:"));
    assert!(stdout.contains(
        "[PATH]               Start in a directory, or focus a file in its parent directory"
    ));
    assert!(stdout.contains("--cwd-file FILE  Write the final current directory to FILE on exit"));
    assert!(stdout.contains("-h, --help           Print help"));
    assert!(stdout.contains("-V, --version        Print version"));
    assert!(stdout.contains("Commands:"));
    assert!(
        stdout.contains(
            "shell init <SHELL>        Print shell integration for bash, zsh, fish, or nu"
        )
    );
    assert!(
        stdout.contains(
            "shell install [SHELL]    Install shell integration for bash, zsh, fish, or nu"
        )
    );
    assert!(
        stdout.contains(
            "shell uninstall [SHELL]  Remove shell integration for bash, zsh, fish, or nu"
        )
    );
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
fn missing_path_argument_exits_with_clear_error() {
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
fn more_than_one_path_is_rejected() {
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
