use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-actions-{label}-{unique}"))
}

fn make_auto_reload_ready(app: &mut App) {
    app.directory_runtime.last_auto_reload_at =
        Instant::now() - AUTO_RELOAD_INTERVAL - Duration::from_millis(1);
}

fn wait_for_directory_load(app: &mut App) {
    for _ in 0..100 {
        let _ = app.process_background_jobs();
        if app.directory_runtime.pending_load.is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for directory load");
}

#[test]
fn watcher_reload_detects_new_visible_entries() {
    let root = temp_path("auto-reload-visible");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.directory_runtime.watch = None;
    assert_eq!(app.entries.len(), 1);

    let second = root.join("two.txt");
    fs::write(&second, "world").expect("failed to write second file");
    app.directory_runtime
        .watch_tx
        .send(crate::fs::DirectoryWatchEvent::Changed(vec![second]))
        .expect("failed to queue watch event");

    assert!(
        !app.process_auto_reload()
            .expect("watch processing should succeed"),
        "watch processing should debounce before reloading",
    );
    app.directory_runtime.pending_reload_at = Some(Instant::now() - Duration::from_millis(1));

    assert!(
        app.process_auto_reload()
            .expect("auto reload should succeed"),
        "watch-driven reload should report a change",
    );
    wait_for_directory_load(&mut app);
    assert_eq!(app.entries.len(), 2);
    assert!(app.entries.iter().any(|entry| entry.name == "two.txt"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn watcher_rescan_event_triggers_reload() {
    let root = temp_path("auto-reload-rescan");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.directory_runtime.watch = None;
    assert_eq!(app.entries.len(), 1);

    fs::write(root.join("two.txt"), "world").expect("failed to write second file");
    app.directory_runtime
        .watch_tx
        .send(crate::fs::DirectoryWatchEvent::Rescan)
        .expect("failed to queue rescan event");

    assert!(
        !app.process_auto_reload()
            .expect("watch processing should succeed"),
        "watch processing should debounce before reloading",
    );
    app.directory_runtime.pending_reload_at = Some(Instant::now() - Duration::from_millis(1));

    assert!(
        app.process_auto_reload()
            .expect("auto reload should succeed"),
        "rescan-driven reload should report a change",
    );
    wait_for_directory_load(&mut app);
    assert_eq!(app.entries.len(), 2);
    assert!(app.entries.iter().any(|entry| entry.name == "two.txt"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn watcher_reload_ignores_hidden_entries_when_hidden_files_are_off() {
    let root = temp_path("auto-reload-hidden");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("visible.txt"), "hello").expect("failed to write visible file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.directory_runtime.watch = None;
    assert!(!app.show_hidden);
    assert_eq!(app.entries.len(), 1);

    let hidden = root.join(".secret");
    fs::write(&hidden, "hidden").expect("failed to write hidden file");
    app.directory_runtime
        .watch_tx
        .send(crate::fs::DirectoryWatchEvent::Changed(vec![hidden]))
        .expect("failed to queue watch event");

    assert!(
        !app.process_auto_reload()
            .expect("watch processing should succeed"),
        "hidden-only changes should not trigger a reload schedule",
    );
    assert!(app.directory_runtime.pending_reload_at.is_none());
    assert_eq!(app.entries.len(), 1);
    assert_eq!(app.entries[0].name, "visible.txt");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn polling_fallback_respects_its_throttle_window() {
    let root = temp_path("auto-reload-throttle");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.directory_runtime.watch = None;
    app.directory_runtime.use_polling_reload = true;
    fs::write(root.join("two.txt"), "world").expect("failed to write second file");

    assert!(
        !app.process_auto_reload()
            .expect("auto reload should succeed"),
        "reload should stay idle inside the throttle window",
    );
    assert_eq!(app.entries.len(), 1);

    make_auto_reload_ready(&mut app);
    assert!(
        app.process_auto_reload()
            .expect("auto reload should succeed"),
        "reload should run once the throttle window has elapsed",
    );
    wait_for_directory_load(&mut app);
    assert_eq!(app.entries.len(), 2);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn selection_summary_is_compact_for_files() {
    let root = temp_path("selection-summary-file");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("note.txt"), "hello").expect("failed to write file");

    let app = App::new_at(root.clone()).expect("failed to create app");
    assert_eq!(app.selection_summary(), "1/1  note.txt");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn selection_summary_marks_directories_with_trailing_slash() {
    let root = temp_path("selection-summary-dir");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create temp dirs");

    let app = App::new_at(root.clone()).expect("failed to create app");
    assert_eq!(app.selection_summary(), "1/1  child/");

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
    assert_eq!(app.cwd, root);
    assert_eq!(app.entries.len(), 1);
    assert!(app.navigation_history.back.is_empty());
    assert!(app.navigation_history.forward.is_empty());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn go_back_failure_preserves_history() {
    let root = temp_path("history-missing");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let missing = root.join("missing");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation_history.back.push(HistoryEntry {
        cwd: missing.clone(),
        selected_path: None,
    });

    assert!(app.go_back().is_err());
    assert_eq!(app.cwd, root);
    assert_eq!(
        app.navigation_history.back,
        vec![HistoryEntry {
            cwd: missing,
            selected_path: None,
        }]
    );
    assert!(app.navigation_history.forward.is_empty());

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
    app.view_mode = ViewMode::List;
    app.set_frame_state(FrameState {
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 3,
        },
        ..FrameState::default()
    });

    app.reload().expect("reload should queue successfully");
    app.select_index(6);
    wait_for_directory_load(&mut app);

    assert_eq!(app.selected, 6);
    assert_eq!(app.scroll_row, 4);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
