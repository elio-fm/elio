use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-{label}-{unique}"))
}

/// Wraps `s` in single quotes, escaping embedded single quotes so the result
/// is safe to embed in a POSIX shell command string.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[test]
fn launch_application_with_target_creates_a_separate_process_group() {
    let root = temp_path("detached-open");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let capture = root.join("capture.txt");
    let capture_str = capture
        .to_str()
        .expect("capture path should be valid utf-8");
    let command = format!(
        "pgid=$(ps -o pgid= -p $$ | tr -d ' '); printf '%s %s\\n' \"$$\" \"$pgid\" > {}",
        shell_quote(capture_str)
    );
    launch_application_with_target("/bin/sh", &["-c", &command], &root)
        .expect("failed to spawn fake opener");

    let mut capture_text = String::new();
    for _ in 0..300 {
        match fs::read_to_string(&capture) {
            Ok(contents) if !contents.is_empty() => {
                capture_text = contents;
                break;
            }
            _ => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let mut parts = capture_text.split_whitespace();
    let pid = parts
        .next()
        .expect("capture should contain pid")
        .parse::<i32>()
        .expect("pid should be numeric");
    let process_group = parts
        .next()
        .expect("capture should contain process group")
        .parse::<i32>()
        .expect("process group should be numeric");

    assert_eq!(process_group, pid);
    assert_ne!(process_group, unsafe { libc::getpgrp() });

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn launch_application_executes_program() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_path("detached-open-command");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let sentinel = root.join("ran");
    let script = root.join("fake-app.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\n", sentinel.display()),
    )
    .expect("failed to write sentinel script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
        .expect("failed to make sentinel script executable");

    launch_application(script.to_str().unwrap(), &[]).expect("launch_application should succeed");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while !sentinel.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let ran = sentinel.exists();
    fs::remove_dir_all(&root).ok();
    assert!(ran, "script must have run");
}

#[test]
#[cfg(not(target_os = "macos"))]
fn launch_application_reports_immediate_nonzero_exit() {
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg("exit 7");

    let result = detached_spawn_with_deadline(
        &mut command,
        std::time::Duration::from_secs(5),
        std::time::Duration::from_millis(10),
    );

    let error = result.expect_err("immediate nonzero exit should be reported");
    assert!(
        error.to_string().contains("process exited with"),
        "error should report child exit status, got: {error}"
    );
}

#[test]
#[cfg(not(target_os = "macos"))]
fn launch_application_detaches_when_deadline_expires() {
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg("sleep 1");

    let result = detached_spawn_with_deadline(
        &mut command,
        std::time::Duration::ZERO,
        std::time::Duration::from_millis(10),
    );

    assert!(
        result.is_ok(),
        "deadline expiry should detach the still-running child: {result:?}"
    );
}
