use std::{
    env, fs,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

const CANCELLABLE_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(5);

static COMMAND_CAPTURE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn run_command_capture_stdout_cancellable<F>(
    mut command: Command,
    capture_label: &str,
    canceled: &F,
) -> Option<Vec<u8>>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    // Use a temp file instead of a pipe so long-running tools can write freely
    // while we keep polling for cancellation.
    let capture_path = command_capture_path(capture_label);
    let stdout = fs::File::create(&capture_path).ok()?;
    let mut child = match command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            let _ = fs::remove_file(&capture_path);
            return None;
        }
    };

    let status = wait_for_child_cancellable(&mut child, canceled);
    let output = status
        .filter(|status| status.success())
        .and_then(|_| fs::read(&capture_path).ok());
    let _ = fs::remove_file(&capture_path);
    output
}

pub(crate) fn run_command_status_cancellable<F>(mut command: Command, canceled: &F) -> Option<bool>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    wait_for_child_cancellable(&mut child, canceled).map(|status| status.success())
}

fn wait_for_child_cancellable<F>(child: &mut Child, canceled: &F) -> Option<ExitStatus>
where
    F: Fn() -> bool,
{
    loop {
        if canceled() {
            kill_and_wait(child);
            return None;
        }

        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => thread::sleep(CANCELLABLE_COMMAND_POLL_INTERVAL),
            Err(_) => {
                kill_and_wait(child);
                return None;
            }
        }
    }
}

fn kill_and_wait(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn command_capture_path(label: &str) -> PathBuf {
    let id = COMMAND_CAPTURE_ID.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!("elio-{label}-{}-{id}.tmp", std::process::id()))
}

#[cfg(test)]
#[path = "tests/command_cancellation.rs"]
mod tests;
