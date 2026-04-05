use super::super::App;
use crate::app::ClipOp;
use std::{
    env,
    ffi::OsString,
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// ── helpers ──────────────────────────────────────────────────────────────────

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-clipboard-{label}-{unique}"))
}

/// Poll `process_background_jobs` until there is no active paste and no queued
/// follow-up paste left to start, or the timeout expires.
fn wait_for_paste(app: &mut App) {
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.paste_progress().is_none() && app.jobs.queued_pastes.is_empty() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for paste to complete");
}

fn wait_for_paste_and_reload(app: &mut App) {
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.paste_progress().is_none()
            && app.jobs.queued_pastes.is_empty()
            && app.navigation.directory_runtime.pending_load.is_none()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for paste and directory reload to complete");
}

fn clipboard_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(unix)]
struct ClipboardEnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

#[cfg(unix)]
impl ClipboardEnvGuard {
    fn isolate() -> Self {
        const VARS: &[&str] = &[
            "ELIO_TEST_CLIPBOARD_TOOL",
            "ELIO_TEST_OSC52_CAPTURE",
            "ELIO_CLIPBOARD_OSC52",
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
fn install_fake_clipboard_tool(root: &std::path::Path, capture_path: &std::path::Path) -> PathBuf {
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
fn install_backgrounding_clipboard_tool(
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
    assert_eq!(app.navigation.entries.len(), 1);

    // Yank the selected entry.
    app.yank();
    assert_eq!(
        app.clipboard_info(),
        Some((1, ClipOp::Yank)),
        "clipboard should hold the yanked path"
    );

    // Point cwd at the destination (direct assignment avoids the async
    // directory-load path; we only care about the paste behaviour here).
    app.navigation.cwd = dst_dir.clone();
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
    assert_eq!(app.navigation.entries.len(), 1);

    app.cut();
    assert_eq!(app.clipboard_info(), Some((1, ClipOp::Cut)));

    app.navigation.cwd = dst_dir.clone();
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
    app.navigation.selected_paths.insert(src_dir.join("a.txt"));
    app.navigation.selected_paths.insert(src_dir.join("b.txt"));
    app.yank();

    app.navigation.cwd = dst_dir.clone();
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
    app.navigation.cwd = dst_dir.clone();
    app.paste().unwrap();

    // Simulate a newer paste superseding the old one: bump paste_token and
    // clear paste_progress manually so we can verify nothing revives it.
    app.jobs.paste_token = app.jobs.paste_token.wrapping_add(1);
    app.jobs.paste_progress = None;

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
    app.navigation.cwd = dst_dir.clone();
    app.paste().unwrap();

    assert!(
        app.paste_progress().is_some(),
        "progress should be live before cancel"
    );

    // Simulate Esc: cancel the current paste token and clear progress immediately.
    app.jobs.scheduler.cancel_paste(app.jobs.paste_token);
    app.jobs.paste_progress = None;

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
    app.navigation.cwd = dst1.clone();
    app.paste().unwrap();
    let cancelled_token = app.jobs.paste_token; // == 1
    app.jobs.scheduler.cancel_paste(cancelled_token);
    app.jobs.paste_progress = None;

    // Re-yank and start a second paste to a different destination.  Its token
    // is 2; cancel_token stored in PasteShared is still 1, so the second
    // paste must NOT be stopped.
    app.jobs.clipboard = Some(super::super::state::Clipboard {
        paths: vec![src_dir.join("file.txt")],
        op: ClipOp::Yank,
    });
    app.navigation.cwd = dst2.clone();
    app.paste().unwrap();

    assert_ne!(
        app.jobs.paste_token, cancelled_token,
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
    app.navigation.cwd = dst1.clone();
    app.paste().unwrap();

    let token_after_first = app.jobs.paste_token;
    assert!(app.paste_progress().is_some());

    // Queue a second paste after changing both the source selection and the
    // destination directory.  The queued snapshot must preserve both.
    app.navigation.cwd = src_dir.clone();
    app.select_index(1);
    app.yank();
    app.navigation.cwd = dst2.clone();
    app.paste().unwrap();

    assert_eq!(
        app.jobs.paste_token, token_after_first,
        "paste_token must not change until the queued paste actually starts"
    );
    assert_eq!(
        app.jobs.queued_pastes.len(),
        1,
        "second paste should be queued"
    );
    assert!(
        app.status.contains("Queued paste"),
        "status should indicate that the second paste was queued"
    );
    assert_eq!(app.jobs.queued_pastes[0].dest_dir, dst2);
    assert_eq!(app.jobs.queued_pastes[0].paths, vec![src_dir.join("b.txt")]);

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
    app.navigation.cwd = dst1.clone();
    app.paste().unwrap();

    app.jobs.clipboard = Some(super::super::state::Clipboard {
        paths: vec![src_dir.join("b.txt")],
        op: ClipOp::Yank,
    });
    app.navigation.cwd = missing_dst.clone();
    app.paste().unwrap();

    app.jobs.clipboard = Some(super::super::state::Clipboard {
        paths: vec![src_dir.join("c.txt")],
        op: ClipOp::Yank,
    });
    app.navigation.cwd = dst3.clone();
    app.paste().unwrap();

    assert_eq!(app.jobs.queued_pastes.len(), 2);

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
    app.navigation.cwd = dst_dir.clone();
    app.paste().unwrap();
    let first_token = app.jobs.paste_token;

    app.jobs.clipboard = Some(super::super::state::Clipboard {
        paths: vec![src_dir.join("b.txt")],
        op: ClipOp::Yank,
    });
    app.navigation.cwd = dst_dir.clone();
    app.paste().unwrap();

    let mut queued_started = false;
    for _ in 0..500 {
        let _ = app.process_background_jobs();
        if app.jobs.paste_token != first_token {
            queued_started = true;
            let reload_queued = app.navigation.directory_runtime.pending_load.is_some();
            let queue_drained = app.paste_progress().is_none() && app.jobs.queued_pastes.is_empty();
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
    app.navigation.cwd = dst1.clone();
    app.paste().unwrap();

    app.jobs.clipboard = Some(super::super::state::Clipboard {
        paths: vec![src_dir.join("b.txt")],
        op: ClipOp::Yank,
    });
    app.navigation.cwd = dst2.clone();
    app.paste().unwrap();
    assert_eq!(app.jobs.queued_pastes.len(), 1);

    app.handle_event(crossterm::event::Event::Key(
        crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Esc),
    ))
    .unwrap();

    assert!(app.paste_progress().is_none());
    assert!(
        app.jobs.queued_pastes.is_empty(),
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
    assert!(app.paste_progress().is_none());

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
    app.navigation.cwd = dst_dir.clone();
    app.paste().unwrap();

    let token = app.jobs.paste_token;
    app.paste().unwrap();

    assert_eq!(app.jobs.paste_token, token);
    assert_eq!(
        app.status,
        "Paste in progress — yank or cut another item to queue it"
    );

    wait_for_paste(&mut app);

    fs::remove_dir_all(&src_dir).unwrap();
    fs::remove_dir_all(&dst_dir).unwrap();
}

#[test]
fn copy_overlay_populates_expected_rows_for_selected_file() {
    let root = temp_path("copy-overlay-rows");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report.final.md");
    fs::write(&file, "notes").expect("failed to write test file");

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();

    assert!(app.copy_is_open(), "copy overlay should open");
    assert_eq!(app.copy_title(), "Copy to clipboard");
    assert_eq!(app.copy_row_count(), 4);
    assert_eq!(app.copy_row_label(0), "Copy file name");
    assert_eq!(app.copy_row_label(1), "Name without extension");
    assert_eq!(app.copy_row_label(2), "File path");
    assert_eq!(app.copy_row_label(3), "Directory path");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_shortcut_writes_expected_text_to_system_clipboard() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-copy");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    let capture = root.join("clipboard.txt");
    fs::write(&file, "notes").expect("failed to write test file");
    let tool = install_fake_clipboard_tool(&root, &capture);

    unsafe {
        env::set_var("ELIO_TEST_CLIPBOARD_TOOL", &tool);
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should succeed");

    let copied = fs::read_to_string(&capture).expect("fake clipboard tool should capture text");
    assert_eq!(
        copied,
        file.display().to_string(),
        "fake clipboard tool should capture the copied file path"
    );
    assert_eq!(app.status, "Copied file path");
    assert!(
        !app.copy_is_open(),
        "successful copy should close the overlay"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_shortcut_uses_osc52_when_no_clipboard_tool_is_installed() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-osc52");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    let capture = root.join("osc52.txt");
    fs::write(&file, "notes").expect("failed to write test file");

    unsafe {
        env::set_var("ELIO_TEST_OSC52_CAPTURE", &capture);
        env::set_var("TERM", "xterm-kitty");
        env::set_var("KITTY_WINDOW_ID", "1");
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should succeed");

    let osc52 = fs::read_to_string(&capture).expect("osc52 capture should exist");
    assert!(
        osc52.starts_with("\u{1b}]52;c;"),
        "expected osc52 clipboard escape, got: {osc52:?}"
    );
    assert!(
        osc52.ends_with("\u{1b}\\"),
        "expected osc52 clipboard escape terminator, got: {osc52:?}"
    );
    assert_eq!(app.status, "Copied file path");
    assert!(
        !app.copy_is_open(),
        "successful copy should close the overlay"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_shortcut_uses_osc52_in_alacritty_without_clipboard_tool() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-osc52-alacritty");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    let capture = root.join("osc52.txt");
    fs::write(&file, "notes").expect("failed to write test file");

    unsafe {
        env::set_var("ELIO_TEST_OSC52_CAPTURE", &capture);
        env::set_var("TERM", "alacritty");
        env::set_var("ALACRITTY_SOCKET", "/tmp/elio-alacritty.sock");
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should succeed");

    let osc52 = fs::read_to_string(&capture).expect("osc52 capture should exist");
    assert!(
        osc52.starts_with("\u{1b}]52;c;"),
        "expected osc52 clipboard escape, got: {osc52:?}"
    );
    assert!(
        osc52.ends_with("\u{1b}\\"),
        "expected osc52 clipboard escape terminator, got: {osc52:?}"
    );
    assert_eq!(app.status, "Copied file path");
    assert!(
        !app.copy_is_open(),
        "successful copy should close the overlay"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_shortcut_uses_osc52_override_for_unknown_terminals() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-osc52-override");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    let capture = root.join("osc52.txt");
    fs::write(&file, "notes").expect("failed to write test file");

    unsafe {
        env::set_var("ELIO_TEST_OSC52_CAPTURE", &capture);
        env::set_var("TERM", "vt100-unknown");
        env::set_var("ELIO_CLIPBOARD_OSC52", "1");
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should succeed");

    let osc52 = fs::read_to_string(&capture).expect("osc52 capture should exist");
    assert!(
        osc52.starts_with("\u{1b}]52;c;"),
        "expected osc52 clipboard escape, got: {osc52:?}"
    );
    assert!(
        osc52.ends_with("\u{1b}\\"),
        "expected osc52 clipboard escape terminator, got: {osc52:?}"
    );
    assert_eq!(app.status, "Copied file path");
    assert!(
        !app.copy_is_open(),
        "successful copy should close the overlay"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_reports_short_error_when_no_clipboard_backend_is_available() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-no-backend");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    let file = root.join("docs/report final.md");
    fs::write(&file, "notes").expect("failed to write test file");

    unsafe {
        env::set_var("TERM", "vt100-unknown");
        env::set_var("PATH", "");
    }

    let mut app = App::new_at(root.join("docs")).expect("failed to create app");
    app.open_copy_overlay();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('p'),
    ))
    .expect("copy shortcut should not error");

    assert_eq!(app.status, "Clipboard helper not found");
    assert!(
        app.copy_is_open(),
        "copy overlay should remain open when clipboard copy fails"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn copy_overlay_does_not_block_on_backgrounding_clipboard_helpers() {
    let _lock = clipboard_env_lock();
    let _env = ClipboardEnvGuard::isolate();
    let root = temp_path("copy-overlay-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let report = root.join("aaa-report.txt");
    fs::write(&report, "hello").expect("failed to write test file");
    let capture = root.join("clipboard.txt");
    let tool = install_backgrounding_clipboard_tool(&root, &capture);

    unsafe {
        env::set_var("ELIO_TEST_CLIPBOARD_TOOL", &tool);
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.open_copy_overlay();
    let start = std::time::Instant::now();
    app.handle_copy_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('c'),
    ))
    .expect("copy confirmation should succeed");

    assert!(
        start.elapsed() < Duration::from_millis(500),
        "copy confirmation should not block on helpers that hand work off to background processes"
    );
    assert_eq!(
        fs::read_to_string(&capture).expect("backgrounding clipboard tool should capture stdin"),
        report
            .file_name()
            .expect("test file should have a file name")
            .to_string_lossy()
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
