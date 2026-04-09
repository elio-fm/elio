use super::super::*;
use super::helpers::temp_path;
use std::{
    ffi::OsString,
    fs,
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};

fn terminal_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct TerminalEnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl TerminalEnvGuard {
    fn isolate() -> Self {
        const VARS: &[&str] = &[
            "TERM",
            "TERM_PROGRAM",
            "KITTY_WINDOW_ID",
            "WARP_SESSION_ID",
            "ALACRITTY_SOCKET",
            "WT_SESSION",
        ];

        let saved = VARS
            .iter()
            .map(|&var| (var, std::env::var_os(var)))
            .collect::<Vec<_>>();
        unsafe {
            for &var in VARS {
                std::env::remove_var(var);
            }
        }
        Self { saved }
    }
}

impl Drop for TerminalEnvGuard {
    fn drop(&mut self) {
        unsafe {
            for (var, value) in self.saved.drain(..) {
                match value {
                    Some(value) => std::env::set_var(var, value),
                    None => std::env::remove_var(var),
                }
            }
        }
    }
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
fn foot_sixel_browser_wheel_defers_preview_refresh() {
    let _lock = terminal_env_lock();
    let _guard = TerminalEnvGuard::isolate();
    unsafe {
        std::env::set_var("TERM", "foot");
    }

    let root = temp_path("wheel-foot-sixel-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.jpg", "b.jpg", "c.jpg"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::Default;
    app.enable_terminal_image_previews();
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
    let _lock = terminal_env_lock();
    let _guard = TerminalEnvGuard::isolate();
    unsafe {
        std::env::set_var("WT_SESSION", "00000000-0000-0000-0000-000000000001");
    }

    let root = temp_path("wheel-wt-sixel-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.jpg", "b.jpg", "c.jpg"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::Default;
    app.enable_terminal_image_previews();
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
