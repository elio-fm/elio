use super::{
    ACTIVE_SCROLL_POLL_INTERVAL, EditorTempCleanup, IDLE_POLL_INTERVAL,
    event_implies_terminal_focus, event_poll_interval, keyboard_enhancement_is_unsupported,
};
use crossterm::event::Event;
use std::{
    fs, io,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn event_poll_interval_stays_idle_while_terminal_is_unfocused() {
    let interval = event_poll_interval(
        IDLE_POLL_INTERVAL,
        false,
        [
            Some(Duration::from_millis(25)),
            Some(Duration::from_millis(10)),
        ],
    );

    assert_eq!(interval, IDLE_POLL_INTERVAL);
}

#[test]
fn event_poll_interval_uses_pending_timer_when_terminal_is_focused() {
    let delay = Duration::from_millis(25);
    let interval = event_poll_interval(
        ACTIVE_SCROLL_POLL_INTERVAL,
        true,
        [None, Some(delay), Some(Duration::from_millis(50))],
    );

    assert!(interval <= delay);
}

#[test]
fn input_events_imply_terminal_focus() {
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };

    assert!(event_implies_terminal_focus(&Event::Key(KeyEvent::new(
        KeyCode::Char('j'),
        KeyModifiers::NONE
    ))));
    assert!(event_implies_terminal_focus(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    })));
    assert!(event_implies_terminal_focus(&Event::Paste(
        "text".to_string()
    )));
}

#[test]
fn focus_and_resize_events_do_not_imply_terminal_focus() {
    assert!(!event_implies_terminal_focus(&Event::FocusLost));
    assert!(!event_implies_terminal_focus(&Event::FocusGained));
    assert!(!event_implies_terminal_focus(&Event::Resize(80, 24)));
}

#[test]
fn editor_temp_cleanup_removes_document_on_drop() {
    let path = std::env::temp_dir().join(format!(
        "elio-editor-cleanup-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::write(&path, "alpha.txt\n").expect("failed to create editor temp document");
    {
        let _cleanup = EditorTempCleanup(path.clone());
    }
    assert!(!path.exists());
}

#[test]
fn keyboard_enhancement_unsupported_detection_matches_crossterm_error() {
    let error = io::Error::new(
        io::ErrorKind::Unsupported,
        "Keyboard progressive enhancement not implemented for the legacy Windows API.",
    );

    assert!(keyboard_enhancement_is_unsupported(&error));
}

#[test]
fn keyboard_enhancement_unsupported_detection_rejects_other_errors() {
    let error = io::Error::other("some other terminal error");

    assert!(!keyboard_enhancement_is_unsupported(&error));
}
