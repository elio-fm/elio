use crate::app::App;
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-actions-{label}-{unique}"))
}

pub(super) fn make_auto_reload_ready(app: &mut App) {
    app.file_browser.directory_runtime.last_auto_reload_at =
        Instant::now() - Duration::from_secs(3);
}

pub(super) fn wait_for_directory_load(app: &mut App) {
    for _ in 0..300 {
        let _ = app.process_background_jobs();
        if app.file_browser.directory_runtime.pending_load.is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for directory load");
}

pub(super) fn wait_for_directory_reload(app: &mut App, expected_entries: usize) {
    for _ in 0..500 {
        let _ = app.process_auto_reload();
        let _ = app.process_background_jobs();
        if app.file_browser.entries.len() == expected_entries
            && app
                .file_browser
                .directory_runtime
                .pending_reload_at
                .is_none()
            && app
                .file_browser
                .directory_runtime
                .pending_fingerprint_scan
                .is_none()
            && app.file_browser.directory_runtime.pending_load.is_none()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "timed out waiting for directory reload: entries={}, pending_reload={}, pending_fingerprint_scan={}, pending_load={}, pending_background_work={}",
        app.file_browser.entries.len(),
        app.file_browser
            .directory_runtime
            .pending_reload_at
            .is_some(),
        app.file_browser
            .directory_runtime
            .pending_fingerprint_scan
            .is_some(),
        app.file_browser.directory_runtime.pending_load.is_some(),
        app.has_pending_background_work(),
    );
}
