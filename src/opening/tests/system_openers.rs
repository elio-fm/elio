use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-{label}-{unique}"))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[test]
fn uses_first_available_unix_backend() {
    let root = temp_path("open-backends-first");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let capture = root.join("capture.txt");
    let capture_str = capture
        .to_str()
        .expect("capture path should be valid utf-8");
    let command = format!("printf 'xdg-open' > {}", shell_quote(capture_str));

    let result = open_with_unix_backends(
        &capture,
        &[
            ("/bin/sh", &["-c", &command][..]),
            ("this-program-does-not-exist-elio", &[][..]),
        ],
    );

    assert!(result.is_ok(), "expected Ok, got {result:?}");

    for _ in 0..300 {
        match fs::read_to_string(&capture) {
            Ok(contents) if !contents.is_empty() => break,
            _ => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let recorded = fs::read_to_string(&capture).expect("capture should exist");
    assert_eq!(recorded.trim(), "xdg-open");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn skips_missing_unix_backend_and_tries_next() {
    let root = temp_path("open-backends-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let capture = root.join("capture.txt");
    let capture_str = capture
        .to_str()
        .expect("capture path should be valid utf-8");
    let command = format!("printf 'gio' > {}", shell_quote(capture_str));
    let result = open_with_unix_backends(
        &capture,
        &[
            ("this-program-does-not-exist-elio", &[][..]),
            ("/bin/sh", &["-c", &command][..]),
        ],
    );

    assert!(result.is_ok(), "expected Ok after fallback, got {result:?}");

    let mut recorded = String::new();
    for _ in 0..300 {
        match fs::read_to_string(&capture) {
            Ok(contents) if !contents.is_empty() => {
                recorded = contents;
                break;
            }
            _ => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(recorded.trim(), "gio");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn reports_when_all_unix_backends_are_missing() {
    let result = open_with_unix_backends(
        Path::new("/tmp/anything"),
        &[
            ("this-program-does-not-exist-elio-a", &[][..]),
            ("this-program-does-not-exist-elio-b", &[][..]),
        ],
    );

    assert_eq!(
        result.unwrap_err(),
        "No desktop opener available in this session"
    );
}

#[test]
fn propagates_non_notfound_backend_errors_immediately() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_path("open-backends-permerror");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let not_executable = root.join("not-executable");
    fs::write(&not_executable, "#!/bin/sh\n").expect("failed to write file");
    let mut permissions = fs::metadata(&not_executable).unwrap().permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(&not_executable, permissions).unwrap();

    let script = root.join("should-not-run");
    fs::write(&script, "#!/bin/sh\n").expect("failed to write script");

    let result = open_with_unix_backends(
        Path::new("/tmp/anything"),
        &[
            (not_executable.to_str().unwrap(), &[][..]),
            (script.to_str().unwrap(), &[][..]),
        ],
    );

    let error = result.unwrap_err();
    assert!(
        error.contains("not-executable"),
        "error should name the failing backend, got: {error}"
    );
    assert!(
        !error.contains("should-not-run"),
        "second backend should not appear in error, got: {error}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn prefers_gio_when_available() {
    use std::cell::Cell;

    let fallback_called = Cell::new(false);
    let result = open_unix_preferring_gio_with(
        "gio",
        || Ok(()),
        || {
            fallback_called.set(true);
            Ok(())
        },
    );

    assert!(result.is_ok(), "expected Ok, got {result:?}");
    assert!(!fallback_called.get(), "fallback should not run");
}

#[test]
fn falls_back_when_gio_is_missing() {
    use std::cell::Cell;

    let fallback_called = Cell::new(false);
    let result = open_unix_preferring_gio_with(
        "gio",
        || Err(io::Error::new(io::ErrorKind::NotFound, "missing")),
        || {
            fallback_called.set(true);
            Ok(())
        },
    );

    assert!(result.is_ok(), "expected Ok after fallback, got {result:?}");
    assert!(fallback_called.get(), "fallback should run");
}

#[test]
fn falls_back_when_gio_exits_nonzero() {
    use std::cell::Cell;

    let fallback_called = Cell::new(false);
    let result = open_unix_preferring_gio_with(
        "gio",
        || Err(io::Error::other("process exited with exit status: 1")),
        || {
            fallback_called.set(true);
            Ok(())
        },
    );

    assert!(result.is_ok(), "expected Ok after fallback, got {result:?}");
    assert!(fallback_called.get(), "fallback should run");
}

#[test]
fn gio_open_detaches_when_deadline_expires() {
    let root = temp_path("open-gio-timeout-detach");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let capture = root.join("capture.txt");
    let capture_str = capture.to_str().unwrap();
    let command = format!("sleep 1; printf 'gio' > {}", shell_quote(capture_str));
    let mut shell = Command::new("/bin/sh");
    shell.arg("-c").arg(command);

    let started = std::time::Instant::now();
    let result = spawn_with_deadline(
        shell,
        std::time::Duration::from_millis(50),
        std::time::Duration::from_millis(5),
    );
    let elapsed = started.elapsed();

    assert!(
        result.is_ok(),
        "expected Ok via detach path, got {result:?}"
    );
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "should return before the opener finishes its 1s sleep, took {elapsed:?}"
    );

    let mut recorded = String::new();
    for _ in 0..300 {
        match fs::read_to_string(&capture) {
            Ok(contents) if !contents.is_empty() => {
                recorded = contents;
                break;
            }
            _ => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(recorded.trim(), "gio");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
