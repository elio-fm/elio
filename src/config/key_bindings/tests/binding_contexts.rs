use super::super::super::*;

#[test]
fn default_rename_and_restore_share_r_in_disjoint_contexts() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let key_bindings = KeyBindings::default();
    let r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);

    assert_eq!(key_bindings.rename.to_string(), "r/F2");
    assert_eq!(key_bindings.restore_from_trash.to_string(), "r");
    assert_eq!(
        key_bindings.action_for_key_in_context(r, KeyContext::Normal),
        Some(Action::Rename)
    );
    assert_eq!(
        key_bindings.action_for_key_in_context(r, KeyContext::Trash),
        Some(Action::RestoreFromTrash)
    );
    assert_eq!(
        key_bindings.action_for_key_in_context(
            KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        Some(Action::Rename)
    );
    assert_eq!(
        key_bindings.action_for_key_in_context(
            KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
            KeyContext::Trash,
        ),
        None
    );
}

#[test]
fn normal_actions_cannot_reuse_contextual_r_default() {
    let config = Config::from_str(
        r#"
[keys]
open = "r"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.open.to_string(), "o");
    assert_eq!(config.keys.rename.to_string(), "r/F2");
    assert_eq!(config.keys.restore_from_trash.to_string(), "r");
}

#[test]
fn contextual_rename_and_restore_bindings_can_overlap() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
extract_archive = []
rename = "e"
restore_from_trash = "e"
"#,
    )
    .expect("config should parse");
    let e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);

    assert_eq!(
        config.keys.action_for_key_in_context(e, KeyContext::Normal),
        Some(Action::Rename)
    );
    assert_eq!(
        config.keys.action_for_key_in_context(e, KeyContext::Trash),
        Some(Action::RestoreFromTrash)
    );
}

#[test]
fn disabled_rename_removes_f2_without_disabling_restore() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
rename = []
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.keys.action_for_key_in_context(
            KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        None
    );
    assert_eq!(
        config.keys.action_for_key_in_context(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            KeyContext::Trash,
        ),
        Some(Action::RestoreFromTrash)
    );
}

#[test]
fn disabled_restore_from_trash_keeps_normal_rename_bindings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
restore_from_trash = []
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.keys.action_for_key_in_context(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            KeyContext::Trash,
        ),
        None
    );
    assert_eq!(
        config.keys.action_for_key_in_context(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        Some(Action::Rename)
    );
    assert_eq!(
        config.keys.action_for_key_in_context(
            KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
            KeyContext::Normal,
        ),
        Some(Action::Rename)
    );
}

#[test]
fn restore_from_trash_cannot_reuse_global_open_binding() {
    let config = Config::from_str(
        r#"
[keys]
restore_from_trash = "o"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.open.to_string(), "o");
    assert_eq!(config.keys.restore_from_trash.to_string(), "r");
}
