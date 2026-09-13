//! elio is a snappy, batteries-included terminal file manager.
//!
//! This crate is published for installing elio through Cargo. Its library
//! interface is intended for internal use and may change without notice.
//!
//! See the [elio documentation](https://elio-fm.github.io/) for installation,
//! configuration, and usage.

mod app;
mod archive;
mod background_jobs;
mod chooser;
mod config;
mod duplicate_finder;
mod elevated_session;
mod file_browser;
mod file_classification;
mod file_operations;
mod filesystem;
mod fuzzy_finder;
mod goto_menu;
mod input_handling;
mod opening;
mod places;
mod preview;
mod terminal_images;
mod terminal_runtime;
mod theme;
mod ui;

use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Default)]
#[doc(hidden)]
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

#[doc(hidden)]
pub fn run() -> Result<()> {
    run_with_options(RunOptions::default())
}

#[doc(hidden)]
pub fn run_at(cwd: PathBuf) -> Result<()> {
    run_with_options(RunOptions {
        start_dir: Some(cwd),
        cwd_file: None,
    })
}

#[doc(hidden)]
pub fn run_with_options(options: RunOptions) -> Result<()> {
    run_with_startup_options(options, None, false, None, None, None).map(|_| ())
}

#[doc(hidden)]
pub fn run_user_fs_helper() -> Result<()> {
    #[cfg(unix)]
    {
        elevated_session::run(background_jobs::run_user_trash_helper)
    }
    #[cfg(not(unix))]
    {
        elevated_session::run()
    }
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
    config::initialize(config_file.as_deref())?;
    theme::initialize(theme_file.as_deref())?;
    terminal_runtime::run_with_startup_state(
        options,
        start_focus,
        reveal_hidden_start_focus,
        chooser_file,
    )
}
