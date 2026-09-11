use super::super::super::*;

#[test]
fn remaining_browser_shortcuts_can_be_overridden_and_freed() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
search_files = "ctrl+s"
select_all = "A"
history_back = "alt+h"
history_forward = "alt+l"
open = ["o", "ctrl+f", "ctrl+a", "alt+left", "alt+right"]
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
        Some(Action::SearchFiles)
    );
    assert_eq!(config.keys.action_for('A'), Some(Action::SelectAll));
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT)),
        Some(Action::HistoryBack)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT)),
        Some(Action::HistoryForward)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT)),
        Some(Action::Open)
    );
}

#[test]
fn browser_control_defaults_are_configurable_actions() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.action_for('g'), Some(Action::GoTo));
    assert_eq!(key_bindings.action_for('G'), Some(Action::JumpLast));
    assert_eq!(key_bindings.action_for(' '), Some(Action::ToggleSelection));
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        Some(Action::CyclePlacesNext)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        Some(Action::CyclePlacesPrevious)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        Some(Action::CyclePlacesPrevious)
    );

    let shift_tab = Config::from_str("[keys]\ncycle_places_previous = \"shift+tab\"")
        .expect("config should parse");
    assert_eq!(
        shift_tab
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        Some(Action::CyclePlacesPrevious)
    );
    assert_eq!(
        shift_tab
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        Some(Action::CyclePlacesPrevious)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
        Some(Action::GoParent)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
        Some(Action::PageUp)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
        Some(Action::PageDown)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
        Some(Action::JumpFirst)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
        Some(Action::JumpLast)
    );
}

#[test]
fn browser_control_defaults_can_be_freed_for_other_actions() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
go_to = []
toggle_selection = []
cycle_places_next = []
cycle_places_previous = []
go_parent = []
page_up = []
page_down = []
jump_first = []
jump_last = []
open = ["o", "g", "G", "space", "tab", "backtab", "backspace", "pageup", "pagedown", "home", "end"]
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.action_for('g'), Some(Action::Open));
    assert_eq!(config.keys.action_for('G'), Some(Action::Open));
    assert_eq!(config.keys.action_for(' '), Some(Action::Open));
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        Some(Action::Open)
    );

    let nav_right = Config::from_str(
        r#"
[keys]
cycle_places_previous = []
nav_right = "backtab"
"#,
    )
    .expect("config should parse");
    assert_eq!(
        nav_right
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        Some(Action::NavRight)
    );
    assert_eq!(
        nav_right
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        Some(Action::NavRight)
    );

    let shift_tab_nav_right = Config::from_str(
        r#"
[keys]
cycle_places_previous = []
nav_right = "shift+tab"
"#,
    )
    .expect("config should parse");
    assert_eq!(
        shift_tab_nav_right
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        Some(Action::NavRight)
    );
    assert_eq!(
        shift_tab_nav_right
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        Some(Action::NavRight)
    );

    let default_keys = KeyBindings::default();
    assert_eq!(
        default_keys.action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        Some(Action::CyclePlacesPrevious)
    );
    assert_eq!(
        default_keys.action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        Some(Action::CyclePlacesPrevious)
    );

    let literal_space = Config::from_str(
        r#"
[keys]
toggle_selection = []
open = " "
"#,
    )
    .expect("config should parse");
    assert_eq!(literal_space.keys.action_for(' '), Some(Action::Open));
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
        Some(Action::Open)
    );
}
