use super::super::*;

#[test]
fn keys_default_bindings_are_sane() {
    let config = Config::default_config();
    assert_eq!(config.keys.yank, 'y');
    assert_eq!(config.keys.cut, 'x');
    assert_eq!(config.keys.paste, 'p');
    assert_eq!(config.keys.quit, 'q');
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
yank = "j"
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
    assert_eq!(key_bindings.action_for('q'), Some(Action::Quit));
    assert_eq!(key_bindings.action_for('o'), Some(Action::Open));
    assert_eq!(key_bindings.action_for('O'), Some(Action::OpenWith));
    assert_eq!(key_bindings.action_for('j'), None);
    assert_eq!(key_bindings.action_for('z'), None);
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
