pub(super) use crate::app::App;
pub(super) use crate::file_operations::ClipOp;
pub(super) use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
pub(super) use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
pub(super) use std::{
    env,
    ffi::OsString,
    sync::{Mutex, OnceLock},
};

// ── helpers ──────────────────────────────────────────────────────────────────

pub(super) fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-clipboard-{label}-{unique}"))
}

/// Poll `process_background_jobs` until there is no active paste and no queued
/// follow-up paste left to start, or the timeout expires.
pub(super) fn wait_for_paste(app: &mut App) {
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.file_operations.paste_progress().is_none()
            && app.file_operations.queued_pastes.is_empty()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for paste to complete");
}

pub(super) fn wait_for_paste_and_reload(app: &mut App) {
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.file_operations.paste_progress().is_none()
            && app.file_operations.queued_pastes.is_empty()
            && app.file_browser.directory_runtime.pending_load.is_none()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for paste and directory reload to complete");
}

#[cfg(unix)]
pub(super) fn clipboard_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(unix)]
pub(super) struct ClipboardEnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

#[cfg(unix)]
impl ClipboardEnvGuard {
    pub(super) fn isolate() -> Self {
        const VARS: &[&str] = &[
            "ELIO_TEST_CLIPBOARD_TOOL",
            "ELIO_TEST_OSC52_CAPTURE",
            "ELIO_TEST_TMUX_SET_CLIPBOARD",
            "ELIO_CLIPBOARD_OSC52",
            "TMUX",
            "TERM",
            "TERM_PROGRAM",
            "KITTY_WINDOW_ID",
            "WARP_SESSION_ID",
            "ALACRITTY_SOCKET",
            "VTE_VERSION",
            "PATH",
        ];

        let saved = VARS
            .iter()
            .map(|name| (*name, env::var_os(name)))
            .collect::<Vec<_>>();
        for name in VARS {
            unsafe {
                env::remove_var(name);
            }
        }

        Self { saved }
    }
}

#[cfg(unix)]
impl Drop for ClipboardEnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.saved {
            if let Some(value) = value {
                unsafe {
                    env::set_var(name, value);
                }
            } else {
                unsafe {
                    env::remove_var(name);
                }
            }
        }
    }
}

// Resolve before ClipboardEnvGuard::isolate removes PATH. Keep the executable's
// name (rather than canonicalizing symlinks) for multicall tools such as BusyBox.
#[cfg(unix)]
fn fixture_tool(name: &str) -> PathBuf {
    env::split_paths(&env::var_os("PATH").expect("fixture tools require PATH"))
        .map(|directory| directory.join(name))
        .find(|candidate| {
            fs::metadata(candidate).is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        })
        .map(|path| std::path::absolute(path).expect("fixture tool path should be absolute"))
        .unwrap_or_else(|| panic!("clipboard fixture requires {name} on PATH"))
}

#[cfg(unix)]
fn shell_quote(path: &std::path::Path) -> String {
    format!(
        "'{}'",
        path.to_str()
            .expect("fixture paths should be UTF-8")
            .replace('\'', "'\\''")
    )
}

#[cfg(unix)]
pub(super) fn install_fake_clipboard_tool(
    root: &std::path::Path,
    capture_path: &std::path::Path,
) -> PathBuf {
    let tool = root.join("fake-clipboard");
    fs::write(
        &tool,
        format!(
            "#!/bin/sh\n{} > {}\n",
            shell_quote(&fixture_tool("cat")),
            shell_quote(capture_path)
        ),
    )
    .expect("failed to write fake clipboard tool");
    let mut permissions = fs::metadata(&tool)
        .expect("fake clipboard tool metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&tool, permissions).expect("failed to chmod fake clipboard tool");
    tool
}

#[cfg(unix)]
pub(super) fn install_backgrounding_clipboard_tool(
    root: &std::path::Path,
    capture_path: &std::path::Path,
) -> PathBuf {
    let tool = root.join("fake-clipboard-background");
    fs::write(
        &tool,
        format!(
            "#!/bin/sh\n{cat} > {capture}\n({sleep} 1) >/dev/null 2>&1 &\nexit 0\n",
            cat = shell_quote(&fixture_tool("cat")),
            sleep = shell_quote(&fixture_tool("sleep")),
            capture = shell_quote(capture_path)
        ),
    )
    .expect("failed to write backgrounding clipboard tool");
    let mut permissions = fs::metadata(&tool)
        .expect("backgrounding clipboard tool metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&tool, permissions).expect("failed to chmod backgrounding clipboard tool");
    tool
}

#[cfg(unix)]
#[test]
fn clipboard_fixtures_work_with_isolated_path_and_shell_metacharacters() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let _lock = clipboard_env_lock();
    let cat = fixture_tool("cat");
    let sleep = fixture_tool("sleep");
    let _restore_env = ClipboardEnvGuard::isolate();
    let root = temp_path("fixture's quoted space $directory");
    fs::create_dir_all(&root).expect("failed to create fixture root");
    std::os::unix::fs::symlink(cat, root.join("cat")).expect("failed to link cat");
    let sleep_capture = root.join("sleep's arguments.txt");
    let sleep_wrapper = root.join("sleep");
    fs::write(
        &sleep_wrapper,
        format!(
            "#!/bin/sh\nprintf '%s' \"$1\" > {}\nexec {} \"$@\"\n",
            shell_quote(&sleep_capture),
            shell_quote(&sleep)
        ),
    )
    .expect("failed to write sleep wrapper");
    fs::set_permissions(&sleep_wrapper, fs::Permissions::from_mode(0o755))
        .expect("failed to chmod sleep wrapper");
    unsafe {
        env::set_var("PATH", &root);
    }
    let capture = root.join("clipboard's captured text.txt");
    let tools = [
        install_fake_clipboard_tool(&root, &capture),
        install_backgrounding_clipboard_tool(&root, &capture),
    ];
    let _isolated_env = ClipboardEnvGuard::isolate();
    assert!(env::var_os("PATH").is_none());
    for tool in tools {
        let mut child = Command::new(tool)
            // Empty PATH prevents even a shell's default search path helping.
            .env("PATH", "")
            .stdin(Stdio::piped())
            .spawn()
            .expect("failed to start clipboard fixture");
        child
            .stdin
            .take()
            .expect("fixture stdin should be piped")
            .write_all(b"clipboard 'payload'\nsecond line\n")
            .expect("failed to write fixture input");
        assert!(child.wait().expect("fixture should exit").success());
        assert_eq!(
            fs::read(&capture).expect("fixture should capture stdin"),
            b"clipboard 'payload'\nsecond line\n"
        );
        fs::remove_file(&capture).expect("failed to reset capture");
    }
    for _ in 0..100 {
        if fs::read(&sleep_capture).is_ok_and(|contents| contents == b"1") {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read(&sleep_capture).expect("background fixture should invoke resolved sleep"),
        b"1"
    );
    fs::remove_dir_all(root).expect("failed to remove fixture root");
}
