use super::*;
use crate::{background_jobs::folder_sizes::FolderSizeResult, file_browser::ViewMode};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "elio-folder-sizes-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("a-empty")).unwrap();
        fs::create_dir_all(root.join("z-large/nested")).unwrap();
        fs::write(root.join("z-large/nested/data"), [0; 23]).unwrap();
        fs::write(root.join("z-large/.hidden"), [0; 7]).unwrap();
        fs::write(root.join("file"), [0; 10]).unwrap();
        // Navigation canonicalizes paths; use the same identity for cached totals.
        Self(root.canonicalize().unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn wait_for_load(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.file_browser.directory_runtime.pending_load.is_some() {
        app.process_background_jobs();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

fn finish(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.file_browser.folder_sizes.pending > 0 {
        app.process_folder_sizes();
        assert!(Instant::now() < deadline, "folder worker did not finish");
        std::thread::yield_now();
    }
}

#[test]
fn folder_sizes_real_worker_revisit_revalidates_once_and_preserves_focus() {
    let fixture = Fixture::new();
    let mut app = App::new_at(fixture.0.clone()).unwrap();
    assert_eq!(app.file_browser.folder_sizes.pending, 0);
    for grid in [false, true] {
        app.file_browser.view_mode = ViewMode::from_start_in_grid(grid);
        app.file_browser.sort_mode = SortMode::Size;
        let focus = fixture.0.join("a-empty");
        app.file_browser.selected = app
            .file_browser
            .entries
            .iter()
            .position(|e| e.path == focus)
            .unwrap();
        app.file_browser.selected_paths.insert(focus.clone());
        app.start_folder_sizes();
        finish(&mut app);
        assert_eq!(app.file_browser.entries[0].name, "z-large");
        assert_eq!(app.selected_entry().unwrap().path, focus);
        assert!(app.file_browser.selected_paths.contains(&focus));
        assert_eq!(
            app.file_browser.folder_sizes.current[&fixture.0.join("z-large")],
            30
        );
        let token = app.file_browser.folder_sizes.token;
        assert!(!app.process_folder_sizes());
        assert_eq!(app.file_browser.folder_sizes.token, token);
    }
    app.set_dir(fixture.0.join("z-large")).unwrap();
    wait_for_load(&mut app);
    finish(&mut app);
    fs::write(fixture.0.join("a-empty/new"), [0; 60]).unwrap();
    app.go_back().unwrap();
    wait_for_load(&mut app);
    assert_eq!(
        app.file_browser.folder_sizes.current[&fixture.0.join("a-empty")],
        0
    );
    assert_eq!(app.file_browser.folder_sizes.pending, 2);
    finish(&mut app);
    assert_eq!(app.file_browser.entries[0].name, "a-empty");
    assert_eq!(
        app.file_browser.folder_sizes.current[&fixture.0.join("a-empty")],
        60
    );
}

#[cfg(unix)]
#[test]
fn folder_sizes_does_not_scan_directory_symlinks() {
    let fixture = Fixture::new();
    let link = fixture.0.join("linked");
    std::os::unix::fs::symlink(fixture.0.join("z-large"), &link).unwrap();
    let mut app = App::new_at(fixture.0.clone()).unwrap();
    app.file_browser.sort_mode = SortMode::Size;
    app.start_folder_sizes();
    assert_eq!(app.file_browser.folder_sizes.pending, 2);
    finish(&mut app);
    assert!(!app.file_browser.folder_sizes.current.contains_key(&link));
    assert_eq!(app.file_browser.folder_sizes.current.len(), 2);
}

#[test]
fn folder_sizes_missing_folder_is_unknown_not_zero() {
    let fixture = Fixture::new();
    let mut app = App::new_at(fixture.0.clone()).unwrap();
    let missing = fixture.0.join("z-large");
    fs::remove_dir_all(&missing).unwrap();
    app.file_browser.sort_mode = SortMode::Size;
    app.start_folder_sizes();
    finish(&mut app);
    assert!(!app.file_browser.folder_sizes.current.contains_key(&missing));
    assert_eq!(
        app.file_browser.folder_sizes.current[&fixture.0.join("a-empty")],
        0
    );
    assert_eq!(app.file_browser.entries[0].name, "a-empty");
}

#[test]
fn folder_sizes_batches_reject_stale_and_incomplete_totals() {
    let fixture = Fixture::new();
    let mut app = App::new_at(fixture.0.clone()).unwrap();
    app.file_browser.sort_mode = SortMode::Size;
    // No real scan needed for deterministic completion ordering.
    app.cancel_folder_sizes();
    let token = app.file_browser.folder_sizes.token;
    let path = fixture.0.join("a-empty");
    app.file_browser.local_filter.query = "a-".into();
    app.file_browser.apply_local_filter_preserving_selection();
    let hidden_mark = fixture.0.join("z-large");
    app.file_browser.selected_paths.insert(hidden_mark.clone());
    app.file_browser
        .folder_sizes
        .begin(token, &[path.clone(), fixture.0.join("z-large")]);
    assert!(app.apply_folder_size_results(vec![
        FolderSizeResult {
            token,
            path: path.clone(),
            size: Some(0)
        },
        FolderSizeResult {
            token,
            path: fixture.0.join("z-large"),
            size: None
        },
    ]));
    assert_eq!(app.selected_entry().unwrap().path, path);
    assert_eq!(app.file_browser.entries.len(), 1);
    assert!(app.file_browser.selected_paths.contains(&hidden_mark));
    assert_eq!(app.file_browser.folder_sizes.pending, 0);
    assert_eq!(app.file_browser.folder_sizes.current.get(&path), Some(&0));
    assert!(
        !app.file_browser
            .folder_sizes
            .current
            .contains_key(&fixture.0.join("z-large"))
    );
    app.file_browser
        .folder_sizes
        .begin(token, std::slice::from_ref(&path));
    assert!(app.apply_folder_size_results(vec![FolderSizeResult {
        token,
        path: path.clone(),
        size: None
    }]));
    assert!(!app.file_browser.folder_sizes.current.contains_key(&path));
    app.start_folder_sizes();
    assert!(!app.apply_folder_size_results(vec![FolderSizeResult {
        token,
        path: path.clone(),
        size: Some(999)
    }]));
    let token = app.file_browser.folder_sizes.token;
    app.set_dir(fixture.0.join("a-empty")).unwrap();
    assert_eq!(app.file_browser.folder_sizes.pending, 0);
    assert!(!app.apply_folder_size_results(vec![FolderSizeResult {
        token,
        path: path.clone(),
        size: Some(999)
    }]));
    app.file_browser.sort_mode = SortMode::Size;
    app.cycle_sort_mode().unwrap();
    assert_eq!(app.file_browser.folder_sizes.pending, 0);
    assert!(!app.apply_folder_size_results(vec![FolderSizeResult {
        token,
        path,
        size: Some(999)
    }]));
}
