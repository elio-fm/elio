use super::{
    Action, HelpTopic, RUN_USAGE, Shell, ShellIntegrationCommand, duplicate_option, missing_path,
    parse, unexpected_argument,
};
use std::path::PathBuf;

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

#[test]
fn no_arguments_runs_with_defaults() {
    assert!(matches!(parse([]).unwrap(), Action::Run(_)));
}

#[test]
fn standard_actions_are_recognized() {
    assert!(matches!(
        parse(["--help".to_string()]).unwrap(),
        Action::Help(HelpTopic::Root)
    ));
    assert!(matches!(
        parse(["--version".to_string()]).unwrap(),
        Action::Version
    ));
    assert!(matches!(
        parse(["--internal-user-fs-helper".to_string()]).unwrap(),
        Action::UserFsHelper
    ));
}

#[test]
fn config_and_theme_accept_separate_and_inline_paths() {
    let action = parse(strings(&[
        "--theme=/tmp/custom-theme.toml",
        "--config",
        "/tmp/custom-config.toml",
    ]))
    .expect("config and theme options should parse");

    let Action::Run(options) = action else {
        panic!("config and theme options should run elio");
    };
    assert_eq!(
        options.config_file,
        Some(PathBuf::from("/tmp/custom-config.toml"))
    );
    assert_eq!(
        options.theme_file,
        Some(PathBuf::from("/tmp/custom-theme.toml"))
    );
}

#[test]
fn shell_help_topics_are_recognized() {
    assert!(matches!(
        parse(strings(&["shell", "--help"])).unwrap(),
        Action::Help(HelpTopic::Shell)
    ));
    assert!(matches!(
        parse(strings(&["shell", "install", "--help"])).unwrap(),
        Action::Help(HelpTopic::ShellInstall)
    ));
}

#[test]
fn shell_integration_commands_are_recognized() {
    assert!(matches!(
        parse(strings(&["shell", "init", "fish"])).unwrap(),
        Action::ShellIntegration(ShellIntegrationCommand::Init(Shell::Fish))
    ));
    assert!(matches!(
        parse(strings(&["shell", "install"])).unwrap(),
        Action::ShellIntegration(ShellIntegrationCommand::Install(None))
    ));
    assert!(matches!(
        parse(strings(&["shell", "uninstall", "nu"])).unwrap(),
        Action::ShellIntegration(ShellIntegrationCommand::Uninstall(Some(Shell::Nu)))
    ));
}

#[test]
fn duplicate_options_include_run_usage() {
    let error = duplicate_option("--config").to_string();
    assert!(error.contains("error: '--config' cannot be used more than once"));
    assert!(error.contains(RUN_USAGE));
}

#[test]
fn missing_paths_include_run_usage() {
    let error = missing_path("--theme").to_string();
    assert!(error.contains("error: expected a file path after '--theme'"));
    assert!(error.contains(RUN_USAGE));
}

#[test]
fn unexpected_arguments_suggest_similar_options() {
    let error = unexpected_argument("--chooser").to_string();
    assert!(error.contains("error: unexpected argument '--chooser' found"));
    assert!(error.contains("tip: a similar argument exists: '--chooser-file'"));
}
