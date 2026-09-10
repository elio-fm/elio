mod app;
mod archive;
mod config;
mod elevated_session;
mod file_info;
mod fs;
mod opening;
mod preview;
mod terminal_runtime;
mod ui;

use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct RunOptions {
    pub start_dir: Option<PathBuf>,
    pub cwd_file: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum RunOutcome {
    Success,
    Cancelled,
}

pub fn run() -> Result<()> {
    run_with_options(RunOptions::default())
}

pub fn run_at(cwd: PathBuf) -> Result<()> {
    run_with_options(RunOptions {
        start_dir: Some(cwd),
        cwd_file: None,
    })
}

pub fn run_with_options(options: RunOptions) -> Result<()> {
    terminal_runtime::run_with_startup_state(options, None, false, None, None, None).map(|_| ())
}

#[doc(hidden)]
pub fn run_user_fs_helper() -> Result<()> {
    elevated_session::run()
}

#[doc(hidden)]
pub fn run_with_startup_options(
    options: RunOptions,
    start_focus: Option<PathBuf>,
    reveal_hidden_start_focus: bool,
    chooser_file: Option<PathBuf>,
    config_file: Option<PathBuf>,
    theme_file: Option<PathBuf>,
) -> Result<RunOutcome> {
    terminal_runtime::run_with_startup_state(
        options,
        start_focus,
        reveal_hidden_start_focus,
        chooser_file,
        config_file,
        theme_file,
    )
}
