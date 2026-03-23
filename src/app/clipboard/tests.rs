use super::super::App;
use crate::app::ClipOp;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-clipboard-{label}-{unique}"))
}

/// Poll `process_background_jobs` until `paste_progress` is `None` (meaning
/// the worker sent its final `done=true` result and the directory reload was
/// queued) or the timeout expires.
fn wait_for_paste(app: &mut App) {
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.paste_progress().is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for paste to complete");
}

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
    assert_eq!(app.entries.len(), 1);

    // Yank the selected entry.
    app.yank();
    assert_eq!(
        app.clipboard_info(),
        Some((1, ClipOp::Yank)),
        "clipboard should hold the yanked path"
    );

    // Point cwd at the destination (direct assignment avoids the async
    // directory-load path; we only care about the paste behaviour here).
    app.cwd = dst_dir.clone();
    app.paste().unwrap();

    // paste() should immediately set up paste_progress.
    assert!(
        app.paste_progress().is_some(),
        "paste_progress should be set while paste is in flight"
    );
    let (_, total, op) = app.paste_progress().unwrap();
    assert_eq!(total, 1);
    assert_eq!(op, ClipOp::Yank);

    // Clipboard is consumed immediately on paste().
    assert!(
        app.clipboard_info().is_none(),
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

// ── cut / move path ───────────────────────────────────────────────────────────

#[test]
fn cut_and_paste_moves_file_to_destination() {
    let src_dir = temp_path("cut-src");
    let dst_dir = temp_path("cut-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("move_me.txt"), "payload").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    assert_eq!(app.entries.len(), 1);

    app.cut();
    assert_eq!(app.clipboard_info(), Some((1, ClipOp::Cut)));

    app.cwd = dst_dir.clone();
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
    app.selected_paths.insert(src_dir.join("a.txt"));
    app.selected_paths.insert(src_dir.join("b.txt"));
    app.yank();

    app.cwd = dst_dir.clone();
    app.paste().unwrap();

    // Immediately after paste() the progress should be live with total = 2.
    assert_eq!(
        app.paste_progress().map(|(_, t, _)| t),
        Some(2),
        "paste_progress total should match the number of yanked items"
    );

    wait_for_paste(&mut app);

    assert!(
        app.paste_progress().is_none(),
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
    app.cwd = dst_dir.clone();
    app.paste().unwrap();

    // Simulate a newer paste superseding the old one: bump paste_token and
    // clear paste_progress manually so we can verify nothing revives it.
    app.paste_token = app.paste_token.wrapping_add(1);
    app.paste_progress = None;

    // Drain all incoming results.  Because none carry the current token they
    // must all be silently discarded.
    for _ in 0..300 {
        let _ = app.process_background_jobs();
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        app.paste_progress().is_none(),
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
    app.cwd = dst_dir.clone();
    app.paste().unwrap();

    assert!(
        app.paste_progress().is_some(),
        "progress should be live before cancel"
    );

    // Simulate Esc: cancel the current paste token and clear progress immediately.
    app.scheduler.cancel_paste(app.paste_token);
    app.paste_progress = None;

    assert!(
        app.paste_progress().is_none(),
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
        app.paste_progress().is_none(),
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
    app.cwd = dst1.clone();
    app.paste().unwrap();
    let cancelled_token = app.paste_token; // == 1
    app.scheduler.cancel_paste(cancelled_token);
    app.paste_progress = None;

    // Re-yank and start a second paste to a different destination.  Its token
    // is 2; cancel_token stored in PasteShared is still 1, so the second
    // paste must NOT be stopped.
    app.clipboard = Some(super::super::state::Clipboard {
        paths: vec![src_dir.join("file.txt")],
        op: ClipOp::Yank,
    });
    app.cwd = dst2.clone();
    app.paste().unwrap();

    assert_ne!(
        app.paste_token, cancelled_token,
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

// ── second paste blocked while one is running ────────────────────────────────

#[test]
fn second_paste_is_blocked_while_one_is_in_progress() {
    let src_dir = temp_path("block-src");
    let dst_dir = temp_path("block-dst");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("a.txt"), "a").unwrap();
    fs::write(src_dir.join("b.txt"), "b").unwrap();

    let mut app = App::new_at(src_dir.clone()).unwrap();
    app.yank();
    app.cwd = dst_dir.clone();
    app.paste().unwrap();

    let token_after_first = app.paste_token;
    assert!(app.paste_progress().is_some());

    // Attempt a second paste while the first is still in flight.
    // clipboard is None (consumed), so set it directly.
    app.clipboard = Some(super::super::state::Clipboard {
        paths: vec![src_dir.join("b.txt")],
        op: ClipOp::Yank,
    });
    app.paste().unwrap();

    // Token must not have changed — the second paste was rejected.
    assert_eq!(
        app.paste_token, token_after_first,
        "paste_token must not change when second paste is blocked"
    );
    assert!(
        app.status.contains("in progress"),
        "status should indicate a paste is already running"
    );

    wait_for_paste(&mut app);

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

// ── nothing-to-paste ─────────────────────────────────────────────────────────

#[test]
fn paste_with_empty_clipboard_sets_status_and_leaves_no_progress() {
    let dir = temp_path("empty-paste");
    fs::create_dir_all(&dir).unwrap();

    let mut app = App::new_at(dir.clone()).unwrap();
    app.paste().unwrap();

    assert_eq!(app.status, "Nothing to paste");
    assert!(app.paste_progress().is_none());

    fs::remove_dir_all(&dir).unwrap();
}
