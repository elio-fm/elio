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
        if app.paste_progress().is_none() && app.file_operations.queued_pastes.is_empty() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for paste to complete");
}

pub(super) fn wait_for_paste_and_reload(app: &mut App) {
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.paste_progress().is_none()
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

#[cfg(unix)]
pub(super) fn install_fake_clipboard_tool(
    root: &std::path::Path,
    capture_path: &std::path::Path,
) -> PathBuf {
    let tool = root.join("fake-clipboard");
    fs::write(
        &tool,
        format!("#!/bin/sh\ncat > '{}'\n", capture_path.display()),
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
            "#!/bin/sh\ncat > '{capture}'\n(sleep 1) >/dev/null 2>&1 &\nexit 0\n",
            capture = capture_path.display()
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
