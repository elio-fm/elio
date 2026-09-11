use super::super::super::*;

#[test]
fn parser_edge_cases_fall_back_without_panicking() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let suspicious_values = [
        "",
        "+",
        "++",
        "ctrl+",
        "+ctrl+o",
        "ctrl++o",
        "ctrl+alt+",
        "ctrl+alt+shift",
        "ctrl+shift+o",
        "shift++",
        "super+o",
        "cmd+o",
        "ctrl+spacebar",
        "shift+space",
        "right+ctrl",
        "ctrl+alt+right+extra",
        "enter+ctrl",
        "\t",
        "\n",
        "🔥🔥",
    ];

    for value in suspicious_values {
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\t', "\\t")
            .replace('\n', "\\n");
        let config = Config::from_str(&format!("[keys]\nopen = \"{escaped}\""))
            .expect("config should parse or safely fall back");
        assert!(
            config.keys.open.to_string() == "o"
                || config.keys.action_for_key(KeyEvent::new(
                    KeyCode::Right,
                    KeyModifiers::CONTROL | KeyModifiers::ALT,
                )) == Some(Action::Open),
            "unexpected parse result for {value:?}: {}",
            config.keys.open
        );
    }
}

#[test]
fn literal_space_collides_with_space_named_key() {
    let config = Config::from_str(
        r#"
[keys]
open = " "
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for(' '), Some(Action::ToggleSelection));
    assert_eq!(config.keys.action_for('o'), Some(Action::Open));
}

#[test]
fn keys_rejects_multi_char_string_and_uses_default() {
    let config = Config::from_str(
        r#"
[keys]
yank = "yy"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.yank, 'y');
}

#[test]
fn keys_rejects_empty_string_and_uses_default() {
    let config = Config::from_str(
        r#"
[keys]
yank = ""
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.yank, 'y');
}

#[test]
fn keys_rejects_reserved_char_and_uses_default() {
    let config = Config::from_str(
        r#"
[keys]
yank = "?"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.yank, 'y');
}

#[test]
fn keys_rejects_control_characters_and_uses_default() {
    let config = Config::from_str("[keys]\nquit = \"\\t\"").expect("config should parse");
    assert_eq!(config.keys.quit, 'q');

    let config = Config::from_str("[keys]\nquit = \"\\n\"").expect("config should parse");
    assert_eq!(config.keys.quit, 'q');
}

#[test]
fn keys_rejects_user_user_duplicate_and_uses_defaults() {
    let config = Config::from_str(
        r#"
[keys]
yank = "p"
paste = "p"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.yank, 'y');
    assert_eq!(config.keys.paste, 'p');
}

#[test]
fn keys_rejects_user_default_collision_and_uses_default() {
    let config = Config::from_str(
        r#"
[keys]
yank = "d"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.yank, 'y');
    assert_eq!(config.keys.trash.to_string(), "d/Del");
}

#[test]
fn keys_allows_swapping_two_defaults() {
    let config = Config::from_str(
        r#"
[keys]
yank = "x"
cut = "y"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.yank, 'x');
    assert_eq!(config.keys.cut, 'y');
}

#[test]
fn function_keys_can_be_bound_and_displayed() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
open = "F5"
open_with = ["O", "f12"]
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.open.to_string(), "F5");
    assert_eq!(config.keys.open_with.to_string(), "O/F12");
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE)),
        Some(Action::OpenWith)
    );
}

#[test]
fn named_keys_and_modifiers_are_case_insensitive() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
page_up = "PageUp"
cycle_places_previous = "Shift+Tab"
open = "Alt+Up"
open_with = "O"
open_or_enter = "SHIft+Enter"
rename = ["r", "F2"]
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.page_up.to_string(), "PageUp");
    assert_eq!(config.keys.cycle_places_previous.to_string(), "Shift+Tab");
    assert_eq!(config.keys.open.to_string(), "Alt+↑");
    assert_eq!(config.keys.open_with.to_string(), "O");
    assert_eq!(config.keys.open_or_enter.to_string(), "Shift+Enter");
    assert_eq!(config.keys.rename.to_string(), "r/F2");
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
        Some(Action::PageUp)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)),
        Some(Action::CyclePlacesPrevious)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT)),
        Some(Action::Open)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT)),
        Some(Action::OpenWith)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
        Some(Action::OpenOrEnter)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)),
        Some(Action::Rename)
    );
}

#[test]
fn shift_character_bindings_keep_using_uppercase_character_form() {
    let cases = ["Shift+O", "Shift+o", "SHIft+O", "SHIft+o"];

    for value in cases {
        let config =
            Config::from_str(&format!("[keys]\nopen = \"{value}\"")).expect("config should parse");

        assert_eq!(
            config.keys.open.to_string(),
            "o",
            "{value:?} should fall back; shifted characters are written as uppercase chars"
        );
    }

    let config = Config::from_str("[keys]\nopen_with = \"O\"").expect("config should parse");
    assert_eq!(config.keys.open_with.to_string(), "O");
}

#[test]
fn shift_backtab_bindings_fall_back_because_backtab_already_means_shift_tab() {
    let cases = ["Shift+BackTab", "shift+backtab", "SHIft+BackTab"];

    for value in cases {
        let config = Config::from_str(&format!("[keys]\ncycle_places_previous = \"{value}\""))
            .expect("config should parse");

        assert_eq!(
            config.keys.cycle_places_previous.to_string(),
            "Shift+Tab",
            "{value:?} should fall back; use shift+tab or backtab instead"
        );
    }
}
