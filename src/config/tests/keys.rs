use super::super::*;

#[test]
fn keys_default_bindings_are_sane() {
    let config = Config::default_config();
    assert_eq!(config.keys.yank, 'y');
    assert_eq!(config.keys.cut, 'x');
    assert_eq!(config.keys.paste, 'p');
    assert_eq!(config.keys.delete_permanently, 'D');
    assert_eq!(config.keys.quit, 'q');
    assert_eq!(config.keys.quit_without_cd, 'Q');
    assert_eq!(config.keys.zoxide, 'z');
    assert_eq!(config.keys.shell, '!');
}

#[test]
fn keys_can_be_overridden() {
    let config = Config::from_str(
        r#"
[keys]
yank = "Y"
cut = "X"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.yank, 'Y');
    assert_eq!(config.keys.cut, 'X');
    assert_eq!(config.keys.paste, 'p');
}

#[test]
fn keys_accept_array_overrides() {
    let config = Config::from_str(
        r#"
[keys]
open = ["o", "e"]
open_with = ["O"]
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('o'), Some(Action::Open));
    assert_eq!(config.keys.action_for('e'), Some(Action::Open));
    assert_eq!(config.keys.action_for('O'), Some(Action::OpenWith));
}

#[test]
fn keys_accepts_empty_array_to_unbind_action() {
    let config = Config::from_str(
        r#"
[keys]
open = []
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.open.to_string(), "");
    assert_eq!(config.keys.action_for('o'), None);
}

#[test]
fn unbound_action_frees_its_default_key_for_another_action() {
    let config = Config::from_str(
        r#"
[keys]
open = []
shell = "o"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('o'), Some(Action::Shell));
    assert_eq!(config.keys.action_for('!'), None);
}

#[test]
fn keys_rejects_duplicate_inside_array_and_uses_default() {
    let config = Config::from_str(
        r#"
[keys]
open = ["e", "e"]
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.open, 'o');
    assert_eq!(config.keys.action_for('e'), None);
}

#[test]
fn keys_rejects_invalid_array_member_and_uses_default() {
    let config = Config::from_str(
        r#"
[keys]
open = ["e", "space"]
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.open, 'o');
    assert_eq!(config.keys.action_for('e'), None);
}

#[test]
fn keys_rejects_array_collision_with_other_binding() {
    let config = Config::from_str(
        r#"
[keys]
open = ["o", "p"]
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.open, 'o');
    assert_eq!(config.keys.paste, 'p');
    assert_eq!(config.keys.action_for('p'), Some(Action::Paste));
}

#[test]
fn key_display_joins_multiple_bindings() {
    let config = Config::from_str(
        r#"
[keys]
open = ["o", "e"]
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.open.to_string(), "o/e");
}

#[test]
fn keys_accept_modifier_bindings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
open = "ctrl+o"
open_with = "alt+o"
open_or_enter = ["enter", "ctrl+enter"]
nav_right = "shift+right"
nav_left = "ctrl+alt+up"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.open.to_string(), "Ctrl+O");
    assert_eq!(config.keys.open_with.to_string(), "Alt+O");
    assert_eq!(config.keys.open_or_enter.to_string(), "Enter/Ctrl+Enter");
    assert_eq!(config.keys.nav_right.to_string(), "Shift+→");
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
            .action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)),
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
fn keys_match_modifiers_exactly() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
nav_right = "right"
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
nav_right = []
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

    // Modifiers are intentionally lower-case only; uppercase aliases fall back
    // instead of being accepted ambiguously.
    assert_eq!(config.keys.open.to_string(), "o");
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
    assert_eq!(config.keys.trash, 'd');
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
fn action_for_returns_correct_action_for_default_bindings() {
    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.action_for('y'), Some(Action::Yank));
    assert_eq!(key_bindings.action_for('x'), Some(Action::Cut));
    assert_eq!(key_bindings.action_for('p'), Some(Action::Paste));
    assert_eq!(
        key_bindings.action_for('D'),
        Some(Action::DeletePermanently)
    );
    assert_eq!(key_bindings.action_for('q'), Some(Action::Quit));
    assert_eq!(key_bindings.action_for('Q'), Some(Action::QuitWithoutCd));
    assert_eq!(key_bindings.action_for('o'), Some(Action::Open));
    assert_eq!(key_bindings.action_for('O'), Some(Action::OpenWith));
    assert_eq!(
        key_bindings.action_for_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )),
        Some(Action::OpenOrEnter)
    );
    assert_eq!(key_bindings.action_for('z'), Some(Action::Zoxide));
    assert_eq!(key_bindings.action_for('!'), Some(Action::Shell));
    assert_eq!(key_bindings.action_for('h'), Some(Action::NavLeft));
    assert_eq!(key_bindings.action_for('j'), Some(Action::NavDown));
    assert_eq!(key_bindings.action_for('k'), Some(Action::NavUp));
    assert_eq!(key_bindings.action_for('l'), Some(Action::NavRight));
}

