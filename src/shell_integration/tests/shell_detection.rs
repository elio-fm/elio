use super::super::{Shell, shell_detection::*};

#[test]
fn shell_name_from_command_handles_paths_login_shells_and_arguments() {
    assert_eq!(
        shell_name_from_command("/usr/bin/zsh\n").as_deref(),
        Some("zsh")
    );
    assert_eq!(shell_name_from_command("-zsh").as_deref(), Some("zsh"));
    assert_eq!(
        shell_name_from_command("/opt/homebrew/bin/fish --login").as_deref(),
        Some("fish")
    );
    assert_eq!(shell_name_from_command("  "), None);
}

#[test]
fn detect_shell_from_command_distinguishes_supported_unsupported_and_unknown() {
    assert_eq!(
        detect_shell_from_command("/usr/bin/fish\n"),
        ShellDetection::Supported(Shell::Fish)
    );
    assert_eq!(
        detect_shell_from_command("/usr/bin/nu --login\n"),
        ShellDetection::Supported(Shell::Nu)
    );
    assert_eq!(
        detect_shell_from_command("-nushell\n"),
        ShellDetection::Supported(Shell::Nu)
    );
    assert_eq!(
        detect_shell_from_command("/usr/bin/pwsh\n"),
        ShellDetection::Supported(Shell::Pwsh)
    );
    assert_eq!(
        detect_shell_from_command("shell_integration_cli\n"),
        ShellDetection::Unknown
    );
}

#[test]
fn detect_shell_from_command_handles_windows_executable_names() {
    assert_eq!(
        detect_shell_from_command("pwsh.exe"),
        ShellDetection::Supported(Shell::Pwsh)
    );
    assert_eq!(
        detect_shell_from_command("PWSH.EXE"),
        ShellDetection::Supported(Shell::Pwsh)
    );
    assert_eq!(
        detect_shell_from_command("nu.exe"),
        ShellDetection::Supported(Shell::Nu)
    );
    assert_eq!(
        detect_shell_from_command("powershell.exe"),
        ShellDetection::Unsupported("powershell".to_string())
    );
    assert_eq!(
        detect_shell_from_command("cmd.exe"),
        ShellDetection::Unsupported("cmd".to_string())
    );
    assert_eq!(
        detect_shell_from_command("explorer.exe"),
        ShellDetection::Unknown
    );
    assert_eq!(detect_shell_from_command(".exe"), ShellDetection::Unknown);
}

#[test]
fn shell_parse_accepts_pwsh_but_not_windows_powershell() {
    assert_eq!(Shell::parse("pwsh"), Ok(Shell::Pwsh));
    assert_eq!(Shell::Pwsh.name(), "pwsh");
    assert!(Shell::parse("powershell").is_err());
    assert_eq!(
        detect_shell_from_command("/usr/bin/powershell\n"),
        ShellDetection::Unsupported("powershell".to_string())
    );
}
