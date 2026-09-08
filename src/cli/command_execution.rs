use super::{
    arguments::{self, Action},
    help_output,
    options::Options,
    shell_commands,
};
use anyhow::Result;
use std::{env, process::ExitCode};

pub(crate) fn run() -> Result<ExitCode> {
    dispatch(arguments::parse(env::args().skip(1))?)
}

fn dispatch(action: Action) -> Result<ExitCode> {
    match action {
        Action::Run(options) => execute_elio(options),
        Action::Help(topic) => {
            help_output::print_help(topic);
            Ok(ExitCode::SUCCESS)
        }
        Action::Version => {
            println!("elio {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        Action::ShellIntegration(command) => {
            shell_commands::execute(command)?;
            Ok(ExitCode::SUCCESS)
        }
        Action::UserFsHelper => elio::run_user_fs_helper().map(|()| ExitCode::SUCCESS),
    }
}

fn execute_elio(options: Options) -> Result<ExitCode> {
    let Options {
        start_dir,
        start_focus,
        reveal_hidden_start_focus,
        cwd_file,
        chooser_file,
        config_file,
        theme_file,
    } = options;

    elio::run_with_startup_options(
        elio::RunOptions {
            start_dir,
            cwd_file,
        },
        start_focus,
        reveal_hidden_start_focus,
        chooser_file,
        config_file,
        theme_file,
    )
    .map(exit_code)
}

fn exit_code(outcome: elio::RunOutcome) -> ExitCode {
    match outcome {
        elio::RunOutcome::Success => ExitCode::SUCCESS,
        elio::RunOutcome::Cancelled => ExitCode::FAILURE,
    }
}
