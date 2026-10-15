use anyhow::{Context, Result};
use std::process::Command;

pub(super) const PWSH_PROGRAM: &str = "pwsh";

const MINIMUM_VERSION: (u32, u32) = (7, 4);
const REQUIREMENT: &str = "elio shell integration requires PowerShell 7.4 or newer.";

/// Asks `powershell` for its `--version` and rejects anything older than
/// PowerShell 7.4, so an unsupported PowerShell never has its profile modified.
pub(super) fn ensure_supported_version(mut powershell: Command) -> Result<()> {
    let name = powershell.get_program().to_string_lossy().into_owned();
    let output = powershell.arg("--version").output().with_context(|| {
        format!("error: could not run `{name}` to check its version\n\n{REQUIREMENT}")
    })?;
    let reported = String::from_utf8_lossy(&output.stdout);
    let reported = reported.trim();

    let Some(version) = parse_version(reported).filter(|_| output.status.success()) else {
        anyhow::bail!(
            "error: could not determine the PowerShell version from `{name} --version`\n\n{REQUIREMENT}"
        );
    };

    if version < MINIMUM_VERSION {
        anyhow::bail!(
            "error: unsupported PowerShell version {}.{}\n\n{REQUIREMENT}",
            version.0,
            version.1
        );
    }

    Ok(())
}

/// Extracts `(major, minor)` from output such as `PowerShell 7.4.6` or
/// `PowerShell 7.5.0-preview.3`.
pub(super) fn parse_version(reported: &str) -> Option<(u32, u32)> {
    let version = reported
        .split_whitespace()
        .find(|word| word.starts_with(|first: char| first.is_ascii_digit()))?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}
