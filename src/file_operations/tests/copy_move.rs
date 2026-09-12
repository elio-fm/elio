use super::clipboard_test_support::*;

// ── yank / copy path ─────────────────────────────────────────────────────────

#[test]
fn yank_and_paste_copies_file_to_destination() {
    let src_dir = temp_path("yank-src");
    let dst_dir = temp_path("yank-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("hello.txt"), "data").unwrap();

    // Navigate into src_dir so the entry appears in the list.
    let mut app = App::new_at(src_dir.clone()).unwrap();
    assert_eq!(app.file_browser.entries.len(), 1);

    // Yank the selected entry.
    app.yank();
    assert_eq!(
        app.file_operations.clipboard_info(),
        Some((1, ClipOp::Yank)),
        "clipboard should hold the yanked path"
    );

    // Point cwd at the destination (direct assignment avoids the async
    // directory-load path; we only care about the paste behaviour here).
    app.file_browser.cwd = dst_dir.clone();
    app.paste().unwrap();

    // paste() should immediately set up paste_progress.
    assert!(
        app.file_operations.paste_progress().is_some(),
        "paste_progress should be set while paste is in flight"
    );
    let (_, total, op) = app.file_operations.paste_progress().unwrap();
    assert_eq!(total, 1);
    assert_eq!(op, ClipOp::Yank);

    // Clipboard is consumed immediately on paste().
    assert!(
        app.file_operations.clipboard_info().is_none(),
        "clipboard should be cleared after paste"
    );

    wait_for_paste(&mut app);

    // File should exist in the destination.
    assert!(
        dst_dir.join("hello.txt").exists(),
        "copied file should exist in destination"
    );
    // Source must still exist for yank (copy).
    assert!(
        src_dir.join("hello.txt").exists(),
        "source file should still exist after yank-paste"
    );

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

// ── external drop path ────────────────────────────────────────────────────────

#[test]
fn external_drop_copy_copies_file_to_current_directory() {
    let src_dir = temp_path("drop-copy-src");
    let dst_dir = temp_path("drop-copy-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    let source = src_dir.join("copy_me.txt");
    fs::write(&source, "payload").unwrap();

    let mut app = App::new_at(dst_dir.clone()).unwrap();
    assert!(
        app.drop_external_paths(vec![source.clone()], ClipOp::Yank)
            .unwrap()
    );
    wait_for_paste_and_reload(&mut app);

    assert_eq!(
        fs::read_to_string(dst_dir.join("copy_me.txt")).unwrap(),
        "payload"
    );
    assert!(source.exists(), "copy drop should keep the source file");
    assert_eq!(app.status_message(), "Copied 1 item");

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

#[test]
fn external_drop_move_moves_file_to_current_directory() {
    let src_dir = temp_path("drop-move-src");
    let dst_dir = temp_path("drop-move-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    let source = src_dir.join("move_me.txt");
    fs::write(&source, "payload").unwrap();

    let mut app = App::new_at(dst_dir.clone()).unwrap();
    assert!(
        app.drop_external_paths(vec![source.clone()], ClipOp::Cut)
            .unwrap()
    );
    wait_for_paste_and_reload(&mut app);

    assert_eq!(
        fs::read_to_string(dst_dir.join("move_me.txt")).unwrap(),
        "payload"
    );
    assert!(!source.exists(), "move drop should remove the source file");
    assert_eq!(app.status_message(), "Moved 1 item");

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

#[test]
fn external_drop_move_same_directory_is_clean_noop() {
    let root = temp_path("drop-move-same-dir");
    fs::create_dir_all(&root).unwrap();
    let source = root.join("already_here.txt");
    fs::write(&source, "payload").unwrap();

    let mut app = App::new_at(root.clone()).unwrap();
    assert!(
        !app.drop_external_paths(vec![source.clone()], ClipOp::Cut)
            .unwrap()
    );

    assert_eq!(fs::read_to_string(&source).unwrap(), "payload");
    assert!(!root.join("already_here_1.txt").exists());
    assert_eq!(app.status_message(), "Already here");

    fs::remove_dir_all(&root).unwrap();
}

// ── cut / move path ───────────────────────────────────────────────────────────

#[test]
fn cut_and_paste_moves_file_to_destination() {
    let src_dir = temp_path("cut-src");
    let dst_dir = temp_path("cut-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("move_me.txt"), "payload").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    assert_eq!(app.file_browser.entries.len(), 1);

    app.cut();
    assert_eq!(app.file_operations.clipboard_info(), Some((1, ClipOp::Cut)));

    app.file_browser.cwd = dst_dir.clone();
    app.paste().unwrap();
    wait_for_paste(&mut app);

    assert!(
        dst_dir.join("move_me.txt").exists(),
        "file should be present at destination after move"
    );
    assert!(
        !src_dir.join("move_me.txt").exists(),
        "source file should be gone after move"
    );

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

#[test]
fn yank_allows_selected_parent_of_current_directory() {
    let root = temp_path("yank-parent-selection");
    let parent = root.join("parent");
    fs::create_dir_all(&parent).unwrap();

    let mut app = App::new_at(root.clone()).unwrap();
    app.file_browser.selected_paths.insert(parent.clone());
    app.file_browser.cwd = parent.clone();

    app.yank();

    assert_eq!(app.status_message(), "");
    assert_eq!(
        app.file_operations.clipboard_info(),
        Some((1, ClipOp::Yank))
    );
    assert!(app.file_browser.selected_paths.is_empty());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cut_allows_selected_parent_of_current_directory() {
    let root = temp_path("cut-parent-selection");
    let parent = root.join("parent");
    fs::create_dir_all(&parent).unwrap();

    let mut app = App::new_at(root.clone()).unwrap();
    app.file_browser.selected_paths.insert(parent.clone());
    app.file_browser.cwd = parent.clone();

    app.cut();

    assert_eq!(app.status_message(), "");
    assert_eq!(app.file_operations.clipboard_info(), Some((1, ClipOp::Cut)));
    assert!(app.file_browser.selected_paths.is_empty());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn paste_refuses_folder_into_itself() {
    let root = temp_path("paste-folder-into-self");
    let source = root.join("source");
    let child = source.join("child");
    fs::create_dir_all(&child).unwrap();

    let mut app = App::new_at(child.clone()).unwrap();
    app.file_operations.clipboard = Some(crate::file_operations::Clipboard {
        paths: vec![source.clone()],
        op: ClipOp::Yank,
    });

    app.paste().unwrap();

    assert_eq!(app.status_message(), "Cannot paste a folder into itself");
    assert!(app.file_operations.paste_progress().is_none());
    assert_eq!(
        app.file_operations.clipboard_info(),
        Some((1, ClipOp::Yank))
    );
    assert!(!child.join("source").exists());

    fs::remove_dir_all(&root).unwrap();
}

// ── progress state machine ────────────────────────────────────────────────────

#[test]
fn paste_progress_reflects_total_and_is_cleared_after_completion() {
    let src_dir = temp_path("progress-src");
    let dst_dir = temp_path("progress-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("a.txt"), "a").unwrap();
    fs::write(src_dir.join("b.txt"), "b").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    // Insert both paths into the multi-selection directly (selected_paths is
    // pub(super) within crate::app, which includes this test module).
    app.file_browser
        .selected_paths
        .insert(src_dir.join("a.txt"));
    app.file_browser
        .selected_paths
        .insert(src_dir.join("b.txt"));
    app.yank();

    app.file_browser.cwd = dst_dir.clone();
    app.paste().unwrap();

    // Immediately after paste() the progress should be live with total = 2.
    assert_eq!(
        app.file_operations.paste_progress().map(|(_, t, _)| t),
        Some(2),
        "paste_progress total should match the number of yanked items"
    );

    wait_for_paste(&mut app);

    assert!(
        app.file_operations.paste_progress().is_none(),
        "paste_progress should be None after done"
    );
    assert!(dst_dir.join("a.txt").exists());
    assert!(dst_dir.join("b.txt").exists());

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

// ── stale-token rejection ─────────────────────────────────────────────────────

#[test]
fn stale_token_paste_results_are_ignored() {
    let src_dir = temp_path("stale-src");
    let dst_dir = temp_path("stale-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("file.txt"), "x").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.yank();
    app.file_browser.cwd = dst_dir.clone();
    app.paste().unwrap();

    // Simulate a newer paste superseding the old one: bump paste_token and
    // clear paste_progress manually so we can verify nothing revives it.
    app.file_operations.paste_token = app.file_operations.paste_token.wrapping_add(1);
    app.file_operations.paste_progress = None;

    // Drain all incoming results.  Because none carry the current token they
    // must all be silently discarded.
    for _ in 0..300 {
        let _ = app.process_background_jobs();
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        app.file_operations.paste_progress().is_none(),
        "stale results must not update paste_progress"
    );

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

// ── user cancellation ─────────────────────────────────────────────────────────

#[test]
fn cancelling_paste_clears_progress_and_stops_worker() {
    let src_dir = temp_path("cancel-src");
    let dst_dir = temp_path("cancel-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("x.txt"), "x").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.yank();
    app.file_browser.cwd = dst_dir.clone();
    app.paste().unwrap();

    assert!(
        app.file_operations.paste_progress().is_some(),
        "progress should be live before cancel"
    );

    // Simulate Esc: cancel the current paste token and clear progress immediately.
    app.job_scheduler
        .cancel_paste(app.file_operations.paste_token);
    app.file_operations.paste_progress = None;

    assert!(
        app.file_operations.paste_progress().is_none(),
        "progress should be gone immediately after cancel"
    );

    // Drain results.  The worker will finish its current item and send
    // done=true with token matching the cancelled paste.  The results handler
    // should call queue_directory_load (which is fine — we want a reload after
    // cancel), but paste_progress must stay None throughout.
    for _ in 0..300 {
        let _ = app.process_background_jobs();
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        app.file_operations.paste_progress().is_none(),
        "paste_progress must stay None after cancel drain"
    );

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

// ── cancel old paste, immediately start new paste ─────────────────────────────

#[test]
fn new_paste_after_cancel_is_not_affected_by_old_cancel_token() {
    let src_dir = temp_path("recancel-src");
    let dst1 = temp_path("recancel-dst1");
    let dst2 = temp_path("recancel-dst2");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst1).unwrap();
    fs::create_dir_all(&dst2).unwrap();
    fs::write(src_dir.join("file.txt"), "payload").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();

    // First paste → cancel immediately (token 1 is cancelled).
    app.yank();
    app.file_browser.cwd = dst1.clone();
    app.paste().unwrap();
    let cancelled_token = app.file_operations.paste_token; // == 1
    app.job_scheduler.cancel_paste(cancelled_token);
    app.file_operations.paste_progress = None;

    // Re-yank and start a second paste to a different destination.  Its token
    // is 2; cancel_token stored in PasteShared is still 1, so the second
    // paste must NOT be stopped.
    app.file_operations.clipboard = Some(crate::file_operations::Clipboard {
        paths: vec![src_dir.join("file.txt")],
        op: ClipOp::Yank,
    });
    app.file_browser.cwd = dst2.clone();
    app.paste().unwrap();

    assert_ne!(
        app.file_operations.paste_token, cancelled_token,
        "new paste should have a different token"
    );

    wait_for_paste(&mut app);

    assert!(
        dst2.join("file.txt").exists(),
        "second paste must complete even though token-1 was cancelled"
    );

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst1).unwrap();
    fs::remove_dir_all(&dst2).unwrap();
}

// ── queued pastes ────────────────────────────────────────────────────────────

#[test]
fn yank_paste_then_yank_paste_queues_the_second_snapshot() {
    let src_dir = temp_path("queue-src");
    let dst1 = temp_path("queue-dst-1");
    let dst2 = temp_path("queue-dst-2");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst1).unwrap();
    fs::create_dir_all(&dst2).unwrap();
    fs::write(src_dir.join("a.txt"), "a").unwrap();
    fs::write(src_dir.join("b.txt"), "b").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();

    app.yank();
    app.file_browser.cwd = dst1.clone();
    app.paste().unwrap();

    let token_after_first = app.file_operations.paste_token;
    assert!(app.file_operations.paste_progress().is_some());

    // Queue a second paste after changing both the source selection and the
    // destination directory.  The queued snapshot must preserve both.
    app.file_browser.cwd = src_dir.clone();
    app.select_index(1);
    app.yank();
    app.file_browser.cwd = dst2.clone();
    app.paste().unwrap();

    assert_eq!(
        app.file_operations.paste_token, token_after_first,
        "paste_token must not change until the queued paste actually starts"
    );
    assert_eq!(
        app.file_operations.queued_pastes.len(),
        1,
        "second paste should be queued"
    );
    assert!(
        app.status.contains("Queued paste"),
        "status should indicate that the second paste was queued"
    );
    assert_eq!(app.file_operations.queued_pastes[0].dest_dir, dst2);
    assert_eq!(
        app.file_operations.queued_pastes[0].paths,
        vec![src_dir.join("b.txt")]
    );

    wait_for_paste(&mut app);

    assert!(dst1.join("a.txt").exists());
    assert!(dst2.join("b.txt").exists());

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst1).unwrap();
    fs::remove_dir_all(&dst2).unwrap();
}

#[test]
fn queued_paste_with_missing_destination_fails_and_later_queue_continues() {
    let src_dir = temp_path("queue-missing-src");
    let dst1 = temp_path("queue-missing-dst-1");
    let missing_dst = temp_path("queue-missing-dst-2");
    let dst3 = temp_path("queue-missing-dst-3");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst1).unwrap();
    fs::create_dir_all(&dst3).unwrap();
    fs::write(src_dir.join("a.txt"), "a").unwrap();
    fs::write(src_dir.join("b.txt"), "b").unwrap();
    fs::write(src_dir.join("c.txt"), "c").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.yank();
    app.file_browser.cwd = dst1.clone();
    app.paste().unwrap();

    app.file_operations.clipboard = Some(crate::file_operations::Clipboard {
        paths: vec![src_dir.join("b.txt")],
        op: ClipOp::Yank,
    });
    app.file_browser.cwd = missing_dst.clone();
    app.paste().unwrap();

    app.file_operations.clipboard = Some(crate::file_operations::Clipboard {
        paths: vec![src_dir.join("c.txt")],
        op: ClipOp::Yank,
    });
    app.file_browser.cwd = dst3.clone();
    app.paste().unwrap();

    assert_eq!(app.file_operations.queued_pastes.len(), 2);

    wait_for_paste(&mut app);

    assert!(dst1.join("a.txt").exists());
    assert!(
        !missing_dst.join("b.txt").exists(),
        "paste into a missing destination should fail"
    );
    assert!(
        dst3.join("c.txt").exists(),
        "a later queued paste should still run after an earlier queued failure"
    );

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst1).unwrap();
    fs::remove_dir_all(&dst3).unwrap();
}

#[test]
fn queued_same_destination_pastes_defer_reload_until_queue_drains() {
    let src_dir = temp_path("queue-same-dst-src");
    let dst_dir = temp_path("queue-same-dst-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("a.txt"), "a").unwrap();
    fs::write(src_dir.join("b.txt"), "b").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.yank();
    app.file_browser.cwd = dst_dir.clone();
    app.paste().unwrap();
    let first_token = app.file_operations.paste_token;

    app.file_operations.clipboard = Some(crate::file_operations::Clipboard {
        paths: vec![src_dir.join("b.txt")],
        op: ClipOp::Yank,
    });
    app.file_browser.cwd = dst_dir.clone();
    app.paste().unwrap();

    let mut queued_started = false;
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.file_operations.paste_token != first_token {
            queued_started = true;
            let reload_queued = app.file_browser.directory_runtime.pending_load.is_some();
            let queue_drained = app.file_operations.paste_progress().is_none()
                && app.file_operations.queued_pastes.is_empty();
            assert!(
                !reload_queued || queue_drained,
                "reload should stay deferred until the queued paste to the same destination has finished"
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        queued_started,
        "queued paste should start after the first one finishes"
    );

    wait_for_paste_and_reload(&mut app);

    assert!(dst_dir.join("a.txt").exists());
    assert!(dst_dir.join("b.txt").exists());
    assert_eq!(app.status_message(), "Copied 1 item");

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

#[test]
fn esc_cancels_active_paste_and_clears_queued_pastes() {
    let src_dir = temp_path("queue-cancel-src");
    let dst1 = temp_path("queue-cancel-dst-1");
    let dst2 = temp_path("queue-cancel-dst-2");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst1).unwrap();
    fs::create_dir_all(&dst2).unwrap();
    fs::write(src_dir.join("a.txt"), "a").unwrap();
    fs::write(src_dir.join("b.txt"), "b").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.yank();
    app.file_browser.cwd = dst1.clone();
    app.paste().unwrap();

    app.file_operations.clipboard = Some(crate::file_operations::Clipboard {
        paths: vec![src_dir.join("b.txt")],
        op: ClipOp::Yank,
    });
    app.file_browser.cwd = dst2.clone();
    app.paste().unwrap();
    assert_eq!(app.file_operations.queued_pastes.len(), 1);

    app.handle_event(crossterm::event::Event::Key(
        crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Esc),
    ))
    .unwrap();

    assert!(app.file_operations.paste_progress().is_none());
    assert!(
        app.file_operations.queued_pastes.is_empty(),
        "Esc should clear queued pastes as well as the active paste"
    );

    for _ in 0..300 {
        let _ = app.process_background_jobs();
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        !dst2.join("b.txt").exists(),
        "queued paste should not run after Esc cancels the queue"
    );

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst1).unwrap();
    fs::remove_dir_all(&dst2).unwrap();
}

// ── nothing-to-paste ─────────────────────────────────────────────────────────

#[test]
fn paste_with_empty_clipboard_sets_status_and_leaves_no_progress() {
    let dir = temp_path("empty-paste");
    fs::create_dir_all(&dir).unwrap();

    let mut app = App::new_at(dir.clone()).unwrap();
    app.paste().unwrap();

    assert_eq!(app.status, "Nothing to paste");
    assert!(app.file_operations.paste_progress().is_none());

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn paste_during_active_paste_without_clipboard_explains_how_to_queue() {
    let src_dir = temp_path("queue-hint-src");
    let dst_dir = temp_path("queue-hint-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("a.txt"), "a").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.yank();
    app.file_browser.cwd = dst_dir.clone();
    app.paste().unwrap();

    let token = app.file_operations.paste_token;
    app.paste().unwrap();

    assert_eq!(app.file_operations.paste_token, token);
    assert_eq!(
        app.status,
        "Paste in progress — yank or cut another item to queue it"
    );

    wait_for_paste(&mut app);

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}
