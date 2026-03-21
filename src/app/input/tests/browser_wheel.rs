use super::super::*;
use super::helpers::temp_path;
use std::{fs, thread, time::Duration};

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
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::Default;
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
    let initial_preview_token = app.preview_state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");
    assert!(app.process_pending_scroll());

    assert_eq!(app.selected, 1);
    assert_eq!(app.scroll_row, 1);
    assert!(app.preview_state.token > initial_preview_token);

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
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
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
    let initial_preview_token = app.preview_state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll down should be handled");

    assert_eq!(app.selected, 1);
    assert_eq!(app.scroll_row, 1);
    assert!(app.preview_state.token > initial_preview_token);
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
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
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

    assert_eq!(app.selected, 4);
    assert_eq!(app.scroll_row, 4);
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
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
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

    let initial_token = app.preview_state.token;
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("first scroll down should be handled");
    let after_first_token = app.preview_state.token;
    assert!(after_first_token > initial_token);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("second scroll down should be handled");

    assert_eq!(app.selected, 2);
    assert_eq!(app.preview_state.token, after_first_token);

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    assert!(app.preview_state.token > after_first_token);

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
    app.view_mode = ViewMode::List;
    app.wheel_profile = WheelProfile::HighFrequency;
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

    assert!(app.browser_wheel_post_burst_pending);
    assert!(!app.process_browser_wheel_timers());

    thread::sleep(WHEEL_SCROLL_BURST_WINDOW + Duration::from_millis(20));
    assert!(app.process_browser_wheel_timers());
    assert!(!app.browser_wheel_post_burst_pending);

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
    app.view_mode = ViewMode::List;
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
    let initial_preview_token = app.preview_state.token;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll up should be handled");
    assert!(!app.process_pending_scroll());

    assert_eq!(app.scroll_row, 0);
    assert_eq!(app.selected, 0);
    assert_eq!(app.preview_state.token, initial_preview_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
