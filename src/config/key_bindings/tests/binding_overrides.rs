use super::super::super::*;

#[test]
fn keys_default_bindings_are_sane() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::default_config();
    assert_eq!(config.keys.yank, 'y');
    assert_eq!(config.keys.cut, 'x');
    assert_eq!(config.keys.paste, 'p');
    assert_eq!(config.keys.toggle_preview, 'V');
    assert_eq!(config.keys.action_for('V'), Some(Action::TogglePreview));
    assert_eq!(config.keys.fullscreen_preview, 'P');
    assert_eq!(config.keys.action_for('P'), Some(Action::FullscreenPreview));
    assert_eq!(config.keys.extract_archive, 'e');
    assert_eq!(config.keys.symlink_absolute, '-');
    assert_eq!(config.keys.symlink_relative, '_');
    assert_eq!(config.keys.trash.to_string(), "d/Del");
    assert_eq!(config.keys.action_for('d'), Some(Action::Trash));
    assert_eq!(
        config
            .keys
            .action_for_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)),
        Some(Action::Trash)
    );
    assert_eq!(config.keys.delete_permanently.to_string(), "D/Shift+Del");
    assert_eq!(config.keys.choose.to_string(), "Enter");
    assert_eq!(config.keys.clear_selection.to_string(), "Esc");
    assert_eq!(config.keys.cancel.to_string(), "Esc");
    assert_eq!(config.keys.quit, 'q');
    assert_eq!(config.keys.quit_without_cd, 'Q');
    assert_eq!(config.keys.zoxide, 'z');
    assert_eq!(config.keys.shell, '!');
}

#[test]
fn cancel_binding_can_be_overridden() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
cancel = "ctrl+q"
"#,
    )
    .expect("config should parse");

    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert_eq!(config.keys.action_for_key(key), Some(Action::Cancel));
    assert_eq!(
        config.keys.action_for_key(KeyEvent::from(KeyCode::Esc)),
        Some(Action::ClearSelection)
    );
}

#[test]
fn clear_selection_can_use_a_character_binding() {
    let config = Config::from_str(
        r#"
[keys]
clear_selection = "T"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.action_for('T'), Some(Action::ClearSelection));
    assert_eq!(config.keys.clear_selection.to_string(), "T");
}

#[test]
fn cancel_rejects_bare_character_bindings() {
    let config = Config::from_str(
        r#"
[keys]
cancel = "T"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.cancel.to_string(), "Esc");
}

#[test]
fn clear_selection_and_cancel_may_share_a_binding() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = Config::from_str(
        r#"
[keys]
clear_selection = "ctrl+q"
cancel = "ctrl+q"
"#,
    )
    .expect("config should parse");

    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert_eq!(
        config.keys.action_for_key(key),
        Some(Action::ClearSelection)
    );
    assert!(config.keys.matches_cancel_key(key));
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
fn unknown_keys_are_ignored_without_dropping_valid_overrides() {
    let config = Config::from_str(
        r#"
[keys]
open_withh = "w"
open_with = "M"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.open_with, 'M');
    assert_eq!(config.keys.action_for('w'), None);
}

#[test]
fn fullscreen_preview_key_can_be_overridden() {
    let config = Config::from_str(
        r#"
[keys]
fullscreen_preview = "F"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.action_for('F'), Some(Action::FullscreenPreview));
    assert_eq!(config.keys.action_for('P'), None);
}

#[test]
fn legacy_copy_path_binding_remains_accepted() {
    let config = Config::from_str(
        r#"
[keys]
copy_path = "M"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.action_for('M'), Some(Action::CopyToClipboard));
    assert_eq!(config.keys.action_for('c'), None);
}

#[test]
fn copy_to_clipboard_binding_takes_precedence_over_legacy_name() {
    let config = Config::from_str(
        r#"
[keys]
copy_path = "M"
copy_to_clipboard = "N"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.action_for('N'), Some(Action::CopyToClipboard));
    assert_eq!(config.keys.action_for('M'), None);
}

#[test]
fn symlink_keys_can_be_overridden() {
    let config = Config::from_str(
        r#"
[keys]
symlink_absolute = "m"
symlink_relative = "M"
"#,
    )
    .expect("config should parse");

    assert_eq!(config.keys.action_for('m'), Some(Action::SymlinkAbsolute));
    assert_eq!(config.keys.action_for('M'), Some(Action::SymlinkRelative));
    assert_eq!(config.keys.action_for('-'), None);
    assert_eq!(config.keys.action_for('_'), None);
}

#[test]
fn unknown_key_warning_names_config_path() {
    assert_eq!(
        super::super::binding_validation::unknown_key_action_warning("open_withh"),
        "elio: keys.open_withh: unknown key action; ignoring"
    );
}

#[test]
fn keys_accept_array_overrides() {
    let config = Config::from_str(
        r#"
[keys]
extract_archive = []
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
    assert_eq!(config.keys.action_for('o'), Some(Action::ShellHere));
    assert_eq!(config.keys.action_for('!'), None);
}

#[test]
fn keys_rejects_duplicate_inside_array_and_uses_default() {
    let config = Config::from_str(
        r#"
[keys]
extract_archive = []
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
extract_archive = []
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
extract_archive = []
open = ["o", "e"]
"#,
    )
    .expect("config should parse");
    assert_eq!(config.keys.open.to_string(), "o/e");
}
