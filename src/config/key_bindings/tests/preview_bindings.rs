use super::super::super::*;

#[test]
fn preview_scroll_defaults_map_to_shift_h_j_k_l_and_shift_arrows() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let key_bindings = KeyBindings::default();
    assert_eq!(key_bindings.scroll_preview_up.to_string(), "K/Shift+↑");
    assert_eq!(key_bindings.scroll_preview_down.to_string(), "J/Shift+↓");
    assert_eq!(key_bindings.scroll_preview_left.to_string(), "H/Shift+←");
    assert_eq!(key_bindings.scroll_preview_right.to_string(), "L/Shift+→");
    assert_eq!(key_bindings.action_for('K'), Some(Action::ScrollPreviewUp));
    assert_eq!(key_bindings.action_for('['), None);
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT)),
        Some(Action::ScrollPreviewUp)
    );
    assert_eq!(
        key_bindings.action_for('J'),
        Some(Action::ScrollPreviewDown)
    );
    assert_eq!(key_bindings.action_for(']'), None);
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT)),
        Some(Action::ScrollPreviewDown)
    );
    assert_eq!(
        key_bindings.action_for('H'),
        Some(Action::ScrollPreviewLeft)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT)),
        Some(Action::ScrollPreviewLeft)
    );
    assert_eq!(
        key_bindings.action_for('L'),
        Some(Action::ScrollPreviewRight)
    );
    assert_eq!(
        key_bindings.action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)),
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
fn brackets_remain_valid_configurable_preview_scroll_keys() {
    let config = Config::from_str(
        r#"
[keys]
scroll_preview_up = "["
scroll_preview_down = "]"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.action_for('['), Some(Action::ScrollPreviewUp));
    assert_eq!(config.keys.action_for(']'), Some(Action::ScrollPreviewDown));
    assert_eq!(config.keys.action_for('K'), None);
    assert_eq!(config.keys.action_for('J'), None);
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
        config.keys.scroll_preview_up.to_string(),
        "K/Shift+↑",
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
    assert_eq!(config.keys.scroll_preview_up.to_string(), "K/Shift+↑");
    assert_eq!(config.keys.scroll_preview_down, 'U');
    assert_eq!(config.keys.action_for('U'), Some(Action::ScrollPreviewDown));
    assert_eq!(config.keys.action_for('K'), Some(Action::ScrollPreviewUp));
    assert_eq!(
        config.keys.action_for('J'),
        None,
        "default 'J' is no longer bound because scroll_preview_down was overridden to 'U'"
    );
}
