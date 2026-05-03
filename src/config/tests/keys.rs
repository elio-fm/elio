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