#[test]
fn nav_defaults_include_vim_keys_and_arrow_keys() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.nav_left.to_string(), "h/←");
    assert_eq!(key_bindings.nav_down.to_string(), "j/↓");
    assert_eq!(key_bindings.nav_up.to_string(), "k/↑");
    assert_eq!(key_bindings.nav_right.to_string(), "l/→");
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        Some(Action::NavLeft)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        Some(Action::NavDown)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
        Some(Action::NavUp)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        Some(Action::NavRight)
    );
}

#[test]
fn nav_keys_can_be_overridden_with_chars_and_arrows() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
nav_down = ["n", "down"]
nav_up = "u"
nav_right = "b"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('n'), Some(Action::NavDown));
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        Some(Action::NavDown)
    );
    assert_eq!(config.keys.action_for('u'), Some(Action::NavUp));
    assert_eq!(config.keys.action_for('b'), Some(Action::NavRight));
    assert_eq!(config.keys.action_for('j'), None);
    assert_eq!(config.keys.action_for('k'), None);
    assert_eq!(config.keys.action_for('l'), None);
}

#[test]
fn overriding_nav_right_frees_l_for_another_action() {
    let config = Config::from_str(
        r#"
[keys]
nav_right = "right"
open = ["o", "l"]
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('l'), Some(Action::Open));
    assert_eq!(config.keys.action_for('o'), Some(Action::Open));
}

#[test]
fn keys_rejects_collision_with_default_nav_key() {
    let config = Config::from_str(
        r#"
[keys]
open = "l"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('l'), Some(Action::NavRight));
    assert_eq!(config.keys.action_for('o'), Some(Action::Open));
}

#[test]
fn keys_rejects_collision_with_default_arrow_key() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
open = "right"
"#,
    )
    .expect("config should parse");
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        Some(Action::NavRight)
    );
    assert_eq!(config.keys.action_for('o'), Some(Action::Open));
}

#[test]
fn action_for_reflects_overridden_binding() {
    let config = Config::from_str(
        r#"
[keys]
yank = "Y"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('Y'), Some(Action::Yank));
    assert_eq!(config.keys.action_for('y'), None);
}

#[test]
fn delete_permanently_can_be_overridden() {
    let config = Config::from_str(
        r#"
[keys]
delete_permanently = "X"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('X'), Some(Action::DeletePermanently));
    assert_eq!(config.keys.action_for('D'), None);
}

#[test]
fn quit_without_cd_can_be_overridden() {
    let config = Config::from_str(
        r#"
[keys]
quit_without_cd = "u"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('u'), Some(Action::QuitWithoutCd));
    assert_eq!(config.keys.action_for('Q'), None);
}

#[test]
fn open_with_defaults_to_capital_o() {
    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.open_with, 'O');
    assert_eq!(key_bindings.action_for('O'), Some(Action::OpenWith));
}

#[test]
fn open_with_can_be_overridden() {
    let config = Config::from_str(
        r#"
[keys]
open_with = "w"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('w'), Some(Action::OpenWith));
    assert_eq!(config.keys.action_for('O'), None);
}

#[test]
fn open_or_enter_defaults_to_enter() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.open_or_enter.to_string(), "Enter");
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(Action::OpenOrEnter)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Char('\n'), KeyModifiers::NONE)),
        Some(Action::OpenOrEnter)
    );
}

