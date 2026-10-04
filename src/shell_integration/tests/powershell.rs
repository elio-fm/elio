use super::super::powershell::{ensure_supported_version, parse_version};
use std::process::Command;

/// Stands in for a PowerShell that prints `reported` for `--version`.
#[cfg(unix)]
fn fake_pwsh(reported: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(format!("echo '{reported}'"));
    command
}

#[cfg(windows)]
fn fake_pwsh(reported: &str) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").arg(format!("echo {reported}"));
    command
}

fn check_reported_version(reported: &str) -> anyhow::Result<()> {
    ensure_supported_version(fake_pwsh(reported))
}

#[test]
fn parse_version_reads_major_and_minor() {
    assert_eq!(parse_version("PowerShell 7.4.6"), Some((7, 4)));
    assert_eq!(parse_version("PowerShell 7.5.0-preview.3"), Some((7, 5)));
    assert_eq!(parse_version("7.10.1"), Some((7, 10)));
    assert_eq!(parse_version("PowerShell"), None);
    assert_eq!(parse_version(""), None);
}

#[test]
fn ensure_supported_version_accepts_the_minimum_and_newer() {
    check_reported_version("PowerShell 7.4.0").expect("7.4 should be supported");
    check_reported_version("PowerShell 7.10.2").expect("7.10 should be supported");
    check_reported_version("PowerShell 8.0.0").expect("8.0 should be supported");
}

#[test]
fn ensure_supported_version_rejects_versions_below_the_floor() {
    for (reported, version) in [("PowerShell 7.3.12", "7.3"), ("PowerShell 6.2.7", "6.2")] {
        let error = check_reported_version(reported)
            .expect_err("versions below 7.4 should be rejected")
            .to_string();

        assert!(error.contains(&format!("error: unsupported PowerShell version {version}")));
        assert!(error.contains("requires PowerShell 7.4 or newer"));
    }
}

#[test]
fn ensure_supported_version_rejects_unreadable_versions() {
    let error = check_reported_version("not a version")
        .expect_err("unparseable output should be rejected")
        .to_string();

    assert!(
        error.contains("could not determine the PowerShell version"),
        "{error}"
    );
}

#[test]
fn ensure_supported_version_reports_a_missing_powershell() {
    let error = ensure_supported_version(Command::new("elio-test-missing-pwsh"))
        .expect_err("a missing pwsh should be rejected")
        .to_string();

    assert!(error.contains("to check its version"));
    assert!(error.contains("requires PowerShell 7.4 or newer"));
}
