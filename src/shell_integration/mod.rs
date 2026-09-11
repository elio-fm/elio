mod install;
mod scripts;
mod shell_detection;

pub(crate) use self::install::{install, uninstall};
pub(crate) use self::scripts::{binary_command, init_script};
pub(crate) use self::shell_detection::{Shell, ShellIntegrationAction, detect_shell};

#[cfg(test)]
mod tests;
