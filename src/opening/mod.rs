mod application_launching;
mod open_rules;
pub(crate) mod open_with;
mod system_openers;

pub(crate) use application_launching::launch_application;
#[cfg(unix)]
pub(crate) use application_launching::launch_application_with_target;
#[cfg(unix)]
pub(crate) use open_rules::tokenize_command;
pub(crate) use open_rules::{OpenPlan, plans_for_entries};
pub(crate) use system_openers::open_in_system;
#[cfg(target_os = "macos")]
pub(crate) use system_openers::open_in_text_editor;
#[cfg(test)]
pub(crate) use system_openers::set_open_in_system_capture_for_test;
