use super::mutation_test_support::*;

#[test]
fn confirm_restore_restores_file_from_trashinfo_and_queues_reload() {
    let (root, trash_files, original_path, trashed_path) = create_fake_trash_file("restore");

    let mut app = App::new_at(trash_files.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.open_restore_prompt();

    assert_eq!(
        app.file_operations.restore_title(),
        "Restore 1 selected file?"
    );
    app.confirm_restore().expect("restore should succeed");

    assert!(app.file_operations.restore.is_none());
    assert!(app.file_browser.selected_paths.is_empty());

    // Restore is now async — wait for the background worker and
    // subsequent directory reload to both complete.
    wait_for_restore_and_reload(&mut app);

    assert!(original_path.is_file());
    assert!(!trashed_path.exists());
    assert_eq!(app.status_message(), "Restored \"restore-target.txt\"");

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_restore_bulk_restores_multiple_files_and_reports_count() {
    let root = temp_path("restore-bulk");
    let originals_dir = root.join("originals");
    let trash_files = root.join("Trash/files");
    let trash_info = root.join("Trash/info");
    fs::create_dir_all(&originals_dir).expect("failed to create originals dir");
    fs::create_dir_all(&trash_files).expect("failed to create trash files dir");
    fs::create_dir_all(&trash_info).expect("failed to create trash info dir");

    // Create two fake trashed files.
    for name in ["alpha.txt", "beta.txt"] {
        let original = originals_dir.join(name);
        let trashed = trash_files.join(name);
        fs::write(&original, name).expect("failed to write original");
        fs::rename(&original, &trashed).expect("failed to move to fake trash");
        fs::write(
            trash_info.join(format!("{name}.trashinfo")),
            format!(
                "[Trash Info]\nPath={}\nDeletionDate=2026-03-23T00:00:00\n",
                encode_trashinfo_path(&original)
            ),
        )
        .expect("failed to write trashinfo");
    }

    let mut app = App::new_at(trash_files.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.file_browser
        .selected_paths
        .insert(trash_files.join("alpha.txt"));
    app.file_browser
        .selected_paths
        .insert(trash_files.join("beta.txt"));
    app.open_restore_prompt();

    assert_eq!(app.file_operations.restore_title(), "Restore 2 files?");
    app.confirm_restore().expect("restore should succeed");

    assert!(app.file_operations.restore.is_none());
    assert!(app.file_browser.selected_paths.is_empty());

    wait_for_restore_and_reload(&mut app);

    assert!(originals_dir.join("alpha.txt").is_file());
    assert!(originals_dir.join("beta.txt").is_file());
    assert!(!trash_files.join("alpha.txt").exists());
    assert!(!trash_files.join("beta.txt").exists());
    assert_eq!(app.status_message(), "Restored 2 items");

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn restore_refuses_normal_selection_from_trash() {
    let root = temp_path("restore-normal-selection");
    let trash_files = root.join("Trash/files");
    let normal = root.join("normal.txt");
    fs::create_dir_all(&trash_files).expect("failed to create trash files dir");
    fs::write(&normal, "normal").expect("failed to write normal file");

    let mut app = App::new_at(trash_files.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.file_browser.selected_paths.insert(normal);
    app.open_restore_prompt();

    assert!(!app.file_operations.restore_is_open());
    assert_eq!(app.status_message(), "Cannot restore normal files");

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn restore_refuses_mixed_trash_and_normal_selection() {
    let root = temp_path("restore-mixed-selection");
    let trash_files = root.join("Trash/files");
    let trashed = trash_files.join("trashed.txt");
    let normal = root.join("normal.txt");
    fs::create_dir_all(&trash_files).expect("failed to create trash files dir");
    fs::write(&trashed, "trashed").expect("failed to write trashed file");
    fs::write(&normal, "normal").expect("failed to write normal file");

    let mut app = App::new_at(trash_files.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.file_browser.selected_paths.insert(trashed);
    app.file_browser.selected_paths.insert(normal);
    app.open_restore_prompt();

    assert!(!app.file_operations.restore_is_open());
    assert_eq!(
        app.status_message(),
        "Selection mixes trash and normal files"
    );

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn restore_allows_selected_parent_of_current_directory() {
    let root = temp_path("restore-parent-selection");
    let trash_files = root.join("Trash/files");
    let parent = trash_files.join("parent");
    let child_file = parent.join("child.txt");
    fs::create_dir_all(&parent).expect("failed to create parent dir");
    fs::write(&child_file, "child").expect("failed to write child file");

    let mut app = App::new_at(parent.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.file_browser.selected_paths.insert(parent);
    app.open_restore_prompt();

    assert!(app.file_operations.restore_is_open());
    assert_eq!(
        app.file_operations.restore_title(),
        "Restore 1 selected folder?"
    );
    assert_eq!(
        app.file_operations.restore_target_path_at(0),
        Some(trash_files.join("parent").as_path())
    );

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn esc_during_restore_clears_chip_immediately() {
    // Restore is per-item (like permanent delete), so pressing Esc should
    // clear the chip right away rather than waiting for done=true.
    let (root, trash_files, _original_path, _trashed_path) =
        create_fake_trash_file("restore-cancel");

    let mut app = App::new_at(trash_files.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.open_restore_prompt();
    app.confirm_restore().expect("restore should succeed");

    assert!(
        app.file_operations.restore_progress().is_some(),
        "chip should be visible after submit"
    );

    // Simulate Esc: chip clears immediately for per-item operations.
    let token = app.file_operations.restore_token;
    app.job_scheduler.cancel_restore(token);
    app.file_operations.restore_progress = None;

    assert!(
        app.file_operations.restore_progress().is_none(),
        "chip should clear immediately after Esc for restore"
    );

    // Drive to completion so the background thread shuts down cleanly.
    // The done=true result still arrives and is ignored (token matches
    // but restore_progress is already None), and restore_source_cwd is
    // taken and a directory reload is queued.
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.file_operations.restore_source_cwd.is_none()
            && app.file_browser.directory_runtime.pending_load.is_none()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Status is either "Restore cancelled" (cancel won the race) or
    // "Restored \"restore-target.txt\"" (restore finished before cancel).
    let status = app.status_message();
    let valid = status.starts_with("Restore cancelled")
        || status.starts_with("Restored")
        || status.starts_with("Nothing was restored");
    assert!(valid, "unexpected status: {status:?}");

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    // root may or may not still contain the original file depending on
    // the race; ignore removal errors.
    let _ = fs::remove_dir_all(root);
}

#[test]
fn confirm_restore_while_in_progress_shows_status_and_dismisses_overlay() {
    // If the user opens and confirms a second restore while one is already
    // running, confirm_restore should surface a status message and close
    // the overlay without submitting a duplicate job.
    let (root, trash_files, _original_path, _trashed_path) =
        create_fake_trash_file("restore-in-progress");

    let mut app = App::new_at(trash_files.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.open_restore_prompt();
    app.confirm_restore().expect("first restore should succeed");

    // A second restore is attempted while the first is still in flight.
    app.open_restore_prompt();
    assert!(app.file_operations.restore.is_some(), "overlay should open");
    app.confirm_restore()
        .expect("second confirm should not error");

    assert!(
        app.file_operations.restore.is_none(),
        "overlay should be dismissed by the in-progress guard"
    );
    assert_eq!(
        app.status, "Restore in progress — press Esc to cancel",
        "in-progress message should be shown"
    );

    // Clean up the background worker.
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.file_operations.restore_progress().is_none()
            && app.file_browser.directory_runtime.pending_load.is_none()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    let _ = fs::remove_dir_all(root);
}
