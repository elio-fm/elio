pub(super) use crate::app::App;
#[cfg(unix)]
pub(super) use crate::duplicate_finder::DuplicateFinderSession;
pub(super) use crate::file_browser::DirectoryLoadCompletion;
pub(super) use crate::file_operations::rename;
pub(super) use crate::file_operations::{BulkRenameItem, BulkRenameOverlay};
#[cfg(unix)]
pub(super) use crate::{app::PendingTerminalTask, file_operations::BulkRenameEditorSession};
#[cfg(unix)]
pub(super) use std::{
    env,
    ffi::OsString,
    sync::{Mutex, OnceLock},
};
pub(super) use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
pub(super) fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(unix)]
pub(super) struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

#[cfg(unix)]
impl EnvVarGuard {
    pub(super) fn set_path(key: &'static str, value: &Path) -> Self {
        let original = env::var_os(key);
        unsafe {
            env::set_var(key, value);
        }
        Self { key, original }
    }
}

#[cfg(unix)]
impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.original.as_ref() {
            Some(value) => unsafe {
                env::set_var(self.key, value);
            },
            None => unsafe {
                env::remove_var(self.key);
            },
        }
    }
}

/// Drive background jobs until both the trash worker and the subsequent
/// directory reload have both completed.  Checking only `trash_progress`
/// is not enough: a single `process_background_jobs` call can consume
/// the `Trash(done=true)` result *and* the immediately-queued
/// `Directory` reload in the same batch (a tiny directory scan completes
/// before the loop's next `try_recv`).  Driving until `pending_load` is
/// also gone guarantees that `app.status_message()` holds the final
/// status in all cases.
pub(super) fn wait_for_trash_and_reload(app: &mut App) {
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.trash_progress().is_none()
            && app.file_browser.directory_runtime.pending_load.is_none()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for trash and directory reload to complete");
}

pub(super) fn wait_for_restore_and_reload(app: &mut App) {
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.restore_progress().is_none()
            && app.file_browser.directory_runtime.pending_load.is_none()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for restore and directory reload to complete");
}

pub(super) fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-create-{label}-{unique}"))
}

pub(super) fn take_pending_status(app: &mut App) -> (String, Option<PathBuf>) {
    let load = app
        .file_browser
        .directory_runtime
        .pending_load
        .take()
        .expect("expected queued directory load");
    let status = match load.completion {
        DirectoryLoadCompletion::Status(status) => status,
        DirectoryLoadCompletion::Keep => {
            panic!("expected status completion, got keep")
        }
        DirectoryLoadCompletion::Clear => {
            panic!("expected status completion, got clear")
        }
    };
    (status, load.reselect_path)
}

pub(super) fn encode_trashinfo_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('%', "%25")
        .replace(' ', "%20")
}

pub(super) fn create_fake_trash_file(label: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let root = temp_path(label);
    let originals_dir = root.join("originals");
    let trash_files = root.join("Trash/files");
    let trash_info = root.join("Trash/info");
    fs::create_dir_all(&originals_dir).expect("failed to create originals dir");
    fs::create_dir_all(&trash_files).expect("failed to create trash files dir");
    fs::create_dir_all(&trash_info).expect("failed to create trash info dir");

    let original_path = originals_dir.join("restore-target.txt");
    fs::write(&original_path, "restore me").expect("failed to write original file");

    let trashed_path = trash_files.join("restore-target.txt");
    fs::rename(&original_path, &trashed_path).expect("failed to move file into fake trash");
    fs::write(
        trash_info.join("restore-target.txt.trashinfo"),
        format!(
            "[Trash Info]\nPath={}\nDeletionDate=2026-03-21T00:00:00\n",
            encode_trashinfo_path(&original_path)
        ),
    )
    .expect("failed to write trashinfo");

    (root, trash_files, original_path, trashed_path)
}
