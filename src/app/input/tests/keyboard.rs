use super::super::*;
use super::helpers::{temp_path, wait_for_background_preview, wait_for_directory_load};
use crate::config::Action;
use std::{
    fs, thread,
    time::{Duration, Instant},
};

/// Returns true if `gio mime text/plain` succeeds on this system.
/// Used to tighten open-with tests: when gio is available the overlay
/// must open for common file types; without gio the tests skip gracefully.
#[cfg(all(unix, not(target_os = "macos")))]
fn gio_available() -> bool {
    std::process::Command::new("gio")
        .args(["mime", "text/plain"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(all(unix, not(target_os = "macos")))]
struct OpenInSystemCaptureGuard;

#[cfg(all(unix, not(target_os = "macos")))]
impl OpenInSystemCaptureGuard {
    fn install(path: std::path::PathBuf) -> Self {
        crate::fs::set_open_in_system_capture_for_test(Some(path));
        Self
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
impl Drop for OpenInSystemCaptureGuard {
    fn drop(&mut self) {
        crate::fs::set_open_in_system_capture_for_test(None);
    }
}

#[test]
fn shift_slash_opens_and_closes_help_overlay() {
    let root = temp_path("help-shift-slash");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('/'),
        KeyModifiers::SHIFT,
    )))
    .expect("shift-slash should open help");
    assert!(app.overlays.help);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('/'),
        KeyModifiers::SHIFT,
    )))
    .expect("shift-slash should close help");
    assert!(!app.overlays.help);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn q_sets_should_quit() {
    let root = temp_path("quit-shortcut");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    assert!(!app.should_quit);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('q'))))
        .expect("q should request quit");

    assert!(app.should_quit);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn enter_opens_selected_file_with_system_opener() {
    let root = temp_path("enter-opens-file");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("note.txt");
    fs::write(&file_path, "hello").expect("failed to write temp file");
    let capture = root.join("capture.txt");
    let _capture_guard = OpenInSystemCaptureGuard::install(capture.clone());

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .expect("enter should open selected file");

    let deadline = Instant::now() + Duration::from_millis(1000);
    while !capture.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }

    let opened = fs::read_to_string(&capture).expect("capture should exist");
    assert_eq!(opened, file_path.display().to_string());

    fs::remove_dir_all(root).ok();
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn newline_key_event_also_opens_selected_file() {
    let root = temp_path("newline-opens-file");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("note.txt");
    fs::write(&file_path, "hello").expect("failed to write temp file");
    let capture = root.join("capture.txt");
    let _capture_guard = OpenInSystemCaptureGuard::install(capture.clone());

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('\n'),
        KeyModifiers::NONE,
    )))
    .expect("newline key event should open selected file");

    let deadline = Instant::now() + Duration::from_millis(1000);
    while !capture.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }

    let opened = fs::read_to_string(&capture).expect("capture should exist");
    assert_eq!(opened, file_path.display().to_string());

    fs::remove_dir_all(root).ok();
}

