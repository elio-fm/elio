use super::super::super::*;

#[test]
fn keys_accept_modifier_bindings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
open = "ctrl+o"
open_with = "alt+o"
open_or_enter = ["enter", "ctrl+enter"]
nav_right = "shift+home"
nav_left = "ctrl+alt+up"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.open.to_string(), "Ctrl+O");
    assert_eq!(config.keys.open_with.to_string(), "Alt+O");
    assert_eq!(config.keys.open_or_enter.to_string(), "Enter/Ctrl+Enter");
    assert_eq!(config.keys.nav_right.to_string(), "Shift+Home");
    assert_eq!(config.keys.nav_left.to_string(), "Ctrl+Alt+↑");
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::ALT)),
        Some(Action::OpenWith)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL)),
        Some(Action::OpenOrEnter)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Home, KeyModifiers::SHIFT)),
        Some(Action::NavRight)
    );
    assert_eq!(
        config.keys.action_for_key(KeyEvent::new(
            KeyCode::Up,
            KeyModifiers::CONTROL | KeyModifiers::ALT
        )),
        Some(Action::NavLeft)
    );
}

#[test]
fn caps_lock_character_events_match_uppercase_bindings() {
    use crossterm::event::KeyModifiers;

    let key_bindings = KeyBindings::default();

    assert_eq!(
        key_bindings.action_for_key(caps_lock_char('o', KeyModifiers::NONE)),
        Some(Action::OpenWith)
    );
}

#[test]
fn caps_lock_with_shift_character_events_match_lowercase_bindings() {
    use crossterm::event::KeyModifiers;

    let key_bindings = KeyBindings::default();

    assert_eq!(
        key_bindings.action_for_key(caps_lock_char('o', KeyModifiers::SHIFT)),
        Some(Action::Open)
    );
    assert_eq!(
        key_bindings.action_for_key(caps_lock_char('O', KeyModifiers::SHIFT)),
        Some(Action::Open)
    );
}

#[test]
fn caps_lock_character_events_use_unicode_case_mapping() {
    use crossterm::event::KeyModifiers;

    let config = Config::from_str(
        r#"
[keys]
open = "ñ"
open_with = "Ñ"
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config
            .keys
            .action_for_key(caps_lock_char('ñ', KeyModifiers::NONE)),
        Some(Action::OpenWith)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(caps_lock_char('ñ', KeyModifiers::SHIFT)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(caps_lock_char('Ñ', KeyModifiers::SHIFT)),
        Some(Action::Open)
    );
}

#[test]
fn caps_lock_does_not_change_ctrl_alt_character_matching() {
    use crossterm::event::KeyModifiers;

    let config = Config::from_str(
        r#"
[keys]
open = "ctrl+o"
open_with = "alt+o"
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config
            .keys
            .action_for_key(caps_lock_char('o', KeyModifiers::CONTROL)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(caps_lock_char('o', KeyModifiers::ALT)),
        Some(Action::OpenWith)
    );
}

fn caps_lock_char(
    c: char,
    modifiers: crossterm::event::KeyModifiers,
) -> crossterm::event::KeyEvent {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState};

    KeyEvent::new_with_kind_and_state(
        KeyCode::Char(c),
        modifiers,
        KeyEventKind::Press,
        KeyEventState::CAPS_LOCK,
    )
}

#[test]
fn delete_named_key_supports_plain_and_shift_bindings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
trash = ["d", "del"]
delete_permanently = ["D", "shift+delete"]
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.trash.to_string(), "d/Del");
    assert_eq!(config.keys.delete_permanently.to_string(), "D/Shift+Del");
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)),
        Some(Action::Trash)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::SHIFT)),
        Some(Action::DeletePermanently)
    );
}

#[test]
fn keys_match_modifiers_exactly() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
nav_right = "right"
scroll_preview_right = []
open = "shift+right"
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        Some(Action::NavRight)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
        None
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SUPER)),
        None
    );
}

#[test]
fn modified_and_plain_bindings_do_not_collide() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
extract_archive = []
nav_right = []
scroll_preview_right = []
open = "e"
search_folders = "ctrl+e"
open_or_enter = "right"
nav_left = "shift+right"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.action_for('e'), Some(Action::Open));
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)),
        Some(Action::SearchFolders)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        Some(Action::OpenOrEnter)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)),
        Some(Action::NavLeft)
    );
}

#[test]
fn modified_bindings_detect_collisions() {
    let config = Config::from_str(
        r#"
[keys]
open = "ctrl+e"
search_folders = "ctrl+e"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.open.to_string(), "Ctrl+E");
    assert_eq!(config.keys.search_folders.to_string(), "f");
}

#[test]
fn shifted_char_events_match_uppercase_char_bindings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let key_bindings = KeyBindings::default();
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT)),
        Some(Action::OpenWith)
    );
}

#[test]
fn keys_reject_invalid_modifier_bindings_and_use_default() {
    let cases = [
        "open = \"ctrl+\"",
        "open = \"ctrl++f\"",
        "open = \"ctrl+ctrl+f\"",
        "open = \"cmd+f\"",
        "open = \"ctrl+spacebar\"",
        "open = \"shift+space\"",
        "open = \"ctrl+f\"",
        "open = \"ctrl+c\"",
        "open = \"ctrl+a\"",
        "open = \"ctrl+=\"",
        "open = \"ctrl+-\"",
        "open = \"alt+right\"",
        "open = \"alt+left\"",
    ];

    for override_toml in cases {
        let config =
            Config::from_str(&format!("[keys]\n{override_toml}")).expect("config should parse");
        assert_eq!(config.keys.open.to_string(), "o");
    }

    let config = Config::from_str("[keys]\nopen_with = \"shift+o\"").expect("config should parse");
    assert_eq!(config.keys.open_with.to_string(), "O");
}

#[test]
fn modified_bindings_accept_case_and_order_variants_safely() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
open = "CTRL+O"
open_with = "alt+ctrl+e"
"#,
    )
    .expect("config should parse");

    // Modifier names are case-insensitive, while the final character key keeps its case.
    assert_eq!(config.keys.open.to_string(), "Ctrl+O");
    assert_eq!(config.keys.open_with.to_string(), "Ctrl+Alt+E");
    assert_eq!(
        config.keys.action_for_key(KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL | KeyModifiers::ALT
        )),
        Some(Action::OpenWith)
    );
    assert_eq!(
        config.keys.action_for_key(KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER
        )),
        None,
        "extra terminal modifiers must not accidentally match"
    );
}

#[test]
fn ctrl_alt_character_bindings_match_shifted_terminal_events_without_leaking_plain() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
open = "ctrl+o"
open_with = "alt+o"
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.keys.action_for_key(KeyEvent::new(
            KeyCode::Char('O'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )),
        Some(Action::Open),
        "terminals may report Ctrl+Shift+letter as an uppercase char plus Shift"
    );
    assert_eq!(
        config.keys.action_for_key(KeyEvent::new(
            KeyCode::Char('O'),
            KeyModifiers::ALT | KeyModifiers::SHIFT
        )),
        Some(Action::OpenWith)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE)),
        None,
        "plain o must stay unbound after open moves to Ctrl+O"
    );
}
