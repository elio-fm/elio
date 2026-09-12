mod application_selection;
mod available_applications;
#[cfg(all(unix, not(target_os = "macos")))]
mod linux_bsd;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod terminal_editors;

pub(crate) use application_selection::ApplicationSelection;
pub(crate) use available_applications::*;
