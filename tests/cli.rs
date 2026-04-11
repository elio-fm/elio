use std::process::Command;

fn elio() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elio"))
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
    assert!(stdout.contains("Usage: elio [OPTIONS]"));
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
