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
    let output = run_command_capture_stdout_cancellable(command, "preview-process-test", &|| {
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
