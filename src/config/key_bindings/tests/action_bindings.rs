use super::super::super::*;

#[test]
fn action_for_returns_correct_action_for_default_bindings() {
    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.action_for('y'), Some(Action::Yank));
    assert_eq!(key_bindings.action_for('x'), Some(Action::Cut));
    assert_eq!(key_bindings.action_for('p'), Some(Action::Paste));
    assert_eq!(key_bindings.action_for('e'), Some(Action::ExtractArchive));
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
    assert_eq!(key_bindings.action_for('!'), Some(Action::ShellHere));
    assert_eq!(key_bindings.action_for('h'), Some(Action::NavLeft));
    assert_eq!(key_bindings.action_for('j'), Some(Action::NavDown));
    assert_eq!(key_bindings.action_for('k'), Some(Action::NavUp));
    assert_eq!(key_bindings.action_for('l'), Some(Action::NavRight));
    assert_eq!(
        key_bindings.action_for_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('f'),
            crossterm::event::KeyModifiers::CONTROL,
        )),
        Some(Action::SearchFiles)
    );
    assert_eq!(
        key_bindings.action_for_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::CONTROL,
        )),
        Some(Action::SelectAll)
    );
    assert_eq!(
        key_bindings.action_for_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Left,
            crossterm::event::KeyModifiers::ALT,
        )),
        Some(Action::HistoryBack)
    );
    assert_eq!(
        key_bindings.action_for_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Right,
            crossterm::event::KeyModifiers::ALT,
        )),
        Some(Action::HistoryForward)
    );
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
fn open_with_reserved_shortcuts_follow_custom_menu_controls() {
    let config = Config::from_str(
        r#"
[keys]
nav_down = "n"
nav_up = "m"
nav_right = []
open_or_enter = ["enter", "l", "right"]
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.keys.open_with_reserved_shortcuts(),
        vec!['n', 'm', 'l']
    );
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
fn choose_defaults_to_enter_but_normal_enter_stays_open_or_enter() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.choose.to_string(), "Enter");
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(Action::OpenOrEnter)
    );
    assert_eq!(
        key_bindings.chooser_action_for_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        Some(ChooserKeyAction::Choose)
    );
}

#[test]
fn chooser_lookup_prioritizes_choose_over_smart_open_or_enter() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
nav_right = []
open_or_enter = ["enter", "l", "right"]
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Some(Action::OpenOrEnter)
    );
    assert_eq!(
        config.keys.chooser_action_for_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        Some(ChooserKeyAction::Choose)
    );
    assert_eq!(
        config.keys.chooser_action_for_key(
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        Some(ChooserKeyAction::Normal(Action::OpenOrEnter))
    );
    assert_eq!(
        config.keys.chooser_action_for_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        Some(ChooserKeyAction::Normal(Action::OpenOrEnter))
    );
}

#[test]
fn choose_can_be_unbound_or_rebound_without_changing_normal_enter() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let unbound = Config::from_str(
        r#"
[keys]
choose = []
"#,
    )
    .expect("config should parse");
    assert_eq!(
        unbound.keys.chooser_action_for_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        Some(ChooserKeyAction::Normal(Action::OpenOrEnter))
    );

    let rebound = Config::from_str(
        r#"
[keys]
choose = "ctrl+enter"
"#,
    )
    .expect("config should parse");
    assert_eq!(
        rebound.keys.chooser_action_for_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        Some(ChooserKeyAction::Normal(Action::OpenOrEnter))
    );
    assert_eq!(
        rebound.keys.chooser_action_for_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
            KeyContext::Normal,
        ),
        Some(ChooserKeyAction::Choose)
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
    assert_eq!(key_bindings.action_for('!'), Some(Action::ShellHere));
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
    assert_eq!(config.keys.action_for('S'), Some(Action::ShellHere));
    assert_eq!(config.keys.action_for('!'), None);
}
