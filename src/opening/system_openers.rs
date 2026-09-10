use std::path::Path;
#[cfg(test)]
use std::{cell::RefCell, path::PathBuf};

#[cfg(windows)]
use std::process::{Command, Stdio};
#[cfg(all(unix, not(target_os = "macos")))]
use std::{
    io,
    process::{Command, Stdio},
};

#[cfg(all(unix, not(target_os = "macos")))]
use std::os::unix::process::CommandExt;

#[cfg(test)]
thread_local! {
    static TEST_OPEN_IN_SYSTEM_CAPTURE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(crate) fn open_in_system(target: &Path) -> Result<(), String> {
    #[cfg(test)]
    if let Some(capture) = TEST_OPEN_IN_SYSTEM_CAPTURE.with(|slot| slot.borrow().clone()) {
        use std::{fs::OpenOptions, io::Write};

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&capture)
            .map_err(|error| error.to_string())?;
        if file.metadata().map_err(|error| error.to_string())?.len() > 0 {
            writeln!(file).map_err(|error| error.to_string())?;
        }
        write!(file, "{}", target.display()).map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        super::launch_application_with_target("open", &[], target)
            .map_err(|error| format!("open: {error}"))
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

        Command::new("cmd")
            .args(["/c", "start", ""])
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("cmd: {error}"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        open_unix_preferring_gio(target)
    }
}

#[cfg(test)]
pub(crate) fn set_open_in_system_capture_for_test(path: Option<PathBuf>) {
    TEST_OPEN_IN_SYSTEM_CAPTURE.with(|slot| *slot.borrow_mut() = path);
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_with_unix_backends(target: &Path, backends: &[(&str, &[&str])]) -> Result<(), String> {
    for &(program, args) in backends {
        match super::launch_application_with_target(program, args, target) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("{program}: {error}")),
        }
    }
    Err(String::from("No desktop opener available in this session"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_unix_preferring_gio(target: &Path) -> Result<(), String> {
    open_unix_preferring_gio_impl(target, "gio", "xdg-open")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_unix_preferring_gio_impl(target: &Path, gio: &str, xdg_open: &str) -> Result<(), String> {
    // gio uses GLib MIME detection, which is more consistent with desktop
    // defaults for extension- and name-based MIME matches than the xdg-open path
    // in some sessions. Use a 250ms bounded wait so gio's synchronous failures
    // can fall back. Longer-running portal startup is detached to keep opening
    // responsive; late failures after that point cannot fall back.
    open_unix_preferring_gio_with(
        gio,
        || gio_open(gio, target),
        || open_with_unix_backends(target, &[(xdg_open, &[][..])]),
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_unix_preferring_gio_with(
    gio: &str,
    gio_open: impl FnOnce() -> io::Result<()>,
    fallback_open: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match gio_open() {
        Ok(()) => return Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::Other
                    | io::ErrorKind::PermissionDenied
                    | io::ErrorKind::Interrupted
            ) => {}
        Err(error) => return Err(format!("{gio}: {error}")),
    }
    fallback_open()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn gio_open(program: &str, target: &Path) -> io::Result<()> {
    use std::time::Duration;
    const DEADLINE: Duration = Duration::from_millis(250);
    const POLL: Duration = Duration::from_millis(10);

    gio_open_with_deadline(program, target, DEADLINE, POLL)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn gio_open_with_deadline(
    program: &str,
    target: &Path,
    deadline_duration: std::time::Duration,
    poll: std::time::Duration,
) -> io::Result<()> {
    let mut command = Command::new(program);
    command.arg("open").arg(target);
    spawn_with_deadline(command, deadline_duration, poll)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_with_deadline(
    mut command: Command,
    deadline_duration: std::time::Duration,
    poll: std::time::Duration,
) -> io::Result<()> {
    use std::time::Instant;

    crate::elevated_session::prepare_external(&mut command, None)?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;

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

    // Still running past the deadline: detach it to keep opening responsive.
    // Reap the child in the background to avoid a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(all(test, unix, not(target_os = "macos")))]
#[path = "tests/system_openers.rs"]
mod tests;
