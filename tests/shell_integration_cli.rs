mod support;

use std::{
    fs,
    process::{Command, Output},
};
use support::{elio, temp_path};

fn assert_success(command: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{command} failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_no_stderr(command: &str, output: &Output) {
    assert!(
        output.stderr.is_empty(),
        "{command} printed stderr\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(command: &str, output: &Output) {
    assert!(
        !output.status.success(),
        "{command} unexpectedly succeeded\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_no_stdout(command: &str, output: &Output) {
    assert!(
        output.stdout.is_empty(),
        "{command} printed stdout\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_stderr_contains(command: &str, output: &Output, expected: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "{command} stderr did not contain {expected:?}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
}

fn nu_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn shell_init_fish_prints_sourceable_function() {
    let output = elio()
        .args(["shell", "init", "fish"])
        .output()
        .expect("failed to run elio shell init fish");

    assert_success("elio shell init fish", &output);
    assert_no_stderr("elio shell init fish", &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("function elio"));
    assert!(stdout.contains("switch \"$argv[1]\""));
    assert!(stdout.contains("case shell portal '-*'"));
    assert!(stdout.contains("case --chooser-file '--chooser-file=*'"));
    assert!(stdout.contains(env!("CARGO_BIN_EXE_elio")));
    assert!(stdout.contains("$argv"));
    assert!(stdout.contains("--cwd-file \"$tmp\" $argv"));
    assert!(!stdout.contains("command elio --cwd-file"));
    assert!(stdout.contains("string collect < \"$tmp\""));
    assert!(stdout.contains("return $status_code"));
}

#[test]
fn shell_init_bash_prints_function() {
    let output = elio()
        .args(["shell", "init", "bash"])
        .output()
        .expect("failed to run elio shell init bash");

    assert_success("elio shell init bash", &output);
    assert_no_stderr("elio shell init bash", &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("elio() {"));
    assert!(stdout.contains("case \"${1-}\" in"));
    assert!(stdout.contains("shell|portal|-*)"));
    assert!(stdout.contains("--chooser-file|--chooser-file=*)"));
    assert!(stdout.contains(env!("CARGO_BIN_EXE_elio")));
    assert!(stdout.contains("\"$@\""));
    assert!(stdout.contains("local arg tmp cwd status_code"));
    assert!(stdout.contains("--cwd-file \"$tmp\" \"$@\""));
    assert!(stdout.contains("status_code=$?"));
    assert!(!stdout.contains("command elio --cwd-file"));
    assert!(stdout.contains("cwd=\"$(cat -- \"$tmp\")\""));
    assert!(stdout.contains("return \"$status_code\""));
    assert!(!stdout.contains("local tmp cwd status\n"));
}

#[test]
fn shell_init_zsh_prints_function() {
    let output = elio()
        .args(["shell", "init", "zsh"])
        .output()
        .expect("failed to run elio shell init zsh");

    assert_success("elio shell init zsh", &output);
    assert_no_stderr("elio shell init zsh", &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("elio() {"));
    assert!(stdout.contains(env!("CARGO_BIN_EXE_elio")));
    assert!(stdout.contains("--chooser-file|--chooser-file=*)"));
    assert!(stdout.contains("local arg tmp cwd status_code"));
    assert!(stdout.contains("--cwd-file \"$tmp\" \"$@\""));
    assert!(stdout.contains("status_code=$?"));
    assert!(!stdout.contains("command elio --cwd-file"));
    assert!(stdout.contains("return \"$status_code\""));
    assert!(!stdout.contains("local tmp cwd status\n"));
}

#[test]
fn shell_init_nu_prints_sourceable_command() {
    let output = elio()
        .args(["shell", "init", "nu"])
        .output()
        .expect("failed to run elio shell init nu");

    assert_success("elio shell init nu", &output);
    assert_no_stderr("elio shell init nu", &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("def --env --wrapped elio [...args]"));
    assert!(stdout.contains(&nu_literal(env!("CARGO_BIN_EXE_elio"))));
    assert!(stdout.contains("run-external"));
    assert!(stdout.contains("let has_chooser_file = ($args | any"));
    assert!(stdout.contains("$has_chooser_file"));
    assert!(stdout.contains("if $has_chooser_file {"));
    assert!(stdout.contains("(($args.0 | into string) == 'portal')"));
    assert!(
        stdout
            .find("if $has_chooser_file {")
            .expect("chooser branch should exist")
            < stdout
                .find("| complete")
                .expect("complete branch should exist"),
        "chooser mode must run before the captured Nu pass-through branch"
    );
    assert!(stdout.contains("--cwd-file"));
    assert!(stdout.contains("$env.LAST_EXIT_CODE = $status_code"));
    assert!(!stdout.contains("command elio"));
    assert!(!stdout.contains("local tmp"));
}

#[test]
fn shell_init_pwsh_prints_function() {
    let output = elio()
        .args(["shell", "init", "pwsh"])
        .output()
        .expect("failed to run elio shell init pwsh");

    assert_success("elio shell init pwsh", &output);
    assert_no_stderr("elio shell init pwsh", &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("function elio {"));
    assert!(stdout.contains(env!("CARGO_BIN_EXE_elio")));
    assert!(stdout.contains("& $elioExe --cwd-file $tmp @args"));
    assert!(stdout.contains("[System.IO.Path]::GetTempFileName()"));
    assert!(stdout.contains("Set-Location -LiteralPath $cwd"));
    assert!(stdout.contains("$global:LASTEXITCODE = $statusCode"));
    assert!(!stdout.contains("& elio"));
    assert!(!stdout.contains("command elio"));
    assert!(!stdout.contains("mktemp"));
}

/// Puts a `pwsh` reporting `version` first on PATH, so installs do not depend on
/// the PowerShell installed on the machine running the tests.
#[cfg(unix)]
fn path_with_fake_pwsh(root: &std::path::Path, version: &str) -> std::ffi::OsString {
    let bin = root.join("fake-pwsh-bin");
    fs::create_dir_all(&bin).expect("fake pwsh directory should be created");
    // Written by a child process: an executable this process wrote itself can
    // still be open in a concurrently forked test, which makes running it fail.
    let status = Command::new("sh")
        .arg("-c")
        .arg("printf '#!/bin/sh\\necho \"PowerShell %s\"\\n' \"$2\" > \"$1\" && chmod +x \"$1\"")
        .arg("sh")
        .arg(bin.join("pwsh"))
        .arg(version)
        .status()
        .expect("fake pwsh should be written");
    assert!(status.success(), "fake pwsh should be written");

    let mut paths = vec![bin];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(paths).expect("PATH should be joinable")
}

// PowerShell's profile lives under the Documents known folder on Windows, which no
// environment variable can redirect, so the CLI install tests only run where the
// XDG-based location applies. Windows covers the same lifecycle in the
// shell_integration unit tests.
#[cfg(unix)]
#[test]
fn shell_install_pwsh_adds_managed_block_idempotently() {
    let root = temp_path("pwsh-install");
    let config_home = root.join("config");
    fs::create_dir_all(&config_home).expect("config home should be created");

    for _ in 0..2 {
        let output = elio()
            .args(["shell", "install", "pwsh"])
            .env("PATH", path_with_fake_pwsh(&root, "7.4.0"))
            .env("XDG_CONFIG_HOME", &config_home)
            .output()
            .expect("failed to run elio shell install pwsh");

        assert_success("elio shell install pwsh", &output);
        assert_no_stderr("elio shell install pwsh", &output);
    }

    let profile = config_home.join("powershell/Microsoft.PowerShell_profile.ps1");
    let contents = fs::read_to_string(&profile).expect("pwsh profile should be written");
    assert_eq!(
        contents.matches("# >>> elio shell integration >>>").count(),
        1
    );
    assert_eq!(
        contents.matches("# <<< elio shell integration <<<").count(),
        1
    );
    assert!(contents.contains("function elio {"));
    assert!(contents.contains(env!("CARGO_BIN_EXE_elio")));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_uninstall_pwsh_removes_managed_block_idempotently() {
    let root = temp_path("pwsh-uninstall");
    let config_home = root.join("config");
    let profile_dir = config_home.join("powershell");
    fs::create_dir_all(&profile_dir).expect("profile directory should be created");
    let profile = profile_dir.join("Microsoft.PowerShell_profile.ps1");
    fs::write(&profile, "Set-PSReadLineOption -EditMode Vi\n")
        .expect("existing profile should be written");

    let install = elio()
        .args(["shell", "install", "pwsh"])
        .env("PATH", path_with_fake_pwsh(&root, "7.5.1"))
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install pwsh");
    assert_success("elio shell install pwsh", &install);

    let uninstall = elio()
        .args(["shell", "uninstall", "pwsh"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall pwsh");

    assert_success("elio shell uninstall pwsh", &uninstall);
    assert_no_stderr("elio shell uninstall pwsh", &uninstall);
    let stdout = String::from_utf8_lossy(&uninstall.stdout);
    assert!(stdout.contains("Uninstalled elio shell integration for pwsh"));
    assert!(stdout.contains("Updated:"));
    assert!(stdout.contains(r"Remove-Item Function:\elio"));
    assert_eq!(
        fs::read_to_string(&profile).expect("profile should be readable"),
        "Set-PSReadLineOption -EditMode Vi\n"
    );

    let uninstall_again = elio()
        .args(["shell", "uninstall", "pwsh"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall pwsh again");

    assert_success("elio shell uninstall pwsh", &uninstall_again);
    assert!(String::from_utf8_lossy(&uninstall_again.stdout).contains("No integration found at:"));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_install_pwsh_rejects_versions_below_the_floor_without_touching_the_profile() {
    let root = temp_path("pwsh-version-floor");
    let config_home = root.join("config");
    let profile_dir = config_home.join("powershell");
    fs::create_dir_all(&profile_dir).expect("profile directory should be created");
    let profile = profile_dir.join("Microsoft.PowerShell_profile.ps1");
    fs::write(&profile, "Set-PSReadLineOption -EditMode Vi\n")
        .expect("existing profile should be written");

    let output = elio()
        .args(["shell", "install", "pwsh"])
        .env("PATH", path_with_fake_pwsh(&root, "7.3.12"))
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install pwsh");

    assert_failure("elio shell install pwsh", &output);
    assert_no_stdout("elio shell install pwsh", &output);
    assert_stderr_contains(
        "elio shell install pwsh",
        &output,
        "error: unsupported PowerShell version 7.3",
    );
    assert_stderr_contains(
        "elio shell install pwsh",
        &output,
        "requires PowerShell 7.4 or newer",
    );
    assert_eq!(
        fs::read_to_string(&profile).expect("profile should be readable"),
        "Set-PSReadLineOption -EditMode Vi\n"
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

// Only Linux reports a script's own name, rather than its interpreter, as the
// parent command.
#[cfg(target_os = "linux")]
#[test]
fn shell_install_detects_pwsh_parent_and_checks_its_version() {
    let root = temp_path("detect-pwsh-parent");
    let config_home = root.join("config");
    let path = path_with_fake_pwsh(&root, "7.4.6");
    let fake_pwsh = root.join("fake-pwsh-bin/pwsh");
    // The fake stands in for the parent shell too: asked for anything but its
    // version, it runs elio as a child the way an interactive pwsh would.
    let status = Command::new("sh")
        .arg("-c")
        .arg("printf '#!/bin/sh\\nif [ \"$1\" = --version ]; then echo \"PowerShell 7.4.6\"; exit 0; fi\\n\"$ELIO_TEST_BIN\" shell install\\nexit $?\\n' > \"$1\"")
        .arg("sh")
        .arg(&fake_pwsh)
        .status()
        .expect("fake pwsh should be rewritten");
    assert!(status.success(), "fake pwsh should be rewritten");

    let output = Command::new(&fake_pwsh)
        .env("ELIO_TEST_BIN", env!("CARGO_BIN_EXE_elio"))
        .env("PATH", path)
        .env("SHELL", "/usr/bin/fish")
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install from pwsh");

    assert_success("pwsh: elio shell install", &output);
    assert_no_stderr("pwsh: elio shell install", &output);
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("Installed elio shell integration for pwsh.")
    );
    assert!(
        config_home
            .join("powershell/Microsoft.PowerShell_profile.ps1")
            .exists()
    );
    assert!(!config_home.join("fish/conf.d/elio.fish").exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_install_nu_writes_autoload_file() {
    let root = temp_path("nu-install");
    let config_home = root.join("config");

    let output = elio()
        .args(["shell", "install", "nu"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install nu");

    assert_success("elio shell install nu", &output);
    assert_no_stderr("elio shell install nu", &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Installed elio shell integration for nu"));
    assert!(stdout.contains("nushell/autoload/elio.nu"));
    assert!(stdout.contains(r#"source ""#));

    let integration = config_home.join("nushell/autoload/elio.nu");
    let script = fs::read_to_string(&integration).expect("nu integration should be written");
    assert!(script.contains("def --env --wrapped elio"));
    assert!(script.contains(&nu_literal(env!("CARGO_BIN_EXE_elio"))));
    assert!(script.contains("run-external"));
    assert!(script.contains("--cwd-file"));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_install_nu_refuses_unmanaged_autoload_file() {
    let root = temp_path("nu-install-unmanaged");
    let config_home = root.join("config");
    let integration = config_home.join("nushell/autoload/elio.nu");
    fs::create_dir_all(
        integration
            .parent()
            .expect("integration should have a parent"),
    )
    .expect("nu autoload directory should be created");
    fs::write(
        &integration,
        "def elio [] {}
",
    )
    .expect("unmanaged file should be written");

    let output = elio()
        .args(["shell", "install", "nu"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install nu");

    assert_failure("elio shell install nu", &output);
    assert_no_stdout("elio shell install nu", &output);
    assert_stderr_contains("elio shell install nu", &output, "not managed by elio");
    assert_eq!(
        fs::read_to_string(&integration).expect("unmanaged file should be readable"),
        "def elio [] {}
"
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_uninstall_nu_removes_managed_autoload_file_idempotently() {
    let root = temp_path("nu-uninstall");
    let config_home = root.join("config");
    let integration = config_home.join("nushell/autoload/elio.nu");

    let install = elio()
        .args(["shell", "install", "nu"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install nu");
    assert_success("elio shell install nu", &install);

    let uninstall = elio()
        .args(["shell", "uninstall", "nu"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall nu");

    assert_success("elio shell uninstall nu", &uninstall);
    assert_no_stderr("elio shell uninstall nu", &uninstall);
    let stdout = String::from_utf8_lossy(&uninstall.stdout);
    assert!(stdout.contains("Uninstalled elio shell integration for nu"));
    assert!(stdout.contains("Removed:"));
    assert!(stdout.contains("hide elio"));
    assert!(!integration.exists());

    let uninstall_again = elio()
        .args(["shell", "uninstall", "nu"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall nu again");
    assert_success("elio shell uninstall nu", &uninstall_again);
    assert_no_stderr("elio shell uninstall nu", &uninstall_again);
    assert!(String::from_utf8_lossy(&uninstall_again.stdout).contains("No integration found at:"));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_install_nu_preserves_symlinked_autoload_directory() {
    use std::os::unix::fs::symlink;

    let root = temp_path("nu-install-autoload-symlink");
    let config_home = root.join("config");
    let nushell_dir = config_home.join("nushell");
    let dotfiles_autoload = root.join("dotfiles/nushell/autoload");
    fs::create_dir_all(&nushell_dir).expect("nushell directory should be created");
    fs::create_dir_all(&dotfiles_autoload).expect("dotfiles autoload should be created");
    symlink(&dotfiles_autoload, nushell_dir.join("autoload"))
        .expect("autoload symlink should be created");

    let output = elio()
        .args(["shell", "install", "nu"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install nu");

    assert_success("elio shell install nu", &output);
    assert_no_stderr("elio shell install nu", &output);
    assert!(dotfiles_autoload.join("elio.nu").exists());
    assert!(config_home.join("nushell/autoload/elio.nu").exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_install_nu_preserves_symlinked_autoload_file() {
    use std::os::unix::fs::symlink;

    let root = temp_path("nu-install-file-symlink");
    let config_home = root.join("config");
    let autoload = config_home.join("nushell/autoload");
    let dotfiles = root.join("dotfiles");
    fs::create_dir_all(&autoload).expect("autoload should be created");
    fs::create_dir_all(&dotfiles).expect("dotfiles should be created");
    let target = dotfiles.join("elio.nu");
    let link = autoload.join("elio.nu");
    fs::write(
        &target,
        "# >>> elio shell integration >>>
def elio [] {}
# <<< elio shell integration <<<
",
    )
    .expect("target should be written");
    symlink(&target, &link).expect("autoload file symlink should be created");

    let output = elio()
        .args(["shell", "install", "nu"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install nu");

    assert_success("elio shell install nu", &output);
    assert_no_stderr("elio shell install nu", &output);
    assert!(
        fs::symlink_metadata(&link)
            .expect("link metadata should be readable")
            .file_type()
            .is_symlink(),
        "install should preserve symlinked elio.nu"
    );
    let target_contents = fs::read_to_string(&target).expect("target should be readable");
    assert!(target_contents.contains("def --env --wrapped elio"));
    assert_eq!(
        fs::read_to_string(&link).expect("link should resolve"),
        target_contents
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_install_fish_writes_conf_d_file() {
    let root = temp_path("fish-install");
    let config_home = root.join("config");

    let output = elio()
        .args(["shell", "install", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install fish");

    assert_success("elio shell install fish", &output);
    assert_no_stderr("elio shell install fish", &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Installed elio shell integration for fish"));
    assert!(stdout.contains("conf.d/elio.fish"));

    let integration = config_home.join("fish/conf.d/elio.fish");
    let script = fs::read_to_string(&integration).expect("fish integration should be written");
    assert!(script.contains("function elio"));
    assert!(script.contains(env!("CARGO_BIN_EXE_elio")));
    assert!(script.contains("--cwd-file \"$tmp\" $argv"));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_install_fish_refuses_unmanaged_conf_d_file() {
    let root = temp_path("fish-install-unmanaged");
    let config_home = root.join("config");
    let integration = config_home.join("fish/conf.d/elio.fish");
    fs::create_dir_all(
        integration
            .parent()
            .expect("integration should have a parent"),
    )
    .expect("fish conf.d directory should be created");
    fs::write(&integration, "function elio\nend\n").expect("unmanaged file should be written");

    let output = elio()
        .args(["shell", "install", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install fish");

    assert_failure("elio shell install fish", &output);
    assert_no_stdout("elio shell install fish", &output);
    assert_stderr_contains("elio shell install fish", &output, "not managed by elio");
    assert_eq!(
        fs::read_to_string(&integration).expect("unmanaged file should be readable"),
        "function elio\nend\n"
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_install_fish_preserves_symlinked_managed_conf_d_file() {
    use std::os::unix::fs::symlink;

    let root = temp_path("fish-install-symlink");
    let config_home = root.join("config");
    let conf_d = config_home.join("fish/conf.d");
    let dotfiles = root.join("dotfiles");
    fs::create_dir_all(&conf_d).expect("fish conf.d directory should be created");
    fs::create_dir_all(&dotfiles).expect("dotfiles directory should be created");
    let target = dotfiles.join("elio.fish");
    let integration = conf_d.join("elio.fish");
    fs::write(
        &target,
        "# >>> elio shell integration >>>\nold\n# <<< elio shell integration <<<\n",
    )
    .expect("target fish integration should be written");
    symlink(&target, &integration).expect("fish integration symlink should be created");

    let output = elio()
        .args(["shell", "install", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install fish");

    assert_success("elio shell install fish", &output);
    assert_no_stderr("elio shell install fish", &output);
    assert!(
        fs::symlink_metadata(&integration)
            .expect("integration link metadata should be readable")
            .file_type()
            .is_symlink(),
        "fish install should preserve symlinked managed integration files"
    );
    let target_contents =
        fs::read_to_string(&target).expect("target integration should be readable");
    assert!(target_contents.contains("function elio"));
    assert!(target_contents.contains(env!("CARGO_BIN_EXE_elio")));
    assert_eq!(
        target_contents
            .matches("# >>> elio shell integration >>>")
            .count(),
        1
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_install_bash_adds_managed_block_idempotently() {
    let root = temp_path("bash-install");
    fs::create_dir_all(&root).expect("temp directory should be created");

    for _ in 0..2 {
        let output = elio()
            .args(["shell", "install", "bash"])
            .env("HOME", &root)
            .env("SHELL", "/bin/sh")
            .output()
            .expect("failed to run elio shell install bash");

        assert_success("elio shell install bash", &output);
        assert_no_stderr("elio shell install bash", &output);
    }

    let bashrc = root.join(".bashrc");
    let contents = fs::read_to_string(&bashrc).expect("bashrc should be written");
    assert_eq!(
        contents.matches("# >>> elio shell integration >>>").count(),
        1
    );
    assert_eq!(
        contents.matches("# <<< elio shell integration <<<").count(),
        1
    );
    assert!(contents.contains("elio() {"));
    assert!(contents.contains(env!("CARGO_BIN_EXE_elio")));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_install_bash_preserves_symlinked_startup_file() {
    use std::os::unix::fs::symlink;

    let root = temp_path("bash-install-symlink");
    let home = root.join("home");
    let dotfiles = root.join("dotfiles");
    fs::create_dir_all(&home).expect("home directory should be created");
    fs::create_dir_all(&dotfiles).expect("dotfiles directory should be created");
    let target = dotfiles.join("bashrc");
    let bashrc = home.join(".bashrc");
    fs::write(&target, "export EDITOR=nvim\n").expect("target bashrc should be written");
    symlink(&target, &bashrc).expect("bashrc symlink should be created");

    let output = elio()
        .args(["shell", "install", "bash"])
        .env("HOME", &home)
        .output()
        .expect("failed to run elio shell install bash");

    assert_success("elio shell install bash", &output);
    assert_no_stderr("elio shell install bash", &output);
    assert!(
        fs::symlink_metadata(&bashrc)
            .expect("bashrc link metadata should be readable")
            .file_type()
            .is_symlink(),
        "install should preserve symlinked bashrc"
    );
    let target_contents = fs::read_to_string(&target).expect("target bashrc should be readable");
    assert!(target_contents.contains("export EDITOR=nvim"));
    assert!(target_contents.contains("# >>> elio shell integration >>>"));
    assert_eq!(
        fs::read_to_string(&bashrc).expect("bashrc symlink should resolve"),
        target_contents
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_install_zsh_uses_zdotdir_when_set() {
    let root = temp_path("zsh-install-zdotdir");
    let home = root.join("home");
    let zdotdir = root.join("zsh-config");
    fs::create_dir_all(&home).expect("home directory should be created");

    let output = elio()
        .args(["shell", "install", "zsh"])
        .env("HOME", &home)
        .env("ZDOTDIR", &zdotdir)
        .output()
        .expect("failed to run elio shell install zsh");

    assert_success("elio shell install zsh", &output);
    assert_no_stderr("elio shell install zsh", &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let zdot_zshrc = zdotdir.join(".zshrc");
    assert!(stdout.contains(&format!("Wrote: {}", zdot_zshrc.display())));
    assert!(stdout.contains(&format!("source '{}'", zdot_zshrc.display())));
    assert!(zdot_zshrc.exists());
    assert!(!home.join(".zshrc").exists());
    let contents = fs::read_to_string(&zdot_zshrc).expect("ZDOTDIR zshrc should be readable");
    assert!(contents.contains("# >>> elio shell integration >>>"));
    assert!(contents.contains(env!("CARGO_BIN_EXE_elio")));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_install_zsh_uses_zdotdir_and_preserves_symlinked_startup_file() {
    use std::os::unix::fs::symlink;

    let root = temp_path("zsh-install-zdotdir-symlink");
    let home = root.join("home");
    let zdotdir = root.join("zsh-config");
    let dotfiles = root.join("dotfiles");
    fs::create_dir_all(&home).expect("home directory should be created");
    fs::create_dir_all(&zdotdir).expect("ZDOTDIR should be created");
    fs::create_dir_all(&dotfiles).expect("dotfiles directory should be created");
    let target = dotfiles.join("zshrc");
    let zshrc = zdotdir.join(".zshrc");
    fs::write(&target, "export EDITOR=nvim\n").expect("target zshrc should be written");
    symlink(&target, &zshrc).expect("ZDOTDIR zshrc symlink should be created");

    let output = elio()
        .args(["shell", "install", "zsh"])
        .env("HOME", &home)
        .env("ZDOTDIR", &zdotdir)
        .output()
        .expect("failed to run elio shell install zsh");

    assert_success("elio shell install zsh", &output);
    assert_no_stderr("elio shell install zsh", &output);
    assert!(
        fs::symlink_metadata(&zshrc)
            .expect("ZDOTDIR zshrc link metadata should be readable")
            .file_type()
            .is_symlink(),
        "install should preserve symlinked ZDOTDIR zshrc"
    );
    let target_contents = fs::read_to_string(&target).expect("target zshrc should be readable");
    assert!(target_contents.contains("export EDITOR=nvim"));
    assert!(target_contents.contains("# >>> elio shell integration >>>"));
    assert_eq!(
        fs::read_to_string(&zshrc).expect("ZDOTDIR zshrc symlink should resolve"),
        target_contents
    );
    assert!(!home.join(".zshrc").exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_uninstall_zsh_uses_zdotdir_and_preserves_symlinked_startup_file() {
    use std::os::unix::fs::symlink;

    let root = temp_path("zsh-uninstall-zdotdir-symlink");
    let home = root.join("home");
    let zdotdir = root.join("zsh-config");
    let dotfiles = root.join("dotfiles");
    fs::create_dir_all(&home).expect("home directory should be created");
    fs::create_dir_all(&zdotdir).expect("ZDOTDIR should be created");
    fs::create_dir_all(&dotfiles).expect("dotfiles directory should be created");
    let target = dotfiles.join("zshrc");
    let zshrc = zdotdir.join(".zshrc");
    fs::write(&target, "export EDITOR=nvim\n").expect("target zshrc should be written");
    symlink(&target, &zshrc).expect("ZDOTDIR zshrc symlink should be created");

    let install = elio()
        .args(["shell", "install", "zsh"])
        .env("HOME", &home)
        .env("ZDOTDIR", &zdotdir)
        .output()
        .expect("failed to run elio shell install zsh");
    assert_success("elio shell install zsh", &install);

    let uninstall = elio()
        .args(["shell", "uninstall", "zsh"])
        .env("HOME", &home)
        .env("ZDOTDIR", &zdotdir)
        .output()
        .expect("failed to run elio shell uninstall zsh");

    assert_success("elio shell uninstall zsh", &uninstall);
    assert_no_stderr("elio shell uninstall zsh", &uninstall);
    let stdout = String::from_utf8_lossy(&uninstall.stdout);
    assert!(stdout.contains(&format!("Updated: {}", zshrc.display())));
    assert!(
        fs::symlink_metadata(&zshrc)
            .expect("ZDOTDIR zshrc link metadata should be readable")
            .file_type()
            .is_symlink(),
        "uninstall should preserve symlinked ZDOTDIR zshrc"
    );
    assert_eq!(
        fs::read_to_string(&target).expect("target zshrc should be readable"),
        "export EDITOR=nvim\n"
    );
    assert!(!home.join(".zshrc").exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_uninstall_zsh_preserves_symlinked_startup_file() {
    use std::os::unix::fs::symlink;

    let root = temp_path("zsh-uninstall-symlink");
    let home = root.join("home");
    let dotfiles = root.join("dotfiles");
    fs::create_dir_all(&home).expect("home directory should be created");
    fs::create_dir_all(&dotfiles).expect("dotfiles directory should be created");
    let target = dotfiles.join("zshrc");
    let zshrc = home.join(".zshrc");
    fs::write(&target, "export EDITOR=nvim\n").expect("target zshrc should be written");
    symlink(&target, &zshrc).expect("zshrc symlink should be created");

    let install = elio()
        .args(["shell", "install", "zsh"])
        .env("HOME", &home)
        .output()
        .expect("failed to run elio shell install zsh");
    assert_success("elio shell install zsh", &install);

    let uninstall = elio()
        .args(["shell", "uninstall", "zsh"])
        .env("HOME", &home)
        .output()
        .expect("failed to run elio shell uninstall zsh");

    assert_success("elio shell uninstall zsh", &uninstall);
    assert_no_stderr("elio shell uninstall zsh", &uninstall);
    assert!(
        fs::symlink_metadata(&zshrc)
            .expect("zshrc link metadata should be readable")
            .file_type()
            .is_symlink(),
        "uninstall should preserve symlinked zshrc"
    );
    assert_eq!(
        fs::read_to_string(&target).expect("target zshrc should be readable"),
        "export EDITOR=nvim\n"
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_install_bash_rejects_non_utf8_startup_file() {
    let root = temp_path("bash-install-non-utf8");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let bashrc = root.join(".bashrc");
    fs::write(&bashrc, [0xff, b'a']).expect("bashrc should be written");

    let output = elio()
        .args(["shell", "install", "bash"])
        .env("HOME", &root)
        .output()
        .expect("failed to run elio shell install bash");

    assert_failure("elio shell install bash", &output);
    assert_no_stdout("elio shell install bash", &output);
    assert_stderr_contains("elio shell install bash", &output, "failed to read");
    assert_stderr_contains("elio shell install bash", &output, "as UTF-8");
    assert_eq!(
        fs::read(&bashrc).expect("bashrc should still be readable"),
        vec![0xff, b'a']
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_install_detects_shell_from_environment() {
    let root = temp_path("detect-install");
    let config_home = root.join("config");

    let output = elio()
        .args(["shell", "install"])
        .env("SHELL", "/usr/bin/fish")
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install");

    assert_success("elio shell install", &output);
    assert_no_stderr("elio shell install", &output);
    assert!(config_home.join("fish/conf.d/elio.fish").exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_install_does_not_guess_powershell_from_inherited_environment() {
    let root = temp_path("no-pwsh-guess-install");
    let config_home = root.join("config");

    let output = elio()
        .args(["shell", "install"])
        .env_remove("SHELL")
        .env("PSModulePath", "/usr/local/share/powershell/Modules")
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install");

    assert_failure("elio shell install", &output);
    assert_no_stdout("elio shell install", &output);
    assert_stderr_contains(
        "elio shell install",
        &output,
        "error: could not detect your shell from the parent process or $SHELL",
    );
    assert_stderr_contains("elio shell install", &output, "elio shell install pwsh");
    assert!(!config_home.exists());
}

#[cfg(windows)]
fn elio_from_cmd(subcommand: &str) -> Output {
    use std::os::windows::process::CommandExt;

    Command::new("cmd")
        .raw_arg(format!("/C \"\"%ELIO_TEST_BIN%\" shell {subcommand}\""))
        .env("ELIO_TEST_BIN", env!("CARGO_BIN_EXE_elio"))
        .env_remove("SHELL")
        .output()
        .expect("failed to run elio from cmd.exe")
}

#[cfg(windows)]
fn elio_from_windows_powershell(subcommand: &str) -> Output {
    Command::new("powershell")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!(
            "& $env:ELIO_TEST_BIN shell {subcommand}; exit $LASTEXITCODE"
        ))
        .env("ELIO_TEST_BIN", env!("CARGO_BIN_EXE_elio"))
        .env_remove("SHELL")
        .output()
        .expect("failed to run elio from powershell.exe")
}

#[cfg(windows)]
#[test]
fn shell_install_and_uninstall_refuse_cmd_as_the_active_shell() {
    for subcommand in ["install", "uninstall"] {
        let command = format!("cmd /C elio shell {subcommand}");
        let output = elio_from_cmd(subcommand);

        assert_failure(&command, &output);
        assert_no_stdout(&command, &output);
        assert_stderr_contains(&command, &output, "error: unsupported active shell 'cmd'");
        assert_stderr_contains(&command, &output, &format!("elio shell {subcommand} pwsh"));
    }
}

#[cfg(windows)]
#[test]
fn shell_install_and_uninstall_refuse_windows_powershell_as_the_active_shell() {
    for subcommand in ["install", "uninstall"] {
        let command = format!("powershell -Command elio shell {subcommand}");
        let output = elio_from_windows_powershell(subcommand);

        assert_failure(&command, &output);
        assert_no_stdout(&command, &output);
        assert_stderr_contains(
            &command,
            &output,
            "error: unsupported active shell 'powershell'",
        );
        assert_stderr_contains(&command, &output, "use PowerShell 7.4+ (pwsh) instead");
    }
}

#[test]
fn shell_install_detects_current_parent_shell_before_login_shell_environment() {
    if Command::new("zsh").arg("-c").arg(":").status().is_err() {
        return;
    }

    let root = temp_path("detect-parent-shell-install");
    let home = root.join("home");
    let zdotdir = root.join("zsh-config");
    let fish_config = root.join("fish-config");
    fs::create_dir_all(&home).expect("home directory should be created");
    fs::create_dir_all(&zdotdir).expect("ZDOTDIR should be created");

    let output = Command::new("zsh")
        .arg("-c")
        .arg("\"$ELIO_TEST_BIN\" shell install; :")
        .env("ELIO_TEST_BIN", env!("CARGO_BIN_EXE_elio"))
        .env("SHELL", "/usr/bin/fish")
        .env("HOME", &home)
        .env("ZDOTDIR", &zdotdir)
        .env("XDG_CONFIG_HOME", &fish_config)
        .output()
        .expect("failed to run elio shell install from zsh");

    assert_success("zsh -c elio shell install", &output);
    assert_no_stderr("zsh -c elio shell install", &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let zshrc = zdotdir.join(".zshrc");
    assert!(stdout.contains("Installed elio shell integration for zsh."));
    assert!(stdout.contains(&format!("Wrote: {}", zshrc.display())));
    assert!(zshrc.exists());
    assert!(!fish_config.join("fish/conf.d/elio.fish").exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_install_rejects_unsupported_current_shell_before_login_shell_environment() {
    if Command::new("sh").arg("-c").arg(":").status().is_err() {
        return;
    }

    let root = temp_path("reject-unsupported-parent-install");
    let config_home = root.join("config");
    fs::create_dir_all(&root).expect("temp directory should be created");

    let output = Command::new("sh")
        .arg("-c")
        .arg("\"$ELIO_TEST_BIN\" shell install; status=$?; exit \"$status\"")
        .env("ELIO_TEST_BIN", env!("CARGO_BIN_EXE_elio"))
        .env("SHELL", "/usr/bin/fish")
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install from sh");

    assert_failure("sh -c elio shell install", &output);
    assert_no_stdout("sh -c elio shell install", &output);
    assert_stderr_contains(
        "sh -c elio shell install",
        &output,
        "error: unsupported active shell 'sh'",
    );
    assert_stderr_contains(
        "sh -c elio shell install",
        &output,
        "elio shell install fish",
    );
    assert!(!config_home.join("fish/conf.d/elio.fish").exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_install_explicit_target_works_from_unsupported_current_shell() {
    if Command::new("sh").arg("-c").arg(":").status().is_err() {
        return;
    }

    let root = temp_path("explicit-install-from-unsupported-parent");
    let config_home = root.join("config");

    let output = Command::new("sh")
        .arg("-c")
        .arg("\"$ELIO_TEST_BIN\" shell install fish; status=$?; exit \"$status\"")
        .env("ELIO_TEST_BIN", env!("CARGO_BIN_EXE_elio"))
        .env("SHELL", "/bin/sh")
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install fish from sh");

    assert_success("sh -c elio shell install fish", &output);
    assert_no_stderr("sh -c elio shell install fish", &output);
    assert!(config_home.join("fish/conf.d/elio.fish").exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_uninstall_rejects_unsupported_current_shell_before_login_shell_environment() {
    if Command::new("sh").arg("-c").arg(":").status().is_err() {
        return;
    }

    let root = temp_path("reject-unsupported-parent-uninstall");
    let config_home = root.join("config");
    let conf_d = config_home.join("fish/conf.d");
    let integration = conf_d.join("elio.fish");
    fs::create_dir_all(&conf_d).expect("fish conf.d should be created");
    fs::write(
        &integration,
        "# >>> elio shell integration >>>\nfunction elio\nend\n# <<< elio shell integration <<<\n",
    )
    .expect("fish integration should be written");

    let output = Command::new("sh")
        .arg("-c")
        .arg("\"$ELIO_TEST_BIN\" shell uninstall; status=$?; exit \"$status\"")
        .env("ELIO_TEST_BIN", env!("CARGO_BIN_EXE_elio"))
        .env("SHELL", "/usr/bin/fish")
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall from sh");

    assert_failure("sh -c elio shell uninstall", &output);
    assert_no_stdout("sh -c elio shell uninstall", &output);
    assert_stderr_contains(
        "sh -c elio shell uninstall",
        &output,
        "error: unsupported active shell 'sh'",
    );
    assert_stderr_contains(
        "sh -c elio shell uninstall",
        &output,
        "elio shell uninstall fish",
    );
    assert!(
        integration.exists(),
        "uninstall should not fall back to the login shell"
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_uninstall_bash_removes_managed_block_idempotently() {
    let root = temp_path("bash-uninstall");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let bashrc = root.join(".bashrc");
    fs::write(&bashrc, "export EDITOR=nvim\n").expect("bashrc should be written");

    let install = elio()
        .args(["shell", "install", "bash"])
        .env("HOME", &root)
        .output()
        .expect("failed to run elio shell install bash");
    assert_success("elio shell install bash", &install);

    let uninstall = elio()
        .args(["shell", "uninstall", "bash"])
        .env("HOME", &root)
        .output()
        .expect("failed to run elio shell uninstall bash");

    assert_success("elio shell uninstall bash", &uninstall);
    assert_no_stderr("elio shell uninstall bash", &uninstall);
    let stdout = String::from_utf8_lossy(&uninstall.stdout);
    assert!(stdout.contains("Uninstalled elio shell integration for bash"));
    assert!(stdout.contains("Updated:"));
    assert!(stdout.contains("unset -f elio"));
    assert_eq!(
        fs::read_to_string(&bashrc).expect("bashrc should be readable"),
        "export EDITOR=nvim\n"
    );

    let uninstall_again = elio()
        .args(["shell", "uninstall", "bash"])
        .env("HOME", &root)
        .output()
        .expect("failed to run elio shell uninstall bash again");

    assert_success("elio shell uninstall bash", &uninstall_again);
    assert_no_stderr("elio shell uninstall bash", &uninstall_again);
    assert!(String::from_utf8_lossy(&uninstall_again.stdout).contains("No integration found at:"));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_install_fish_preserves_symlinked_conf_d_directory() {
    use std::os::unix::fs::symlink;

    let root = temp_path("fish-install-conf-d-symlink");
    let config_home = root.join("config");
    let fish_dir = config_home.join("fish");
    let dotfiles_conf_d = root.join("dotfiles/fish/conf.d");
    fs::create_dir_all(&fish_dir).expect("fish directory should be created");
    fs::create_dir_all(&dotfiles_conf_d).expect("dotfiles conf.d should be created");
    let conf_d = fish_dir.join("conf.d");
    symlink(&dotfiles_conf_d, &conf_d).expect("fish conf.d symlink should be created");

    let output = elio()
        .args(["shell", "install", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install fish");

    assert_success("elio shell install fish", &output);
    assert_no_stderr("elio shell install fish", &output);
    assert!(
        fs::symlink_metadata(&conf_d)
            .expect("fish conf.d link metadata should be readable")
            .file_type()
            .is_symlink(),
        "install should preserve symlinked fish conf.d directory"
    );
    let target_in_dotfiles = dotfiles_conf_d.join("elio.fish");
    assert!(target_in_dotfiles.exists());
    assert_eq!(
        fs::read_to_string(conf_d.join("elio.fish")).expect("integration should resolve"),
        fs::read_to_string(&target_in_dotfiles).expect("target integration should be readable")
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_uninstall_fish_preserves_symlinked_conf_d_directory() {
    use std::os::unix::fs::symlink;

    let root = temp_path("fish-uninstall-conf-d-symlink");
    let config_home = root.join("config");
    let fish_dir = config_home.join("fish");
    let dotfiles_conf_d = root.join("dotfiles/fish/conf.d");
    fs::create_dir_all(&fish_dir).expect("fish directory should be created");
    fs::create_dir_all(&dotfiles_conf_d).expect("dotfiles conf.d should be created");
    let conf_d = fish_dir.join("conf.d");
    symlink(&dotfiles_conf_d, &conf_d).expect("fish conf.d symlink should be created");

    let install = elio()
        .args(["shell", "install", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install fish");
    assert_success("elio shell install fish", &install);

    let uninstall = elio()
        .args(["shell", "uninstall", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall fish");

    assert_success("elio shell uninstall fish", &uninstall);
    assert_no_stderr("elio shell uninstall fish", &uninstall);
    assert!(
        fs::symlink_metadata(&conf_d)
            .expect("fish conf.d link metadata should be readable")
            .file_type()
            .is_symlink(),
        "uninstall should preserve symlinked fish conf.d directory"
    );
    assert!(!dotfiles_conf_d.join("elio.fish").exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_uninstall_fish_removes_managed_conf_d_file() {
    let root = temp_path("fish-uninstall");
    let config_home = root.join("config");

    let install = elio()
        .args(["shell", "install", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell install fish");
    assert_success("elio shell install fish", &install);

    let integration = config_home.join("fish/conf.d/elio.fish");
    assert!(integration.exists());

    let uninstall = elio()
        .args(["shell", "uninstall", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall fish");

    assert_success("elio shell uninstall fish", &uninstall);
    assert_no_stderr("elio shell uninstall fish", &uninstall);
    let stdout = String::from_utf8_lossy(&uninstall.stdout);
    assert!(stdout.contains("Uninstalled elio shell integration for fish"));
    assert!(stdout.contains("Removed:"));
    assert!(stdout.contains("functions --erase elio"));
    assert!(!integration.exists());

    let uninstall_again = elio()
        .args(["shell", "uninstall", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall fish again");

    assert_success("elio shell uninstall fish", &uninstall_again);
    assert_no_stderr("elio shell uninstall fish", &uninstall_again);
    assert!(String::from_utf8_lossy(&uninstall_again.stdout).contains("No integration found at:"));

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_uninstall_fish_refuses_unmanaged_conf_d_file() {
    let root = temp_path("fish-uninstall-unmanaged");
    let config_home = root.join("config");
    let integration = config_home.join("fish/conf.d/elio.fish");
    fs::create_dir_all(
        integration
            .parent()
            .expect("integration should have a parent"),
    )
    .expect("fish conf.d directory should be created");
    fs::write(&integration, "function elio\nend\n").expect("unmanaged file should be written");

    let output = elio()
        .args(["shell", "uninstall", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall fish");

    assert_failure("elio shell uninstall fish", &output);
    assert_no_stdout("elio shell uninstall fish", &output);
    assert_stderr_contains("elio shell uninstall fish", &output, "not managed by elio");
    assert!(integration.exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[cfg(unix)]
#[test]
fn shell_uninstall_fish_removes_symlink_but_preserves_target() {
    use std::os::unix::fs::symlink;

    let root = temp_path("fish-uninstall-symlink");
    let config_home = root.join("config");
    let conf_d = config_home.join("fish/conf.d");
    let dotfiles = root.join("dotfiles");
    fs::create_dir_all(&conf_d).expect("fish conf.d directory should be created");
    fs::create_dir_all(&dotfiles).expect("dotfiles directory should be created");
    let target = dotfiles.join("elio.fish");
    let integration = conf_d.join("elio.fish");
    fs::write(
        &target,
        "# >>> elio shell integration >>>\nfunction elio\nend\n# <<< elio shell integration <<<\n",
    )
    .expect("target fish integration should be written");
    symlink(&target, &integration).expect("fish integration symlink should be created");

    let output = elio()
        .args(["shell", "uninstall", "fish"])
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .expect("failed to run elio shell uninstall fish");

    assert_success("elio shell uninstall fish", &output);
    assert_no_stderr("elio shell uninstall fish", &output);
    assert!(
        !integration.exists(),
        "uninstall should remove the active fish conf.d entry"
    );
    assert!(
        target.exists(),
        "uninstall should not delete a symlink target outside conf.d"
    );
    assert!(
        fs::read_to_string(&target)
            .expect("target fish integration should remain readable")
            .contains("function elio")
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn shell_init_rejects_unsupported_shell() {
    let output = elio()
        .args(["shell", "init", "powershell"])
        .output()
        .expect("failed to run elio shell init powershell");

    assert_failure("elio shell init powershell", &output);
    assert_no_stdout("elio shell init powershell", &output);
    assert_stderr_contains(
        "elio shell init powershell",
        &output,
        "error: unsupported shell 'powershell'",
    );
    assert_stderr_contains(
        "elio shell init powershell",
        &output,
        "supported shells: bash, zsh, fish, nu, pwsh",
    );
}

#[test]
fn cwd_file_requires_value() {
    let output = elio()
        .arg("--cwd-file")
        .output()
        .expect("failed to run elio --cwd-file");

    assert_failure("elio --cwd-file", &output);
    assert_no_stdout("elio --cwd-file", &output);
    assert_stderr_contains(
        "elio --cwd-file",
        &output,
        "error: expected a file path after '--cwd-file'",
    );
}

#[test]
fn cwd_file_equals_requires_value() {
    let output = elio()
        .arg("--cwd-file=")
        .output()
        .expect("failed to run elio --cwd-file=");

    assert_failure("elio --cwd-file=", &output);
    assert_no_stdout("elio --cwd-file=", &output);
    assert_stderr_contains(
        "elio --cwd-file=",
        &output,
        "error: expected a file path after '--cwd-file'",
    );
}

#[test]
fn chooser_file_requires_value() {
    let output = elio()
        .arg("--chooser-file")
        .output()
        .expect("failed to run elio --chooser-file");

    assert_failure("elio --chooser-file", &output);
    assert_no_stdout("elio --chooser-file", &output);
    assert_stderr_contains(
        "elio --chooser-file",
        &output,
        "error: expected a file path after '--chooser-file'",
    );
}

#[test]
fn chooser_file_equals_requires_value() {
    let output = elio()
        .arg("--chooser-file=")
        .output()
        .expect("failed to run elio --chooser-file=");

    assert_failure("elio --chooser-file=", &output);
    assert_no_stdout("elio --chooser-file=", &output);
    assert_stderr_contains(
        "elio --chooser-file=",
        &output,
        "error: expected a file path after '--chooser-file'",
    );
}

#[test]
fn duplicate_cwd_file_is_rejected() {
    let first = temp_path("cwd-first");
    let second = temp_path("cwd-second");
    let output = elio()
        .arg("--cwd-file")
        .arg(&first)
        .arg("--cwd-file")
        .arg(&second)
        .output()
        .expect("failed to run elio with duplicate --cwd-file");

    assert_failure("elio with duplicate --cwd-file", &output);
    assert_no_stdout("elio with duplicate --cwd-file", &output);
    assert_stderr_contains(
        "elio with duplicate --cwd-file",
        &output,
        "error: '--cwd-file' cannot be used more than once",
    );
}

#[test]
fn duplicate_chooser_file_is_rejected() {
    let first = temp_path("chooser-first");
    let second = temp_path("chooser-second");
    let output = elio()
        .arg("--chooser-file")
        .arg(&first)
        .arg("--chooser-file")
        .arg(&second)
        .output()
        .expect("failed to run elio with duplicate --chooser-file");

    assert_failure("elio with duplicate --chooser-file", &output);
    assert_no_stdout("elio with duplicate --chooser-file", &output);
    assert_stderr_contains(
        "elio with duplicate --chooser-file",
        &output,
        "error: '--chooser-file' cannot be used more than once",
    );
}

#[test]
fn duplicate_chooser_file_equals_is_rejected() {
    let first = temp_path("chooser-first");
    let second = temp_path("chooser-second");
    let output = elio()
        .arg(format!("--chooser-file={}", first.display()))
        .arg(format!("--chooser-file={}", second.display()))
        .output()
        .expect("failed to run elio with duplicate --chooser-file");

    assert_failure("elio with duplicate --chooser-file=", &output);
    assert_no_stdout("elio with duplicate --chooser-file=", &output);
    assert_stderr_contains(
        "elio with duplicate --chooser-file=",
        &output,
        "error: '--chooser-file' cannot be used more than once",
    );
}

#[test]
fn duplicate_cwd_file_equals_is_rejected() {
    let first = temp_path("cwd-first");
    let second = temp_path("cwd-second");
    let output = elio()
        .arg(format!("--cwd-file={}", first.display()))
        .arg(format!("--cwd-file={}", second.display()))
        .output()
        .expect("failed to run elio with duplicate --cwd-file");

    assert_failure("elio with duplicate --cwd-file=", &output);
    assert_no_stdout("elio with duplicate --cwd-file=", &output);
    assert_stderr_contains(
        "elio with duplicate --cwd-file=",
        &output,
        "error: '--cwd-file' cannot be used more than once",
    );
}
