mod command_destination;
mod menu_entries;

pub(crate) use self::command_destination::resolve_command_destination;
pub(crate) use self::menu_entries::{GotoDestination, GotoMenu, GotoMenuEntry, build_goto_menu};

#[cfg(test)]
mod tests;
