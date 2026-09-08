use super::{
    help_output::HelpTopic,
    options::{self, Options},
    shell_commands::ShellIntegrationCommand,
};
use crate::shell_integration::Shell;
use anyhow::{Error, Result};
use std::path::PathBuf;

const RUN_USAGE: &str = "Usage: elio [OPTIONS] [PATH]";
const SHELL_USAGE: &str = "Usage: elio shell init <SHELL>\n       elio shell install [SHELL]\n       elio shell uninstall [SHELL]";

#[derive(Debug)]
pub(super) enum Action {
    Run(Options),
    Help(HelpTopic),
    Version,
    ShellIntegration(ShellIntegrationCommand),
    UserFsHelper,
}

pub(super) fn parse(args: impl IntoIterator<Item = String>) -> Result<Action> {
    let args = args.into_iter().collect::<Vec<_>>();

    if args.is_empty() {
        return Ok(Action::Run(Options::default()));
    }

    match args.as_slice() {
        [arg] if arg == "--internal-user-fs-helper" => return Ok(Action::UserFsHelper),
        [arg] if arg == "--version" || arg == "-V" => return Ok(Action::Version),
        [arg] if is_help(arg) => return Ok(Action::Help(HelpTopic::Root)),
        [arg, unexpected, ..] if arg == "--version" || arg == "-V" => {
            return Err(unexpected_argument(unexpected));
        }
        [arg, unexpected, ..] if is_help(arg) => {
            return Err(unexpected_argument(unexpected));
        }
        _ => {}
    }

    if let Some(action) = parse_shell(&args)? {
        return Ok(action);
    }

    parse_options(args).map(Action::Run)
}

fn parse_shell(args: &[String]) -> Result<Option<Action>> {
    let [command, rest @ ..] = args else {
        return Ok(None);
    };
    if command != "shell" {
        return Ok(None);
    }

    let action = match rest {
        [help] if is_help(help) => Action::Help(HelpTopic::Shell),
        [subcommand, help] if subcommand == "init" && is_help(help) => {
            Action::Help(HelpTopic::ShellInit)
        }
        [subcommand, help] if subcommand == "install" && is_help(help) => {
            Action::Help(HelpTopic::ShellInstall)
        }
        [subcommand, help] if subcommand == "uninstall" && is_help(help) => {
            Action::Help(HelpTopic::ShellUninstall)
        }
        [subcommand, shell] if subcommand == "init" => Action::ShellIntegration(
            ShellIntegrationCommand::Init(Shell::parse(shell).map_err(Error::msg)?),
        ),
        [subcommand] if subcommand == "install" => {
            Action::ShellIntegration(ShellIntegrationCommand::Install(None))
        }
        [subcommand, shell] if subcommand == "install" => Action::ShellIntegration(
            ShellIntegrationCommand::Install(Some(Shell::parse(shell).map_err(Error::msg)?)),
        ),
        [subcommand] if subcommand == "uninstall" => {
            Action::ShellIntegration(ShellIntegrationCommand::Uninstall(None))
        }
        [subcommand, shell] if subcommand == "uninstall" => Action::ShellIntegration(
            ShellIntegrationCommand::Uninstall(Some(Shell::parse(shell).map_err(Error::msg)?)),
        ),
        [subcommand, _shell, unexpected, ..] if subcommand == "install" => {
            return Err(unexpected_argument_with_usage(
                unexpected,
                "Usage: elio shell install [SHELL]",
            ));
        }
        [subcommand, _shell, unexpected, ..] if subcommand == "uninstall" => {
            return Err(unexpected_argument_with_usage(
                unexpected,
                "Usage: elio shell uninstall [SHELL]",
            ));
        }
        [subcommand, _shell, unexpected, ..] if subcommand == "init" => {
            return Err(unexpected_argument_with_usage(
                unexpected,
                "Usage: elio shell init <SHELL>",
            ));
        }
        [subcommand] if subcommand == "init" => {
            return Err(anyhow::anyhow!(
                "error: expected a shell after 'elio shell init'\n\nsupported shells: bash, zsh, fish, nu"
            ));
        }
        _ => {
            return Err(anyhow::anyhow!(
                "error: expected subcommand 'init', 'install', or 'uninstall' after 'elio shell'\n\n{SHELL_USAGE}"
            ));
        }
    };

    Ok(Some(action))
}

