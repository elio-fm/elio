use super::super::*;
use super::helpers::{temp_path, wait_for_directory_load};
use std::{
    fs, thread,
    time::{Duration, Instant},
};

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
    assert!(app.help_open);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('/'),
        KeyModifiers::SHIFT,
    )))
    .expect("shift-slash should close help");
    assert!(!app.help_open);

    fs::remove_dir_all(root).expect("failed to remove temp root");
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
fn repeated_down_arrow_is_throttled_without_starving_hold_repeat() {
    let root = temp_path("down-arrow-debounce");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("first down arrow should be handled");

    let throttled_at = app
        .last_navigation_key
        .expect("accepted navigation key should be tracked")
        .1;
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("second down arrow should be handled");

    assert_eq!(app.selected, 1);
    assert_eq!(
        app.last_navigation_key
            .expect("throttled navigation key should keep prior timestamp")
            .1,
        throttled_at
    );

    app.last_navigation_key = Some((
        NavigationRepeatKey::Down,
        Instant::now() - KEY_REPEAT_NAV_INTERVAL - Duration::from_millis(1),
    ));
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
        .expect("third down arrow should be handled");

    assert_eq!(app.selected, 2);

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
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.last_wheel_target = Some(WheelTarget::Entries);
    app.select_index(0);
    app.last_selection_change_at =
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

    assert!(app.preview_state.horizontal_scroll > 0);
    assert_eq!(app.selected, 0);
    assert_eq!(app.last_wheel_target, Some(WheelTarget::Preview));

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
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.last_wheel_target = Some(WheelTarget::Preview);
    app.last_selection_change_at =
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

    assert_eq!(app.selected, 1);
    assert_eq!(app.preview_state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_right_arrow_in_list_view_still_enters_directory() {
    let root = temp_path("high-frequency-right-enters");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.last_wheel_target = Some(WheelTarget::Preview);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )))
    .expect("right arrow should be handled");
    wait_for_directory_load(&mut app);

    assert_eq!(app.cwd, child);

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
    app.view_mode = ViewMode::List;
    app.set_media_ffprobe_available_for_tests(false);
    app.set_media_ffmpeg_available_for_tests(false);
    app.last_selection_change_at =
        Instant::now() - WHEEL_SCROLL_BURST_WINDOW - Duration::from_millis(1);

    let initial_token = app.preview_state.token;
    app.move_vertical(1);

    // Cold heavy audio is always deferred regardless of burst window state.
    assert_eq!(app.selected, 1);
    assert_eq!(app.preview_state.token, initial_token);
    assert!(app.preview_state.deferred_refresh_at.is_some());

    app.move_vertical(1);

    assert_eq!(app.selected, 2);
    assert_eq!(app.preview_state.token, initial_token);
    assert!(app.preview_state.deferred_refresh_at.is_some());

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    assert!(app.preview_state.token > initial_token);

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
    app.view_mode = ViewMode::List;
    app.select_index(0);

    // First move: last_key_nav_at is in the past → Immediate preview.
    let token_before = app.preview_state.token;
    app.move_vertical_keyboard(1);
    assert_eq!(app.selected, 1);
    assert!(
        app.preview_state.token > token_before,
        "first move should trigger an immediate preview refresh"
    );
    assert!(
        app.preview_state.deferred_refresh_at.is_none(),
        "first move should not leave a deferred timer"
    );

    // Second move within KEY_NAV_RAPID_THRESHOLD → Deferred preview.
    let token_before = app.preview_state.token;
    app.move_vertical_keyboard(1);
    assert_eq!(app.selected, 2);
    assert_eq!(
        app.preview_state.token, token_before,
        "second rapid move should not immediately refresh preview"
    );
    assert!(
        app.preview_state.deferred_refresh_at.is_some(),
        "second rapid move should schedule a deferred refresh"
    );

    // After the deferred delay the preview fires.
    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    assert!(
        app.preview_state.token > token_before,
        "deferred preview should fire after pause"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_alt_right_does_not_trigger_history_navigation() {
    let root = temp_path("high-frequency-alt-right-no-history");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);
    app.go_back().expect("go back should succeed");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT)))
        .expect("alt-right should be handled");

    assert_eq!(app.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(child.as_path())
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