#[test]
fn c_opens_and_esc_closes_copy_overlay() {
    let root = temp_path("copy-overlay-shortcut");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("report.txt"), "hello").expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('c'))))
        .expect("c should open copy overlay");
    assert!(app.copy_is_open());

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Esc)))
        .expect("esc should close copy overlay");
    assert!(!app.copy_is_open());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn g_opens_goto_overlay_and_goto_shortcuts_keep_g_for_top() {
    let root = temp_path("goto-overlay-shortcut");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.select_last();
    assert_eq!(
        app.navigation.selected, 2,
        "G behavior should still reach the last item"
    );

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('g'))))
        .expect("g should open go-to overlay");
    assert!(app.goto_is_open());
    assert_eq!(
        app.navigation.selected, 2,
        "opening the go-to overlay should not move selection"
    );

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('g'))))
        .expect("g inside go-to overlay should jump to top");
    assert!(!app.goto_is_open());
    assert_eq!(
        app.navigation.selected, 0,
        "go-to g shortcut should move to the top item"
    );

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('G'))))
        .expect("capital G should still move to the last item");
    assert_eq!(
        app.navigation.selected, 2,
        "capital G should keep the old bottom-jump behavior"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tab_and_shift_tab_cycle_sidebar_locations_and_skip_section_rows() {
    let root = temp_path("tab-cycles-pinned-places");
    let downloads = root.join("downloads");
    let usb = root.join("usb");
    fs::create_dir_all(&downloads).expect("failed to create downloads dir");
    fs::create_dir_all(&usb).expect("failed to create usb dir");

    let sidebar_rows = || {
        vec![
            SidebarRow::Item(SidebarItem::new(
                SidebarItemKind::Home,
                "Home",
                "H",
                root.clone(),
            )),
            SidebarRow::Item(SidebarItem::new(
                SidebarItemKind::Downloads,
                "Downloads",
                "D",
                downloads.clone(),
            )),
            SidebarRow::Section { title: "Devices" },
            SidebarRow::Item(SidebarItem::new(
                SidebarItemKind::Device { removable: true },
                "USB",
                "U",
                usb.clone(),
            )),
        ]
    };

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.sidebar = sidebar_rows();

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Tab)))
        .expect("tab should cycle sidebar locations");
    wait_for_directory_load(&mut app);
    assert_eq!(app.navigation.cwd, downloads);

    app.navigation.sidebar = sidebar_rows();
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Tab)))
        .expect("tab should continue into device rows");
    wait_for_directory_load(&mut app);
    assert_eq!(app.navigation.cwd, usb);

    app.navigation.sidebar = sidebar_rows();
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Tab)))
        .expect("tab should wrap back to the first sidebar location");
    wait_for_directory_load(&mut app);
    assert_eq!(app.navigation.cwd, root);

    app.navigation.sidebar = sidebar_rows();
    app.set_dir(usb.clone()).expect("device path should open");
    wait_for_directory_load(&mut app);

    app.navigation.sidebar = sidebar_rows();
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::BackTab)))
        .expect("shift-tab should walk sidebar locations in reverse");
    wait_for_directory_load(&mut app);
    assert_eq!(app.navigation.cwd, downloads);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn repeated_down_arrow_is_throttled_without_starving_hold_repeat() {
    let root = temp_path("down-arrow-debounce");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("first down arrow should be handled");

    let throttled_at = app
        .input
        .last_navigation_key
        .expect("accepted navigation key should be tracked")
        .1;
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("second down arrow should be handled");

    assert_eq!(app.navigation.selected, 1);
    assert_eq!(
        app.input
            .last_navigation_key
            .expect("throttled navigation key should keep prior timestamp")
            .1,
        throttled_at
    );

    app.input.last_navigation_key = Some((
        NavigationRepeatKey::Down,
        Instant::now() - KEY_REPEAT_NAV_INTERVAL - Duration::from_millis(1),
    ));
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("third down arrow should be handled");

    assert_eq!(app.navigation.selected, 2);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_alt_right_scrolls_preview_instead_of_history() {
    let root = temp_path("preview-horizontal-alt-right");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.rs");
    fs::write(
        &file_path,
        "fn main() { let preview_line = \"this line is intentionally long for horizontal preview scrolling\"; }\n",
    )
    .expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.input.last_wheel_target = Some(WheelTarget::Entries);
    app.select_index(0);
    app.input.last_selection_change_at =
        Instant::now() - PREVIEW_AUTO_FOCUS_DELAY - Duration::from_millis(1);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 12,
        ..FrameState::default()
    });

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT)))
        .expect("alt-right should be handled");

    assert!(app.preview.state.horizontal_scroll > 0);
    assert_eq!(app.navigation.selected, 0);
    assert_eq!(app.input.last_wheel_target, Some(WheelTarget::Preview));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_down_arrow_keeps_browser_navigation() {
    let root = temp_path("high-frequency-down-keeps-browser");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.input.last_wheel_target = Some(WheelTarget::Preview);
    app.input.last_selection_change_at =
        Instant::now() - PREVIEW_AUTO_FOCUS_DELAY - Duration::from_millis(1);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 20,
        ..FrameState::default()
    });

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("down arrow should be handled");

    assert_eq!(app.navigation.selected, 1);
    assert_eq!(app.preview.state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_right_arrow_in_list_view_still_enters_directory() {
    let root = temp_path("high-frequency-right-enters");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.input.last_wheel_target = Some(WheelTarget::Preview);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )))
    .expect("right arrow should be handled");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn rapid_audio_navigation_defers_second_cold_heavy_preview_refresh() {
    let root = temp_path("rapid-audio-navigation-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.mp3", "b.mp3", "c.mp3"] {
        fs::write(root.join(name), name).expect("failed to write temp audio file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.set_media_ffprobe_available_for_tests(false);
    app.set_media_ffmpeg_available_for_tests(false);
    app.input.last_selection_change_at =
        Instant::now() - WHEEL_SCROLL_BURST_WINDOW - Duration::from_millis(1);

    let initial_token = app.preview.state.token;
    app.move_vertical(1);

    // Cold heavy audio is always deferred regardless of burst window state.
    assert_eq!(app.navigation.selected, 1);
    assert_eq!(app.preview.state.token, initial_token);
    assert!(app.preview.state.deferred_refresh_at.is_some());

    app.move_vertical(1);

    assert_eq!(app.navigation.selected, 2);
    assert_eq!(app.preview.state.token, initial_token);
    assert!(app.preview.state.deferred_refresh_at.is_some());

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    assert!(app.preview.state.token > initial_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn rapid_key_navigation_defers_preview_for_non_heavy_files() {
    let root = temp_path("rapid-key-nav-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);

    // First move: last_key_nav_at is in the past → Immediate preview.
    let token_before = app.preview.state.token;
    app.move_vertical_keyboard(1);
    assert_eq!(app.navigation.selected, 1);
    assert!(
        app.preview.state.token > token_before,
        "first move should trigger an immediate preview refresh"
    );
    assert!(
        app.preview.state.deferred_refresh_at.is_none(),
        "first move should not leave a deferred timer"
    );

    // Second move within KEY_NAV_RAPID_THRESHOLD → Deferred preview.
    let token_before = app.preview.state.token;
    app.move_vertical_keyboard(1);
    assert_eq!(app.navigation.selected, 2);
    assert_eq!(
        app.preview.state.token, token_before,
        "second rapid move should not immediately refresh preview"
    );
    assert!(
        app.preview.state.deferred_refresh_at.is_some(),
        "second rapid move should schedule a deferred refresh"
    );

    // After the deferred delay the preview fires.
    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    assert!(
        app.preview.state.token > token_before,
        "deferred preview should fire after pause"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn rapid_key_navigation_clears_directory_totals_until_deferred_refresh_runs() {
    let root = temp_path("rapid-key-nav-directory-stats");
    for dir in ["a-dir", "b-dir"] {
        let path = root.join(dir);
        fs::create_dir_all(&path).expect("failed to create temp dir");
        fs::write(path.join("file.txt"), vec![b'x'; 100]).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);
    wait_for_background_preview(&mut app);
    for _ in 0..100 {
        let _ = app.process_background_jobs();
        if matches!(
            app.preview.state.directory_stats,
            Some(PreviewDirectoryStatsState::Complete { .. })
        ) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(
        app.preview.state.directory_stats,
        Some(PreviewDirectoryStatsState::Complete { .. })
    ));

    app.input.last_key_nav_at = Instant::now();
    let token_before = app.preview.state.token;
    app.move_vertical_keyboard(1);

    assert_eq!(app.navigation.selected, 1);
    assert_eq!(app.preview.state.token, token_before);
    assert!(app.preview.state.deferred_refresh_at.is_some());
    assert!(app.preview.state.directory_stats.is_none());

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    for _ in 0..100 {
        let _ = app.process_background_jobs();
        if app.preview_header_detail_for_width(8, 80).as_deref()
            == Some(&format!("1 total item • {}", crate::app::format_size(100)))
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        app.preview_header_detail_for_width(8, 80).as_deref(),
        Some(format!("1 total item • {}", crate::app::format_size(100)).as_str())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_alt_right_does_not_trigger_history_navigation() {
    let root = temp_path("high-frequency-alt-right-no-history");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);
    app.go_back().expect("go back should succeed");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT)))
        .expect("alt-right should be handled");

    assert_eq!(app.navigation.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(child.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

// --- keybindings integration tests ---

/// Parses a [keys] override, derives an action via KeyBindings::action_for,
/// dispatches it through App::dispatch_action, and checks the resulting state.
/// This covers the full config-to-runtime pipeline:
///   TOML string → KeyBindings → action_for → dispatch_action → app state
///
/// Note: App::handle_event reads from the process-wide config singleton, so we
/// cannot inject per-test overrides there.  The dispatch_action path (which
/// handle_event delegates to for every configurable key) is tested here
/// directly with a parsed KeyBindings, giving equivalent coverage.
#[test]
fn rebound_yank_key_dispatches_yank_action() {
    use crate::config::KeyBindings;

    // Parse a config that rebinds yank from "y" to "Y".
    let kb = KeyBindings::from_toml_str("[keys]\nyank = \"Y\"");
    assert_eq!(
        kb.action_for('Y'),
        Some(Action::Yank),
        "new key should map to Yank"
    );
    assert_eq!(
        kb.action_for('y'),
        None,
        "old key should no longer map to Yank"
    );

    // Create an app with a file so yank has something to act on.
    let root = temp_path("rebind-yank-e2e");
    fs::write(root.join("file.txt"), "hello").expect("failed to write file");
    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.select_index(0);

    assert!(app.jobs.clipboard.is_none(), "clipboard should start empty");

    // Dispatch the action the rebound key would trigger.
    let action = kb.action_for('Y').expect("Y should be bound");
    app.dispatch_action(action)
        .expect("dispatch should succeed");

    assert!(
        app.jobs.clipboard.is_some(),
        "yank should have populated the clipboard"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn rebound_quit_key_sets_should_quit() {
    use crate::config::KeyBindings;

    let kb = KeyBindings::from_toml_str("[keys]\nquit = \"Q\"");
    assert_eq!(kb.action_for('Q'), Some(Action::Quit));
    assert_eq!(kb.action_for('q'), None);

    let root = temp_path("rebind-quit-e2e");
    let mut app = App::new_at(root.clone()).expect("failed to create app");
    assert!(!app.should_quit);

    app.dispatch_action(kb.action_for('Q').unwrap())
        .expect("dispatch should succeed");
    assert!(app.should_quit);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn capital_o_opens_open_with_overlay_for_selected_file() {
    let root = temp_path("open-with-overlay-file");
    fs::write(root.join("document.txt"), "hello").expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('O'))))
        .expect("O should be handled");

    let overlay_opened = app.overlays.open_with.is_some();

    // On systems where gio is available, text/plain must have handlers — the overlay
    // must open.  On minimal systems without gio we allow "No applications found".
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if gio_available() {
            assert!(
                overlay_opened,
                "gio is available — overlay must open for text/plain; status: {:?}",
                app.status
            );
        } else {
            let no_apps = app.status == "No apps found, opened with default";
            assert!(
                overlay_opened || no_apps,
                "O on a file should open overlay or report no apps; got status: {:?}",
                app.status
            );
        }
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let no_apps = app.status == "No apps found, opened with default";
        assert!(
            overlay_opened || no_apps,
            "O on a file should open overlay or report no apps; got status: {:?}",
            app.status
        );
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn capital_o_on_directory_sets_status_without_opening_overlay() {
    let root = temp_path("open-with-overlay-dir");
    let child = root.join("subdir");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('O'))))
        .expect("O on a directory should not fail");

    assert!(
        app.overlays.open_with.is_none(),
        "overlay should not open for a directory"
    );
    assert_eq!(app.status, "Open With is for files");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn esc_closes_open_with_overlay() {
    let root = temp_path("open-with-overlay-esc");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.inject_open_with_for_test("Fake App", "/usr/bin/true", vec![], false);

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Esc)))
        .expect("Esc should close the overlay");
    assert!(app.overlays.open_with.is_none());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn open_with_shortcut_confirms_row_and_closes_overlay() {
    let root = temp_path("open-with-overlay-confirm");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.inject_open_with_for_test("Fake App", "/usr/bin/true", vec![], false);

    let shortcut = app
        .open_with_row_shortcut(0)
        .expect("first row should have a shortcut");

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(shortcut))))
        .expect("shortcut should confirm the row");

    assert!(
        app.overlays.open_with.is_none(),
        "overlay should close after confirming"
    );
    // Confirming a row launches the app — status is cleared on success or shows
    // a failure message if the process could not be spawned.
    assert!(
        app.status.is_empty() || app.status.starts_with("Failed to open with"),
        "unexpected status after confirm: {:?}",
        app.status
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn ctrl_c_closes_open_with_overlay() {
    let root = temp_path("open-with-overlay-ctrl-c");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.inject_open_with_for_test("Fake App", "/usr/bin/true", vec![], false);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    )))
    .expect("Ctrl-C should close the overlay");
    assert!(app.overlays.open_with.is_none());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

// ── open-with launch behavior ─────────────────────────────────────────────────

/// Creates a shell script in `dir` that touches `sentinel` when run,
/// makes it executable, and returns its path.
#[cfg(unix)]
fn write_sentinel_script(dir: &std::path::Path, sentinel: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("fake-app.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\n", sentinel.display()),
    )
    .expect("write sentinel script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod sentinel script");
    script
}

#[cfg(unix)]
#[test]
fn detached_open_command_executes_program() {
    let dir = temp_path("detached-open-cmd");
    let sentinel = dir.join("ran");
    let script = write_sentinel_script(&dir, &sentinel);

    crate::fs::detached_open_command(script.to_str().unwrap(), &[])
        .expect("detached_open_command should succeed");

    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1000);
    while !sentinel.exists() && std::time::Instant::now() < deadline {
        thread::sleep(std::time::Duration::from_millis(10));
    }

    let ran = sentinel.exists(); // capture before cleanup
    fs::remove_dir_all(&dir).ok();
    assert!(ran, "script must have run");
}

