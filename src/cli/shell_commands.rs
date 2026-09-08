//! Execution for the `elio shell ...` command family.

use crate::shell_integration::{self, Shell, ShellIntegrationAction};
use anyhow::Result;
use std::env;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ShellIntegrationCommand {
    Init(Shell),
    Install(Option<Shell>),
    Uninstall(Option<Shell>),
}

pub(super) fn execute(command: ShellIntegrationCommand) -> Result<()> {
    match command {
        ShellIntegrationCommand::Init(shell) => {
            let executable = env::current_exe()?;
            let invocation = env::args().next();
            let binary =
                shell_integration::binary_command(shell, invocation.as_deref(), &executable);
            print!("{}", shell_integration::init_script(shell, &binary));
        }
        ShellIntegrationCommand::Install(shell) => {
            let executable = env::current_exe()?;
            let invocation = env::args().next();
            let shell = match shell {
                Some(shell) => shell,
                None => shell_integration::detect_shell(ShellIntegrationAction::Install)?,
            };
            let binary =
                shell_integration::binary_command(shell, invocation.as_deref(), &executable);
            let report = shell_integration::install(shell, &binary)?;
            println!(
                "Installed elio shell integration for {}.",
                report.shell.name()
            );
            println!();
            println!("Wrote: {}", report.path.display());
            println!();
            println!("Restart your shell, or run:");
            println!("  {}", report.reload_command);
            println!();
            println!("From now on, `elio` will change your shell directory on quit.");
        }
        ShellIntegrationCommand::Uninstall(shell) => {
            let shell = match shell {
                Some(shell) => shell,
                None => shell_integration::detect_shell(ShellIntegrationAction::Uninstall)?,
            };
            let report = shell_integration::uninstall(shell)?;
            println!(
                "Uninstalled elio shell integration for {}.",
                report.shell.name()
            );
            println!();
            if report.changed {
                if report.removed_file {
                    println!("Removed: {}", report.path.display());
                } else {
                    println!("Updated: {}", report.path.display());
                }
            } else {
                println!("No integration found at: {}", report.path.display());
            }
            println!();
            println!("Restart your shell, or run:");
            println!("  {}", report.reload_command);
            println!();
            println!("From now on, `elio` will leave your shell directory unchanged.");
        }
    }
    Ok(())
}
