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

pub(in crate::preview) fn run_command_capture_stdout_cancellable<F>(
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

pub(in crate::preview) fn run_command_status_cancellable<F>(
    mut command: Command,
    canceled: &F,
) -> Option<bool>
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
mod tests {
    use super::{run_command_capture_stdout_cancellable, run_command_status_cancellable};
    use std::{
        process::Command,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    #[cfg(not(windows))]
    fn shell_command(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.arg("-c").arg(script);
        command
    }

    #[cfg(windows)]
    fn shell_command(script: &str) -> Command {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(script);
        command
    }

    #[test]
    fn capture_helper_stops_long_running_process_promptly() {
        let canceled = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::clone(&canceled);
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            cancel_flag.store(true, Ordering::Relaxed);
        });

        #[cfg(not(windows))]
        let command = shell_command("sleep 1; printf late");
        #[cfg(windows)]
        let command = shell_command("ping -n 3 127.0.0.1 >NUL && echo late");
        let started_at = Instant::now();
        let output =
            run_command_capture_stdout_cancellable(command, "preview-process-test", &|| {
                canceled.load(Ordering::Relaxed)
            });
        cancel_thread
            .join()
            .expect("cancel thread should finish cleanly");

        assert!(
            output.is_none(),
            "canceled command output should be discarded"
        );
        assert!(
            started_at.elapsed() < Duration::from_millis(500),
            "canceled command should stop promptly"
        );
    }

    #[test]
    fn status_helper_reports_command_success() {
        let command = shell_command("exit 0");

        let result = run_command_status_cancellable(command, &|| false);

        assert_eq!(result, Some(true));
    }

    #[test]
    fn status_helper_reports_command_failure() {
        let command = shell_command("exit 7");

        let result = run_command_status_cancellable(command, &|| false);

        assert_eq!(result, Some(false));
    }
}