#[test]
fn open_or_enter_can_add_l_after_nav_right_frees_it() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
nav_right = "right"
open_or_enter = ["enter", "l"]
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('l'), Some(Action::OpenOrEnter));
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(Action::OpenOrEnter)
    );
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        Some(Action::NavRight)
    );
}

#[test]
fn open_or_enter_rejects_l_while_nav_right_owns_it() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
open_or_enter = ["enter", "l"]
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('l'), Some(Action::NavRight));
    assert_eq!(config.keys.open_or_enter.to_string(), "Enter");
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(Action::OpenOrEnter)
    );
}

#[test]
fn enter_can_move_to_another_action_when_open_or_enter_frees_it() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
open_or_enter = "b"
open = "enter"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('b'), Some(Action::OpenOrEnter));
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(Action::Open)
    );
}

#[test]
fn zoxide_can_be_overridden() {
    let config = Config::from_str(
        r#"
[keys]
zoxide = "Z"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('Z'), Some(Action::Zoxide));
    assert_eq!(config.keys.action_for('z'), None);
}

#[test]
fn shell_defaults_to_bang() {
    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.shell, '!');
    assert_eq!(key_bindings.action_for('!'), Some(Action::Shell));
}

#[test]
fn shell_can_be_overridden() {
    let config = Config::from_str(
        r#"
[keys]
shell = "S"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.action_for('S'), Some(Action::Shell));
    assert_eq!(config.keys.action_for('!'), None);
}

#[test]
fn preview_scroll_defaults_map_to_shift_h_j_k_l() {
    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.scroll_preview_up, 'K');
    assert_eq!(key_bindings.scroll_preview_down, 'J');
    assert_eq!(key_bindings.scroll_preview_left, 'H');
    assert_eq!(key_bindings.scroll_preview_right, 'L');
    assert_eq!(key_bindings.action_for('K'), Some(Action::ScrollPreviewUp));
    assert_eq!(
        key_bindings.action_for('J'),
        Some(Action::ScrollPreviewDown)
    );
    assert_eq!(
        key_bindings.action_for('H'),
        Some(Action::ScrollPreviewLeft)
    );
    assert_eq!(
        key_bindings.action_for('L'),
        Some(Action::ScrollPreviewRight)
    );
}

#[test]
fn scroll_preview_up_can_be_overridden() {
    let config = Config::from_str(
        r#"
[keys]
scroll_preview_up = "U"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.scroll_preview_up, 'U');
    assert_eq!(config.keys.action_for('U'), Some(Action::ScrollPreviewUp));
    assert_eq!(config.keys.action_for('K'), None);
    assert_eq!(
        config.keys.action_for('J'),
        Some(Action::ScrollPreviewDown),
        "untouched bindings should keep their defaults"
    );
}

#[test]
fn scroll_preview_keys_reject_collision_with_other_default() {
    let config = Config::from_str(
        r#"
[keys]
scroll_preview_up = "y"
"#,
    )
    .expect("config should parse");
    assert_eq!(
        config.keys.scroll_preview_up, 'K',
        "user override colliding with default 'y' (yank) must fall back to default 'K'"
    );
    assert_eq!(config.keys.yank, 'y');
    assert_eq!(config.keys.action_for('y'), Some(Action::Yank));
    assert_eq!(
        config.keys.action_for('K'),
        Some(Action::ScrollPreviewUp),
        "default 'K' must remain bound to ScrollPreviewUp"
    );
}

#[test]
fn scroll_preview_keys_reject_user_user_duplicate() {
    // When two user-set bindings collide, the first one in iteration order
    // falls back to its default; the second keeps the user-set value (since
    // the collision is gone after the first reset). This matches the
    // existing yank/paste collision behavior.
    let config = Config::from_str(
        r#"
[keys]
scroll_preview_up   = "U"
scroll_preview_down = "U"
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.scroll_preview_up, 'K');
    assert_eq!(config.keys.scroll_preview_down, 'U');
    assert_eq!(config.keys.action_for('U'), Some(Action::ScrollPreviewDown));
    assert_eq!(config.keys.action_for('K'), Some(Action::ScrollPreviewUp));
    assert_eq!(
        config.keys.action_for('J'),
        None,
        "default 'J' is no longer bound because scroll_preview_down was overridden to 'U'"
    );
}
