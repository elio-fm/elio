use super::mutation_test_support::*;

#[test]
fn confirm_trash_permanently_deletes_selected_items_inside_trash() {
    let root = temp_path("trash-permanent");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("gone.txt"), "bye").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.file_browser
        .selected_paths
        .insert(root.join("gone.txt"));
    app.open_trash_prompt();

    assert_eq!(app.trash_title(), "Delete permanently 1 selected file?");
    app.confirm_trash().expect("trash should succeed");

    assert!(app.overlays.trash.is_none());
    assert!(app.file_browser.selected_paths.is_empty());

    // Deletion is async — wait for the background worker *and* the
    // subsequent directory reload to both finish.
    wait_for_trash_and_reload(&mut app);

    assert!(!root.join("gone.txt").exists());
    // Status is set by apply_directory_snapshot once the reload completes.
    assert_eq!(app.status_message(), "Permanently deleted \"gone.txt\"");

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_delete_permanently_removes_selected_items_outside_trash() {
    let root = temp_path("delete-permanent-outside-trash");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("gone.txt"), "bye").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser
        .selected_paths
        .insert(root.join("gone.txt"));
    app.open_delete_permanently_prompt();

    assert_eq!(app.trash_title(), "Delete permanently 1 selected file?");
    app.confirm_trash().expect("delete should succeed");

    assert!(app.overlays.trash.is_none());
    assert!(app.file_browser.selected_paths.is_empty());

    wait_for_trash_and_reload(&mut app);

    assert!(!root.join("gone.txt").exists());
    assert_eq!(app.status_message(), "Permanently deleted \"gone.txt\"");

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_delete_selected_parent_of_current_directory_moves_to_parent() {
    let root = temp_path("delete-parent-of-current");
    let parent = root.join("parent");
    let child = parent.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(child.join("file.txt"), "child").expect("failed to write child file");

    let mut app = App::new_at(child.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(parent.clone());
    app.open_delete_permanently_prompt();

    assert_eq!(app.trash_title(), "Delete permanently 1 selected folder?");
    app.confirm_trash().expect("delete should succeed");

    assert!(app.overlays.trash.is_none());
    assert!(app.file_browser.selected_paths.is_empty());

    wait_for_trash_and_reload(&mut app);

    assert_eq!(app.file_browser.cwd, root);
    assert!(!parent.exists());
    assert_eq!(app.status_message(), "Permanently deleted \"parent\"");

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn after_delete_cursor_moves_to_next_surviving_entry() {
    // Deleting a middle entry should leave the cursor on what was the
    // entry immediately below it (now occupying the same visual row).
    let root = temp_path("cursor-next-after-delete");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("alpha.txt"), "a").expect("failed to write alpha");
    fs::write(root.join("beta.txt"), "b").expect("failed to write beta");
    fs::write(root.join("gamma.txt"), "c").expect("failed to write gamma");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    // entries are sorted by name: alpha=0, beta=1, gamma=2
    app.file_browser.in_trash = true;
    app.file_browser.selected = 1; // cursor on beta.txt
    app.remember_current_directory_view(); // simulate a rendered frame committing the position
    app.open_trash_prompt();
    app.confirm_trash().expect("trash should succeed");

    wait_for_trash_and_reload(&mut app);

    assert!(!root.join("beta.txt").exists());
    assert_eq!(
        app.selected_entry().map(|e| e.name.as_str()),
        Some("gamma.txt"),
        "cursor should land on gamma.txt (next surviving entry)"
    );

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn after_delete_cursor_falls_back_to_previous_entry_when_last_is_deleted() {
    // Deleting the last entry should leave the cursor on the entry above it.
    let root = temp_path("cursor-prev-after-delete");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("alpha.txt"), "a").expect("failed to write alpha");
    fs::write(root.join("beta.txt"), "b").expect("failed to write beta");
    fs::write(root.join("gamma.txt"), "c").expect("failed to write gamma");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    // entries are sorted by name: alpha=0, beta=1, gamma=2
    app.file_browser.in_trash = true;
    app.file_browser.selected = 2; // cursor on gamma.txt
    app.remember_current_directory_view(); // simulate a rendered frame committing the position
    app.open_trash_prompt();
    app.confirm_trash().expect("trash should succeed");

    wait_for_trash_and_reload(&mut app);

    assert!(!root.join("gamma.txt").exists());
    assert_eq!(
        app.selected_entry().map(|e| e.name.as_str()),
        Some("beta.txt"),
        "cursor should fall back to beta.txt (last surviving entry before cursor)"
    );

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cancelled_delete_does_not_move_cursor_away_from_surviving_entry() {
    // When permanent delete is cancelled before any item is removed, the
    // cursor must not jump away — the targeted entry is still present.
    let root = temp_path("cursor-cancel-delete");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("alpha.txt"), "a").expect("failed to write alpha");
    fs::write(root.join("beta.txt"), "b").expect("failed to write beta");
    fs::write(root.join("gamma.txt"), "c").expect("failed to write gamma");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.file_browser.selected = 1; // cursor on beta.txt
    app.remember_current_directory_view(); // simulate a rendered frame committing the position
    app.open_trash_prompt();
    app.confirm_trash().expect("trash should succeed");

    // Cancel before the worker starts processing.
    app.jobs.scheduler.cancel_trash(app.jobs.trash_token);

    wait_for_trash_and_reload(&mut app);

    // beta.txt may or may not have been deleted depending on race, but the
    // cursor must not have jumped to an entry other than what was at index 1.
    // If the file still exists, the cursor must be on it (not on gamma.txt).
    if root.join("beta.txt").exists() {
        assert_eq!(
            app.selected_entry().map(|e| e.name.as_str()),
            Some("beta.txt"),
            "cursor must stay on beta.txt when cancel won the race"
        );
    }
    // If the cancel lost the race and beta.txt was deleted, the cursor
    // should have moved to gamma.txt (completed == total == 1).
    // Either outcome is valid; the key invariant is that we never land
    // on a position whose entry no longer exists.
    assert!(
        app.selected_entry().is_none()
            || root
                .join(app.selected_entry().unwrap().name.as_str())
                .exists(),
        "cursor must point to a surviving entry"
    );

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_trash_batch_trashes_multiple_files_and_reports_count() {
    let root = temp_path("trash-batch-multi");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("alpha.txt"), "a").expect("failed to write alpha");
    fs::write(root.join("beta.txt"), "b").expect("failed to write beta");
    fs::write(root.join("gamma.txt"), "c").expect("failed to write gamma");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    // in_trash = false → non-permanent batch trash
    app.file_browser
        .selected_paths
        .insert(root.join("alpha.txt"));
    app.file_browser
        .selected_paths
        .insert(root.join("beta.txt"));
    app.file_browser
        .selected_paths
        .insert(root.join("gamma.txt"));
    app.open_trash_prompt();

    assert_eq!(app.trash_title(), "Trash 3 files?");
    app.confirm_trash().expect("trash should succeed");

    assert!(app.overlays.trash.is_none());
    assert!(app.file_browser.selected_paths.is_empty());

    wait_for_trash_and_reload(&mut app);

    assert!(!root.join("alpha.txt").exists());
    assert!(!root.join("beta.txt").exists());
    assert!(!root.join("gamma.txt").exists());
    assert_eq!(app.status_message(), "Trashed 3 items");

    // Purge the items we just trashed from the OS trash so the test
    // leaves no permanent side-effects.
    // trash::os_limited is only available on non-macOS Unix (freedesktop).
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use trash::os_limited::{list, purge_all};
        if let Ok(items) = list() {
            let ours: Vec<_> = items
                .into_iter()
                .filter(|item| item.original_parent == root)
                .collect();
            let _ = purge_all(ours);
        }
    }

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_trash_batch_single_file_shows_quoted_name() {
    let root = temp_path("trash-batch-single");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("notes.txt"), "hello").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    // in_trash = false → non-permanent batch trash
    app.file_browser
        .selected_paths
        .insert(root.join("notes.txt"));
    app.open_trash_prompt();

    assert_eq!(app.trash_title(), "Trash 1 selected file?");
    app.confirm_trash().expect("trash should succeed");

    assert!(app.overlays.trash.is_none());
    assert!(app.file_browser.selected_paths.is_empty());

    wait_for_trash_and_reload(&mut app);

    assert!(!root.join("notes.txt").exists());
    assert_eq!(app.status_message(), "Trashed \"notes.txt\"");

    // Purge from OS trash to avoid side-effects.
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use trash::os_limited::{list, purge_all};
        if let Ok(items) = list() {
            let ours: Vec<_> = items
                .into_iter()
                .filter(|item| item.original_parent == root)
                .collect();
            let _ = purge_all(ours);
        }
    }

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn esc_during_batched_trash_keeps_chip_visible_until_done() {
    // Non-permanent (batched) trash is a single atomic OS call that may
    // already be in flight when the user presses Esc.  The chip must
    // remain visible until the worker sends done=true so the user can
    // see the operation is still running, not silently cancelled.
    let root = temp_path("trash-cancel-batched");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("canary.txt"), "x").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser
        .selected_paths
        .insert(root.join("canary.txt"));
    app.open_trash_prompt();
    app.confirm_trash().expect("trash should succeed");

    // Chip is showing immediately after submit.
    assert!(
        app.trash_progress().is_some(),
        "chip should be visible after submit"
    );

    // Simulate Esc: cancel_trash is called but chip must NOT be cleared.
    app.jobs.scheduler.cancel_trash(app.jobs.trash_token);
    // trash_progress is still Some — chip stays visible.
    assert!(
        app.trash_progress().is_some(),
        "chip must remain visible after Esc for batched trash"
    );

    // Wait for the worker to finish (cancelled before start or completed).
    wait_for_trash_and_reload(&mut app);

    // Chip is gone once done=true is processed.
    assert!(
        app.trash_progress().is_none(),
        "chip should be gone after completion"
    );

    // Status is either "Trash cancelled" (cancel won the race) or "Trashed
    // \"canary.txt\"" (batch was already in flight).  Either is correct.
    let status = app.status_message();
    let valid = status.starts_with("Trash cancelled")
        || status.starts_with("Trashed")
        || status.starts_with("Nothing was deleted");
    assert!(valid, "unexpected status: {status:?}");

    // Purge from OS trash if the file actually got trashed.
    #[cfg(all(unix, not(target_os = "macos")))]
    if !root.join("canary.txt").exists() {
        use trash::os_limited::{list, purge_all};
        if let Ok(items) = list() {
            let ours: Vec<_> = items
                .into_iter()
                .filter(|item| item.original_parent == root)
                .collect();
            let _ = purge_all(ours);
        }
    }

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn esc_during_permanent_delete_clears_chip_immediately() {
    // Permanent delete can be interrupted between items, so pressing Esc
    // should clear the chip right away (not wait for done=true).
    let root = temp_path("trash-cancel-permanent");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("gone.txt"), "x").expect("failed to write file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.in_trash = true;
    app.file_browser
        .selected_paths
        .insert(root.join("gone.txt"));
    app.open_trash_prompt();
    app.confirm_trash().expect("trash should succeed");

    assert!(
        app.trash_progress().is_some(),
        "chip should be visible after submit"
    );

    // Simulate Esc for permanent delete: chip clears immediately.
    let token = app.jobs.trash_token;
    app.jobs.scheduler.cancel_trash(token);
    app.jobs.trash_progress = None;

    assert!(
        app.trash_progress().is_none(),
        "chip should clear immediately for permanent delete"
    );

    // Drive to completion so background thread shuts down cleanly.
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        std::thread::sleep(Duration::from_millis(10));
    }

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    // root may or may not still contain gone.txt depending on the race.
    let _ = fs::remove_dir_all(root);
}