#[cfg(unix)]
#[test]
fn confirm_open_with_launches_program_and_closes_overlay() {
    let dir = temp_path("open-with-launch");
    let sentinel = dir.join("launched");
    let script = write_sentinel_script(&dir, &sentinel);

    let root = temp_path("open-with-launch-root");
    fs::write(root.join("file.txt"), "hello").expect("write file");
    let mut app = App::new_at(root.clone()).expect("create app");
    wait_for_directory_load(&mut app);

    app.inject_open_with_for_test("Fake App", script.to_str().unwrap(), vec![], false);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('1'))))
        .expect("shortcut should fire");

    assert!(
        app.overlays.open_with.is_none(),
        "overlay must close after launch"
    );
    assert!(
        app.status.is_empty(),
        "status should be empty after successful launch; got: {:?}",
        app.status
    );

    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1000);
    while !sentinel.exists() && std::time::Instant::now() < deadline {
        thread::sleep(std::time::Duration::from_millis(10));
    }

    let launched = sentinel.exists(); // capture before cleanup
    fs::remove_dir_all(&dir).ok();
    fs::remove_dir_all(&root).ok();
    assert!(launched, "fake app must have been executed");
}

#[test]
fn confirm_open_with_launch_failure_sets_status() {
    let root = temp_path("open-with-fail");
    fs::write(root.join("file.txt"), "hello").expect("write file");
    let mut app = App::new_at(root.clone()).expect("create app");
    wait_for_directory_load(&mut app);

    // Point at a program that does not exist — spawn will fail.
    app.inject_open_with_for_test(
        "Ghost App",
        "/this/program/absolutely/does/not/exist",
        vec![],
        false,
    );
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('1'))))
        .expect("shortcut should fire");

    assert!(
        app.overlays.open_with.is_none(),
        "overlay must close even on failure"
    );
    assert_eq!(app.status, "Failed to open with Ghost App");

    fs::remove_dir_all(&root).ok();
}
