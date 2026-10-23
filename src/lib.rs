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
pub mod portal;
mod preview;
mod system_clipboard;
mod terminal_images;
mod terminal_runtime;
mod theme;
mod ui;

use anyhow::{Context, Result};
use std::path::PathBuf;

#[doc(hidden)]
pub struct PortalRoutingStatus {
    pub effective: Option<PathBuf>,
    pub value: Option<String>,
    pub state: PortalRoutingState,
}

#[doc(hidden)]
pub enum PortalRoutingState {
    Disabled,
    Managed(Vec<ManagedPortalDesktop>),
    RecoveryRequired,
}

#[doc(hidden)]
pub struct ManagedPortalDesktop {
    pub name: String,
    pub current: bool,
    pub desktop_specific: bool,
}

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
    run_with_startup_options(options, None, false, None, None, None, None).map(|_| ())
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

/// Runs the hidden portal chooser child mode over its private socket.
#[doc(hidden)]
pub fn run_portal_chooser(socket: PathBuf) -> Result<()> {
    portal::chooser_child::run(&socket)
}

/// Runs the hidden D-Bus FileChooser portal backend.
#[doc(hidden)]
pub fn run_portal_service() -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        portal::service::run()
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        anyhow::bail!("the FileChooser portal backend is supported only on Linux and FreeBSD")
    }
}

#[doc(hidden)]
pub fn ensure_portal_terminal(
    detect_terminal: impl FnOnce() -> Option<portal::terminal::TerminalAdapter>,
) -> Result<(bool, &'static str)> {
    let path = config::config_path().context("error: could not determine the elio config path")?;
    match config::ensure_portal_terminal(&path, detect_terminal)? {
        config::ConfigurePortalTerminal::Existing(terminal) => Ok((false, terminal.name())),
        config::ConfigurePortalTerminal::Configured(terminal) => Ok((true, terminal.name())),
    }
}

/// Installs the user-local portal metadata and verifies D-Bus activation.
#[doc(hidden)]
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub fn enable_portal_metadata() -> Result<portal::activation::EnableResult> {
    portal::activation::enable()
}

/// Selects elio only for the FileChooser portal interface.
#[doc(hidden)]
pub fn enable_portal_routing() -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        portal::routing::enable()
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        anyhow::bail!("the FileChooser portal backend is supported only on Linux and FreeBSD")
    }
}

/// Restarts the portal frontend so it rereads the effective routing file.
#[doc(hidden)]
pub fn reactivate_portal_frontend() -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        portal::frontend::reactivate()
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        anyhow::bail!("the FileChooser portal backend is supported only on Linux and FreeBSD")
    }
}

/// Restores every routing entry while elio still owns its FileChooser value.
#[doc(hidden)]
pub fn disable_portal_routing() -> Result<bool> {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        portal::routing::disable()
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        anyhow::bail!("the FileChooser portal backend is supported only on Linux and FreeBSD")
    }
}

/// Reports whether effective FileChooser routing still references elio.
#[doc(hidden)]
pub fn portal_routing_references_elio() -> Result<bool> {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        portal::routing::elio_referenced()
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        anyhow::bail!("the FileChooser portal backend is supported only on Linux and FreeBSD")
    }
}

/// Returns the read-only portal routing state.
#[doc(hidden)]
pub fn portal_routing_status() -> Result<PortalRoutingStatus> {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let status = portal::routing::status()?;
        let state = match status.state {
            portal::routing::StatusState::Disabled => PortalRoutingState::Disabled,
            portal::routing::StatusState::RecoveryRequired => PortalRoutingState::RecoveryRequired,
            portal::routing::StatusState::Managed(desktops) => PortalRoutingState::Managed(
                desktops
                    .into_iter()
                    .map(|desktop| ManagedPortalDesktop {
                        name: desktop.name,
                        current: desktop.current,
                        desktop_specific: desktop.desktop_specific,
                    })
                    .collect(),
            ),
        };
        Ok(PortalRoutingStatus {
            effective: status.effective,
            value: status.value,
            state,
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        anyhow::bail!("the FileChooser portal backend is supported only on Linux and FreeBSD")
    }
}

/// Removes metadata that elio can prove it created and still owns.
#[doc(hidden)]
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub fn disable_portal_metadata() -> Result<portal::activation::DisableResult> {
    portal::activation::disable()
}

/// Inspects only user-local portal metadata; it does not alter portal routing.
#[doc(hidden)]
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub fn portal_metadata_status() -> Result<portal::activation::Status> {
    portal::activation::status()
}

#[doc(hidden)]
pub fn run_with_startup_options(
    options: RunOptions,
    start_focus: Option<PathBuf>,
    reveal_hidden_start_focus: bool,
    chooser_file: Option<PathBuf>,
    save_as: Option<Option<PathBuf>>,
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
        save_as,
    )
}
