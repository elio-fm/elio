use super::helpers::{
    make_auto_reload_ready, temp_path, wait_for_directory_load, wait_for_directory_reload,
};
use crate::app::{App, ScreenRegions, ViewMetrics};
use crate::file_browser::{
    DirectoryHistoryMode, DirectoryLoadCompletion, HistoryEntry, PendingDirectoryLoad, ViewMode,
};
use std::{
    fs,
    time::{Duration, Instant},
};

#[test]
fn configured_folders_first_applies_to_browser_and_directory_previews() {
    use crate::filesystem::SortMode;

    const SETTING_ENV: &str = "ELIO_TEST_FOLDERS_FIRST";
    let Ok(setting) = std::env::var(SETTING_ENV) else {
        // Config is process-wide and immutable; isolate each setting from other tests.
        for setting in ["default", "true", "false"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    concat!(
                        module_path!(),
                        "::configured_folders_first_applies_to_browser_and_directory_previews"
                    )
                    .trim_start_matches("elio::"),
                ])
                .env(SETTING_ENV, setting)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "folders_first {setting}:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
        return;
    };

    let root = temp_path("folders-first");
    let cwd = root.join("browser");
    let folder = cwd.join("z-folder");
    fs::create_dir_all(&folder).unwrap();
    fs::write(cwd.join("a-file"), "content").unwrap();
    for name in ["entry2", "entry20"] {
        fs::create_dir(folder.join(name)).unwrap();
    }
    for name in ["entry1", "entry10"] {
        fs::write(folder.join(name), "content").unwrap();
    }
    // Make the file newer than the folder without relying on filesystem clock resolution.
    fs::File::options()
        .write(true)
        .open(cwd.join("a-file"))
        .unwrap()
        .set_modified(fs::metadata(&folder).unwrap().modified().unwrap() + Duration::from_secs(60))
        .unwrap();
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        if setting == "default" {
            String::new()
        } else {
            format!("[ui]\nfolders_first = {setting}\n")
        },
    )
    .unwrap();
    crate::config::initialize(Some(&config_path)).unwrap();
    let mut app = App::new_at(cwd).unwrap();
    let expected = if setting == "false" {
        ["a-file", "z-folder"]
    } else {
        ["z-folder", "a-file"]
    };
    for (mode, reload) in [
        (SortMode::Name, false),
        (SortMode::Name, true),
        (SortMode::Modified, true),
        (SortMode::Size, true),
    ] {
        app.file_browser.sort_mode = mode;
        if reload {
            app.reload().unwrap();
            wait_for_directory_load(&mut app);
        }
        assert_eq!(
            app.file_browser
                .entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            expected,
            "browser {mode:?}, reload={reload}"
        );
    }

    app.file_browser.selected = app
        .file_browser
        .entries
        .iter()
        .position(|entry| entry.path == folder)
        .unwrap();
    app.refresh_preview();
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.preview.state.content.lines.len() != 4 {
        assert!(Instant::now() < deadline, "directory preview timed out");
        app.process_background_jobs();
        std::thread::sleep(Duration::from_millis(10));
    }
    let expected = if setting == "false" {
        ["entry1", "entry10", "entry2", "entry20"]
    } else {
        ["entry2", "entry20", "entry1", "entry10"]
    };
    // A repeated refresh must also retain the configured order when using the cache.
    for refresh in [false, true] {
        if refresh {
            let cache_hits = app.preview.state.metrics.cache_hits;
            app.refresh_preview();
            assert!(app.preview.state.metrics.cache_hits > cache_hits);
        }
        let names = app
            .preview
            .state
            .content
            .lines
            .iter()
            .map(|line| line.spans.last().unwrap().content.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(names, expected, "preview refresh={refresh}");
    }
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn configured_default_sort_initializes_browser_and_retains_runtime_sort() {
    use crate::filesystem::SortMode;

    const SETTING_ENV: &str = "ELIO_TEST_DEFAULT_SORT";
    let Ok(setting) = std::env::var(SETTING_ENV) else {
        for setting in ["default", "name", "modified", "size"] {
            for grid in [false, true] {
                for folders_first in [false, true] {
                    let output = std::process::Command::new(std::env::current_exe().unwrap())
                        .args([
                            "--exact",
                            concat!(
                                module_path!(),
                                "::configured_default_sort_initializes_browser_and_retains_runtime_sort"
                            )
                            .trim_start_matches("elio::"),
                        ])
                        .env(SETTING_ENV, setting)
                        .env("ELIO_TEST_SORT_GRID", grid.to_string())
                        .env("ELIO_TEST_SORT_FOLDERS_FIRST", folders_first.to_string())
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "default_sort={setting}, grid={grid}, folders_first={folders_first}:\n{}\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
                }
            }
        }
        return;
    };

    let grid = std::env::var("ELIO_TEST_SORT_GRID").unwrap() == "true";
    let folders_first = std::env::var("ELIO_TEST_SORT_FOLDERS_FIRST").unwrap() == "true";
    let root = temp_path("sort-by");
    let cwd = root.join("browser");
    let folder = cwd.join("z-folder");
    fs::create_dir_all(&folder).unwrap();
    let epoch = std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    for directory in [&cwd, &folder] {
        for (name, content, seconds) in [("a", "aa", 0), ("b", "b", 120), ("c", "ccc", 60)] {
            let path = directory.join(name);
            fs::write(&path, content).unwrap();
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(epoch + Duration::from_secs(seconds))
                .unwrap();
        }
    }
    let config_path = root.join("config.toml");
    let sort_setting = if setting == "default" {
        String::new()
    } else {
        format!("default_sort = {setting:?}\n")
    };
    fs::write(
        &config_path,
        format!("[ui]\n{sort_setting}start_in_grid = {grid}\nfolders_first = {folders_first}\n"),
    )
    .unwrap();
    crate::config::initialize(Some(&config_path)).unwrap();
    let mut app = App::new_at(cwd.clone()).unwrap();
    if setting == "size" {
        assert_eq!(app.file_browser.folder_sizes.pending, 1);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while app.file_browser.folder_sizes.pending > 0 {
            app.process_folder_sizes();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(app.file_browser.folder_sizes.current[&folder], 6);
        assert_eq!(app.file_browser.entries[0].name, "z-folder");
    } else {
        assert_eq!(app.file_browser.folder_sizes.pending, 0);
    }
    let initial = match setting.as_str() {
        "modified" => SortMode::Modified,
        "size" => SortMode::Size,
        _ => SortMode::Name,
    };
    assert_eq!(
        app.file_browser.view_mode,
        ViewMode::from_start_in_grid(grid)
    );
    let mut mode = initial;
    for cycle in 0..4 {
        if cycle > 0 {
            app.cycle_sort_mode().unwrap();
            wait_for_directory_load(&mut app);
            mode = mode.cycle();
        }
        for step in 0..5 {
            match step {
                1 => app.reload().unwrap(),
                2 => app.set_dir(folder.clone()).unwrap(),
                3 => app.go_back().unwrap(),
                4 => app.toggle_view_mode(),
                _ => {}
            }
            wait_for_directory_load(&mut app);
            assert_eq!(app.file_browser.sort_mode, mode);
            let names = app
                .file_browser
                .entries
                .iter()
                .filter(|entry| entry.name != "z-folder")
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>();
            let expected = match mode {
                SortMode::Name => ["a", "b", "c"],
                SortMode::Modified => ["b", "c", "a"],
                SortMode::Size => ["c", "a", "b"],
            };
            assert_eq!(names, expected, "cycle={cycle}, step={step}");
            if app.file_browser.cwd == cwd {
                if folders_first || mode == SortMode::Modified {
                    assert_eq!(app.file_browser.entries[0].name, "z-folder");
                } else if mode == SortMode::Name {
                    assert_eq!(app.file_browser.entries.last().unwrap().name, "z-folder");
                }
            }
        }
    }
    assert_eq!(mode, initial);
    drop(app);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn watcher_reload_detects_new_visible_entries() {
    let root = temp_path("auto-reload-visible");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.directory_runtime.watch = None;
    assert_eq!(app.file_browser.entries.len(), 1);

    let second = root.join("two.txt");
    fs::write(&second, "world").expect("failed to write second file");
    app.file_browser
        .directory_runtime
        .watch_tx
        .send(crate::filesystem::DirectoryWatchEvent::Changed(vec![
            second,
        ]))
        .expect("failed to queue watch event");

    assert!(
        !app.process_auto_reload()
            .expect("watch processing should succeed"),
        "watch processing should debounce before reloading",
    );
    app.file_browser.directory_runtime.pending_reload_at =
        Some(Instant::now() - Duration::from_millis(1));

    assert!(
        !app.process_auto_reload()
            .expect("auto reload should succeed"),
        "watch-driven reload should schedule an async fingerprint scan first",
    );
    wait_for_directory_reload(&mut app, 2);
    assert_eq!(app.file_browser.entries.len(), 2);
    assert!(
        app.file_browser
            .entries
            .iter()
            .any(|entry| entry.name == "two.txt")
    );

    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn watcher_rescan_event_triggers_reload() {
    let root = temp_path("auto-reload-rescan");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.directory_runtime.watch = None;
    assert_eq!(app.file_browser.entries.len(), 1);

    fs::write(root.join("two.txt"), "world").expect("failed to write second file");
    app.file_browser
        .directory_runtime
        .watch_tx
        .send(crate::filesystem::DirectoryWatchEvent::Rescan)
        .expect("failed to queue rescan event");

    assert!(
        !app.process_auto_reload()
            .expect("watch processing should succeed"),
        "watch processing should debounce before reloading",
    );
    app.file_browser.directory_runtime.pending_reload_at =
        Some(Instant::now() - Duration::from_millis(1));

    assert!(
        !app.process_auto_reload()
            .expect("auto reload should succeed"),
        "rescan-driven reload should schedule an async fingerprint scan first",
    );
    wait_for_directory_reload(&mut app, 2);
    assert_eq!(app.file_browser.entries.len(), 2);
    assert!(
        app.file_browser
            .entries
            .iter()
            .any(|entry| entry.name == "two.txt")
    );

    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn watcher_reload_ignores_hidden_entries_when_hidden_files_are_off() {
    let root = temp_path("auto-reload-hidden");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("visible.txt"), "hello").expect("failed to write visible file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.directory_runtime.watch = None;
    assert!(!app.file_browser.show_hidden);
    assert_eq!(app.file_browser.entries.len(), 1);

    let hidden = root.join(".secret");
    fs::write(&hidden, "hidden").expect("failed to write hidden file");
    app.file_browser
        .directory_runtime
        .watch_tx
        .send(crate::filesystem::DirectoryWatchEvent::Changed(vec![
            hidden,
        ]))
        .expect("failed to queue watch event");

    assert!(
        !app.process_auto_reload()
            .expect("watch processing should succeed"),
        "hidden-only changes should not trigger a reload schedule",
    );
    assert!(
        app.file_browser
            .directory_runtime
            .pending_reload_at
            .is_none()
    );
    assert_eq!(app.file_browser.entries.len(), 1);
    assert_eq!(app.file_browser.entries[0].name, "visible.txt");

    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sidebar_refresh_rebuilds_places_once_per_interval() {
    let root = temp_path("sidebar-refresh");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.places.rows.clear();
    app.places.last_refresh_at = Instant::now() - Duration::from_secs(3);

    assert!(
        app.process_sidebar_refresh(),
        "stale refresh windows should rebuild places"
    );
    assert!(
        !app.places.rows.is_empty(),
        "refresh should restore the builtin places list"
    );

    let sidebar_after_refresh = app.places.rows.clone();
    assert!(
        !app.process_sidebar_refresh(),
        "freshly refreshed sidebars should not rebuild again immediately"
    );
    assert_eq!(app.places.rows, sidebar_after_refresh);

    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn polling_fallback_respects_its_throttle_window() {
    let root = temp_path("auto-reload-throttle");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.directory_runtime.watch = None;
    app.file_browser.directory_runtime.use_polling_reload = true;
    fs::write(root.join("two.txt"), "world").expect("failed to write second file");

    assert!(
        !app.process_auto_reload()
            .expect("auto reload should succeed"),
        "reload should stay idle inside the throttle window",
    );
    assert_eq!(app.file_browser.entries.len(), 1);

    make_auto_reload_ready(&mut app);
    assert!(
        !app.process_auto_reload()
            .expect("auto reload should succeed"),
        "reload should schedule an async fingerprint scan once the throttle window has elapsed",
    );
    wait_for_directory_reload(&mut app, 2);
    assert_eq!(app.file_browser.entries.len(), 2);

    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn set_dir_failure_keeps_previous_directory_state() {
    let root = temp_path("set-dir-missing");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("note.txt"), "hello").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let missing = root.join("missing");

    assert!(app.set_dir(missing).is_err());
    assert_eq!(app.file_browser.cwd, root);
    assert_eq!(app.file_browser.entries.len(), 1);
    assert!(app.file_browser.directory_history.back.is_empty());
    assert!(app.file_browser.directory_history.forward.is_empty());

    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn go_back_failure_preserves_history() {
    let root = temp_path("history-missing");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let missing = root.join("missing");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.directory_history.back.push(HistoryEntry {
        cwd: missing.clone(),
        selected_path: None,
    });

    assert!(app.go_back().is_err());
    assert_eq!(app.file_browser.cwd, root);
    assert_eq!(
        app.file_browser.directory_history.back,
        vec![HistoryEntry {
            cwd: missing,
            selected_path: None,
        }]
    );
    assert!(app.file_browser.directory_history.forward.is_empty());

    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn reload_restores_latest_remembered_view_state() {
    let root = temp_path("reload-latest-view-state");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..8 {
        fs::write(root.join(format!("file-{index}.txt")), format!("{index}"))
            .expect("failed to write file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.view_mode = ViewMode::List;
    app.set_screen_regions(ScreenRegions {
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 3,
        },
        ..ScreenRegions::default()
    });

    app.reload().expect("reload should queue successfully");
    app.select_index(6);
    wait_for_directory_load(&mut app);

    assert_eq!(app.file_browser.selected, 6);
    assert_eq!(app.file_browser.scroll_row, 4);

    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn same_directory_reselect_updates_pending_load_instead_of_dropping_it() {
    let root = temp_path("same-dir-reselect-pending");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let beta = root.join("beta.txt");
    fs::write(&beta, "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.directory_runtime.pending_load = Some(PendingDirectoryLoad {
        token: 99,
        target_cwd: app.file_browser.cwd.clone(),
        previous_cwd: app.file_browser.cwd.clone(),
        previous_selected_path: app.selected_entry().map(|entry| entry.path.clone()),
        previous_selection_name: None,
        reselect_path: None,
        history_mode: DirectoryHistoryMode::None,
        refresh_search: false,
        completion: DirectoryLoadCompletion::Keep,
    });

    app.set_dir_transition(
        root.clone(),
        DirectoryHistoryMode::PushCurrent,
        Some(beta.clone()),
        DirectoryLoadCompletion::Status("Located beta.txt".to_string()),
    )
    .expect("same-directory reselect should update the pending load");

    let load = app
        .file_browser
        .directory_runtime
        .pending_load
        .as_ref()
        .expect("pending load should remain queued");
    assert_eq!(load.reselect_path.as_deref(), Some(beta.as_path()));
    match &load.completion {
        DirectoryLoadCompletion::Status(status) => assert_eq!(status, "Located beta.txt"),
        other => panic!("expected status completion, got {other:?}"),
    }

    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
