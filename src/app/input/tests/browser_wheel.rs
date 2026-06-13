use super::super::*;
use super::helpers::temp_path;
use std::{
    fs, thread,
    time::{Duration, Instant},
};

fn wait_for_selected_directory_count(app: &mut App) {
    for _ in 0..100 {
        let _ = app.process_background_jobs();
        if app
            .selected_entry()
            .is_some_and(|entry| app.directory_item_count_label(entry).is_some())
        {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for selected directory count");
}

#[test]
fn wheel_burst_smoothing_coalesces_dense_input() {
    let mut lane = ScrollLane::new();

    for _ in 0..6 {
        App::queue_scroll(&mut lane, 1, ENTRY_WHEEL_TUNING);
    }

    assert!(lane.pending.abs() < 6);
    assert!(lane.pending > 0);
}

#[test]
fn short_entry_wheel_burst_keeps_full_distance() {
    let mut lane = ScrollLane::new();

    for _ in 0..3 {
        App::queue_scroll(&mut lane, 1, ENTRY_WHEEL_TUNING);
    }

    assert_eq!(lane.pending, 3);
}

#[test]
fn browser_wheel_updates_selection_and_preview_immediately() {
    let root = temp_path("wheel-selection-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::Default;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });
    let initial_preview_token = app.preview.state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");
    assert!(app.process_pending_scroll());

    assert_eq!(app.navigation.selected, 1);
    assert_eq!(app.navigation.scroll_row, 1);
    assert!(app.preview.state.token > initial_preview_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_browser_wheel_moves_selection_immediately() {
    let root = temp_path("wheel-high-frequency-immediate");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });
    let initial_preview_token = app.preview.state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");

    assert_eq!(app.navigation.selected, 1);
    assert_eq!(app.navigation.scroll_row, 1);
    assert!(app.preview.state.token > initial_preview_token);
    assert!(!app.has_pending_scroll());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_browser_wheel_keeps_large_flick_distance() {
    let root = temp_path("wheel-high-frequency-distance");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..12 {
        fs::write(root.join(format!("{index}.txt")), format!("{index}"))
            .expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });

    for _ in 0..4 {
        app.handle_event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        }))
        .expect("scroll down should be handled");
    }

    assert_eq!(app.navigation.selected, 4);
    assert_eq!(app.navigation.scroll_row, 4);
    assert!(!app.has_pending_scroll());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_browser_wheel_defers_preview_refresh_during_burst() {
    let root = temp_path("wheel-high-frequency-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });

    let initial_token = app.preview.state.token;
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("first scroll down should be handled");
    let after_first_token = app.preview.state.token;
    assert!(after_first_token > initial_token);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("second scroll down should be handled");

    assert_eq!(app.navigation.selected, 2);
    assert_eq!(app.preview.state.token, after_first_token);

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    assert!(app.preview.state.token > after_first_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_browser_wheel_requests_post_burst_redraw() {
    let root = temp_path("wheel-high-frequency-post-burst-redraw");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");

    assert!(app.input.browser_wheel_post_burst_pending);
    assert!(!app.process_browser_wheel_timers());

    thread::sleep(WHEEL_SCROLL_BURST_WINDOW + Duration::from_millis(20));
    assert!(app.process_browser_wheel_timers());
    assert!(!app.input.browser_wheel_post_burst_pending);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn browser_wheel_preserves_preview_when_selection_does_not_change() {
    let root = temp_path("wheel-selection-clamp");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 2,
        },
        ..FrameState::default()
    });
    app.select_index(0);
    let initial_preview_token = app.preview.state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll up should be handled");
    assert!(!app.process_pending_scroll());

    assert_eq!(app.navigation.scroll_row, 0);
    assert_eq!(app.navigation.selected, 0);
    assert_eq!(app.preview.state.token, initial_preview_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_browser_wheel_keeps_visible_directory_counts_live_during_burst() {
    let root = temp_path("wheel-directory-count-live");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..3 {
        let dir = root.join(format!("dir-{index}"));
        fs::create_dir_all(&dir).expect("failed to create child directory");
        fs::write(dir.join("child.txt"), "child").expect("failed to write child file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.select_index(0);
    let frame_state = FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    };
    app.set_frame_state(frame_state.clone());

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");
    assert!(app.browser_wheel_burst_active());
    app.set_frame_state(frame_state);

    thread::sleep(DIRECTORY_ITEM_COUNT_IDLE_DELAY + Duration::from_millis(10));
    assert!(app.browser_wheel_burst_active());
    let _ = app.process_directory_item_count_timer();
    wait_for_selected_directory_count(&mut app);

    let selected = app.selected_entry().expect("selection should exist");
    assert_eq!(selected.name, "dir-1");
    assert_eq!(
        app.directory_item_count_label(selected).as_deref(),
        Some("1 item")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn directory_count_timer_uses_latest_viewport_without_debouncing_every_scroll_step() {
    let root = temp_path("directory-count-throttle-latest");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for index in 0..3 {
        let dir = root.join(format!("dir-{index}"));
        fs::create_dir_all(&dir).expect("failed to create child directory");
        fs::write(dir.join("child.txt"), "child").expect("failed to write child file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    let frame_state = FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    };
    app.select_index(0);
    app.set_frame_state(frame_state.clone());

    thread::sleep(DIRECTORY_ITEM_COUNT_IDLE_DELAY / 2);
    app.select_index(1);
    app.set_frame_state(frame_state);
    thread::sleep(DIRECTORY_ITEM_COUNT_IDLE_DELAY / 2 + Duration::from_millis(10));

    let _ = app.process_directory_item_count_timer();
    wait_for_selected_directory_count(&mut app);

    let selected = app.selected_entry().expect("selection should exist");
    assert_eq!(selected.name, "dir-1");
    assert_eq!(
        app.directory_item_count_label(selected).as_deref(),
        Some("1 item")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn directory_count_timer_is_not_blocked_by_deferred_preview_refresh() {
    let root = temp_path("directory-count-preview-deferred");
    let dir = root.join("dir");
    fs::create_dir_all(&dir).expect("failed to create child directory");
    fs::write(dir.join("child.txt"), "child").expect("failed to write child file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });
    app.preview.state.deferred_refresh_at =
        Some(Instant::now() + HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY);

    thread::sleep(DIRECTORY_ITEM_COUNT_IDLE_DELAY + Duration::from_millis(10));
    let _ = app.process_directory_item_count_timer();
    wait_for_selected_directory_count(&mut app);

    let selected = app.selected_entry().expect("selection should exist");
    assert_eq!(
        app.directory_item_count_label(selected).as_deref(),
        Some("1 item")
    );
    assert!(app.preview.state.deferred_refresh_at.is_some());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn foot_sixel_browser_wheel_defers_preview_refresh() {
    let root = temp_path("wheel-foot-sixel-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.jpg", "b.jpg", "c.jpg"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::Default;
    app.set_terminal_image_protocol_for_tests(
        crate::app::overlays::inline_image::ImageProtocol::Sixel,
        crate::app::overlays::inline_image::TerminalIdentity::Foot,
    );
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_content_area: Some(Rect {
            x: 20,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });
    let initial_preview_token = app.preview.state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");
    assert!(app.process_pending_scroll());

    assert_eq!(app.navigation.selected, 1);
    assert_eq!(app.preview.state.token, initial_preview_token);
    assert!(app.preview.state.deferred_refresh_at.is_some());

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    assert!(app.preview.state.token > initial_preview_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn windows_terminal_sixel_browser_wheel_defers_preview_refresh() {
    let root = temp_path("wheel-wt-sixel-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.jpg", "b.jpg", "c.jpg"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::Default;
    app.set_terminal_image_protocol_for_tests(
        crate::app::overlays::inline_image::ImageProtocol::Sixel,
        crate::app::overlays::inline_image::TerminalIdentity::WindowsTerminal,
    );
    app.select_index(0);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_content_area: Some(Rect {
            x: 20,
            y: 0,
            width: 20,
            height: 8,
        }),
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 1,
        },
        ..FrameState::default()
    });
    let initial_preview_token = app.preview.state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");
    assert!(app.process_pending_scroll());

    assert_eq!(app.navigation.selected, 1);
    assert_eq!(app.preview.state.token, initial_preview_token);
    assert!(app.preview.state.deferred_refresh_at.is_some());

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    assert!(app.preview.state.token > initial_preview_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
