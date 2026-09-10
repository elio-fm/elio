#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::path::Path;
use std::{
    io,
    process::{Command, Stdio},
    time::Duration,
};

const DETACHED_COMMAND_DEADLINE: Duration = Duration::from_millis(250);
const DETACHED_COMMAND_POLL: Duration = Duration::from_millis(10);

#[cfg(unix)]
pub(crate) fn launch_application_with_target(
    program: &str,
    args: &[&str],
    target: &Path,
) -> io::Result<()> {
    let mut command = Command::new(program);
    command.args(args);
    command.arg(target);

    #[cfg(target_os = "macos")]
    if program == "open" {
        return status_spawn(&mut command);
    }

    detached_spawn_with_deadline(
        &mut command,
        DETACHED_COMMAND_DEADLINE,
        DETACHED_COMMAND_POLL,
    )
}

/// Spawns `program` with the given `args` detached from the terminal.
/// Unlike [`launch_application_with_target`], the target path is not appended;
/// it must already be present in `args`.
pub(crate) fn launch_application(program: &str, args: &[String]) -> io::Result<()> {
    let mut command = Command::new(program);
    command.args(args);

    #[cfg(target_os = "macos")]
    if program == "open" {
        return status_spawn(&mut command);
    }

    detached_spawn_with_deadline(
        &mut command,
        DETACHED_COMMAND_DEADLINE,
        DETACHED_COMMAND_POLL,
    )
}

fn detached_spawn_with_deadline(
    command: &mut Command,
    deadline_duration: Duration,
    poll: Duration,
) -> io::Result<()> {
    use std::time::Instant;

    prepare_external_open_command(command)?;
    prepare_detached_command(command);
    let mut child = command.spawn()?;

    let deadline = Instant::now() + deadline_duration;
    while Instant::now() < deadline {
        match child.try_wait()? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => {
                return Err(io::Error::other(format!("process exited with {status}")));
            }
            None => std::thread::sleep(poll),
        }
    }

    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn prepare_detached_command(command: &mut Command) {
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
}

#[cfg(unix)]
fn prepare_external_open_command(command: &mut Command) -> io::Result<()> {
    crate::elevated_session::prepare_external(command, None)
}

#[cfg(not(unix))]
fn prepare_external_open_command(_command: &mut Command) -> io::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn status_spawn(command: &mut Command) -> io::Result<()> {
    prepare_external_open_command(command)?;
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("process exited with {status}")))
    }
}

#[cfg(all(test, unix))]
#[path = "tests/application_launching.rs"]
mod tests;