fn parse_options(args: Vec<String>) -> Result<Options> {
    let mut options = Options::default();
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if path_option_matches(arg, "--cwd-file") {
            if options.cwd_file.is_some() {
                return Err(duplicate_option("--cwd-file"));
            }
            let (file, next_index) = take_path_option_value(&args, index, "--cwd-file")?;
            options.cwd_file = Some(file);
            index = next_index;
            continue;
        }

        if path_option_matches(arg, "--chooser-file") {
            if options.chooser_file.is_some() {
                return Err(duplicate_option("--chooser-file"));
            }
            let (file, next_index) = take_path_option_value(&args, index, "--chooser-file")?;
            options.chooser_file = Some(file);
            index = next_index;
            continue;
        }

        if path_option_matches(arg, "--config") {
            if options.config_file.is_some() {
                return Err(duplicate_option("--config"));
            }
            let (file, next_index) = take_path_option_value(&args, index, "--config")?;
            options.config_file = Some(file);
            index = next_index;
            continue;
        }

        if path_option_matches(arg, "--theme") {
            if options.theme_file.is_some() {
                return Err(duplicate_option("--theme"));
            }
            let (file, next_index) = take_path_option_value(&args, index, "--theme")?;
            options.theme_file = Some(file);
            index = next_index;
            continue;
        }

        if arg.starts_with('-') || options.start_dir.is_some() {
            return Err(unexpected_argument(arg));
        }
        let resolved = options::resolve_path(arg)?;
        options.start_dir = Some(resolved.directory);
        options.start_focus = resolved.focused_entry;
        options.reveal_hidden_start_focus = resolved.reveal_hidden;
        index += 1;
    }

    Ok(options)
}

fn path_option_matches(arg: &str, flag: &str) -> bool {
    arg == flag
        || arg
            .strip_prefix(flag)
            .is_some_and(|suffix| suffix.starts_with('='))
}

fn take_path_option_value(
    args: &[String],
    index: usize,
    flag: &'static str,
) -> Result<(PathBuf, usize)> {
    let arg = &args[index];
    let inline_prefix = format!("{flag}=");
    if let Some(file) = arg.strip_prefix(&inline_prefix) {
        if file.is_empty() {
            return Err(missing_path(flag));
        }
        return Ok((PathBuf::from(file), index + 1));
    }

    let Some(file) = args.get(index + 1) else {
        return Err(missing_path(flag));
    };
    Ok((PathBuf::from(file), index + 2))
}

fn is_help(arg: &str) -> bool {
    arg == "--help" || arg == "-h"
}

fn duplicate_option(flag: &str) -> Error {
    anyhow::anyhow!("error: '{flag}' cannot be used more than once\n\n{RUN_USAGE}")
}

fn missing_path(flag: &str) -> Error {
    anyhow::anyhow!("error: expected a file path after '{flag}'\n\n{RUN_USAGE}")
}

fn unexpected_argument(arg: &str) -> Error {
    unexpected_argument_with_usage(arg, RUN_USAGE)
}

fn unexpected_argument_with_usage(arg: &str, usage: &str) -> Error {
    let mut message = format!("error: unexpected argument '{arg}' found");

    if arg != "--version" && arg != "-V" && ("--version".starts_with(arg) || "-V".starts_with(arg))
    {
        message.push_str("\n\n  tip: a similar argument exists: '--version'");
    } else if arg != "--help" && arg != "-h" && ("--help".starts_with(arg) || "-h".starts_with(arg))
    {
        message.push_str("\n\n  tip: a similar argument exists: '--help'");
    } else if arg != "--cwd-file" && "--cwd-file".starts_with(arg) {
        message.push_str("\n\n  tip: a similar argument exists: '--cwd-file'");
    } else if arg != "--chooser-file" && "--chooser-file".starts_with(arg) {
        message.push_str("\n\n  tip: a similar argument exists: '--chooser-file'");
    } else if arg != "--config" && "--config".starts_with(arg) {
        message.push_str("\n\n  tip: a similar argument exists: '--config'");
    } else if arg != "--theme" && "--theme".starts_with(arg) {
        message.push_str("\n\n  tip: a similar argument exists: '--theme'");
    }

    message.push_str("\n\n");
    message.push_str(usage);
    message.push_str("\n\nFor more information, try '--help'.");
    anyhow::anyhow!(message)
}

#[cfg(test)]
#[path = "tests/arguments.rs"]
mod tests;
