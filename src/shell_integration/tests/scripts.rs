use super::super::{Shell, binary_command, init_script, scripts::nu_string_literal};
use std::path::Path;

#[test]
fn binary_command_uses_path_for_local_invocations() {
    assert_eq!(
        binary_command(
            Shell::Bash,
            Some("target/debug/elio"),
            Path::new("/repo/target/debug/elio")
        ),
        "'/repo/target/debug/elio'"
    );
}

#[test]
fn binary_command_uses_path_for_absolute_invocations() {
    assert_eq!(
        binary_command(
            Shell::Bash,
            Some("/opt/elio/bin/elio"),
            Path::new("/opt/elio/bin/elio")
        ),
        "'/opt/elio/bin/elio'"
    );
}

#[test]
fn binary_command_uses_path_for_windows_invocations() {
    assert_eq!(
        binary_command(
            Shell::Bash,
            Some(r"C:\repo\target\debug\elio.exe"),
            Path::new(r"C:\repo\target\debug\elio.exe")
        ),
        r"'C:\repo\target\debug\elio.exe'"
    );
}

#[test]
fn binary_command_uses_path_lookup_for_normal_invocations() {
    assert_eq!(
        binary_command(Shell::Bash, Some("elio"), Path::new("/versioned/path/elio")),
        "command elio"
    );
}

#[test]
fn binary_command_formats_nu_invocations_for_run_external() {
    assert_eq!(
        binary_command(Shell::Nu, Some("elio"), Path::new("/versioned/path/elio")),
        r#""elio""#
    );
    assert_eq!(
        binary_command(
            Shell::Nu,
            Some("target/debug/elio"),
            Path::new("/repo/target/debug/elio")
        ),
        r#""/repo/target/debug/elio""#
    );
}

#[test]
fn nu_string_literal_escapes_backslashes_and_quotes() {
    assert_eq!(
        nu_string_literal(Path::new(r#"/tmp/path with spaces/eli"o\bin"#)),
        r#""/tmp/path with spaces/eli\"o\\bin""#
    );
}

#[test]
fn posix_init_script_passes_cli_commands_through() {
    let script = init_script(Shell::Bash, "command elio");

    assert!(script.contains("case \"${1-}\" in"));
    assert!(script.contains("shell|-*)"));
    assert!(script.contains("--chooser-file|--chooser-file=*)"));
    assert!(script.contains("command elio \"$@\""));
    assert!(script.contains("local arg tmp cwd status_code"));
    assert!(script.contains("command elio --cwd-file \"$tmp\" \"$@\""));
    assert!(script.contains("status_code=$?"));
    assert!(script.contains("return \"$status_code\""));
    assert!(!script.contains("local tmp cwd status\n"));
}

#[test]
fn fish_init_script_passes_cli_commands_through() {
    let script = init_script(Shell::Fish, "command elio");

    assert!(script.contains("switch \"$argv[1]\""));
    assert!(script.contains("case shell '-*'"));
    assert!(script.contains("case --chooser-file '--chooser-file=*'"));
    assert!(script.contains("command elio $argv"));
    assert!(script.contains("command elio --cwd-file \"$tmp\" $argv"));
    assert!(script.contains("cd \"$cwd\"; or return $status"));
}

#[test]
fn nu_init_script_passes_cli_commands_through_without_posix_syntax() {
    let script = init_script(Shell::Nu, r#""elio""#);

    assert!(script.contains("def --env --wrapped elio [...args]"));
    assert!(script.contains("let has_chooser_file = ($args | any"));
    assert!(script.contains("$has_chooser_file"));
    assert!(script.contains("if $has_chooser_file {"));
    assert!(script.contains("run-external \"elio\" ...$args\n        $env.LAST_EXIT_CODE"));
    assert!(script.contains("run-external \"elio\" ...$args"));
    assert!(script.contains("mktemp -t \"elio-cwd.XXXXXX\""));
    assert!(script.contains("let command_args = ([\"--cwd-file\", $tmp] ++ $args)"));
    assert!(script.contains("run-external \"elio\" ...$command_args"));
    assert!(script.contains("$env.LAST_EXIT_CODE = $status_code"));
    assert!(script.contains("$e.exit_code? | default 127"));
    assert!(script.contains("cd $cwd"));
    assert!(!script.contains("local tmp"));
    assert!(!script.contains("case \"${1-}\""));
    assert!(!script.contains("command elio"));
    assert!(!script.contains("return $status_code"));
    assert!(
        script
            .find("if $has_chooser_file {")
            .expect("chooser branch should exist")
            < script
                .find("| complete")
                .expect("complete branch should exist"),
        "chooser mode must run before the captured pass-through branch"
    );
}
