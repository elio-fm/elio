use super::*;

fn entry_key(entries: &[HelpEntry], action: &str) -> String {
    entries
        .iter()
        .find(|entry| entry.action == action)
        .unwrap_or_else(|| panic!("missing help entry for {action:?}"))
        .key
        .clone()
}

fn entry_keys(entries: &[HelpEntry], action: &str) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| entry.action == action)
        .map(|entry| entry.key.clone())
        .collect()
}

fn has_action(entries: &[HelpEntry], action: &str) -> bool {
    entries.iter().any(|entry| entry.action == action)
}

#[test]
fn help_uses_configurable_browser_control_defaults() {
    let kb = KeyBindings::default();
    let keys = HelpKeys::new(&kb, HelpMode::Normal);
    let navigation = navigation_entries(&keys);
    let clipboard = clipboard_entries(&keys);

    assert_eq!(entry_key(&navigation, "go-to menu"), "g");
    assert_eq!(entry_key(&navigation, "first item"), "Home");
    assert_eq!(entry_key(&navigation, "last item"), "G/End");
    assert_eq!(
        entry_key(&navigation, "page up / down"),
        "PageUp / PageDown"
    );
    assert_eq!(entry_key(&navigation, "cycle places"), "Tab / Shift+Tab");
    assert_eq!(entry_key(&navigation, "back / forward"), "Alt+← / Alt+→");
    assert_eq!(entry_key(&clipboard, "toggle selection"), "Space");
    assert_eq!(entry_key(&clipboard, "select all"), "Ctrl+A");
    assert!(
        !has_action(&clipboard, "confirm selection"),
        "normal help should not show chooser-only actions"
    );
}

#[test]
fn help_reflects_rebound_browser_controls() {
    let kb = KeyBindings::from_toml_str(
        r#"[keys]
go_to = "u"
toggle_selection = "t"
cycle_places_next = "n"
cycle_places_previous = "N"
page_up = "<"
page_down = ">"
jump_first = "1"
jump_last = "2"
select_all = "A"
history_back = "alt+h"
history_forward = "alt+l"
"#,
    );
    let keys = HelpKeys::new(&kb, HelpMode::Normal);
    let navigation = navigation_entries(&keys);
    let clipboard = clipboard_entries(&keys);

    assert_eq!(entry_key(&navigation, "go-to menu"), "u");
    assert_eq!(entry_key(&navigation, "first item"), "1");
    assert_eq!(entry_key(&navigation, "last item"), "2");
    assert_eq!(entry_key(&navigation, "page up / down"), "< / >");
    assert_eq!(entry_key(&navigation, "cycle places"), "n / N");
    assert_eq!(entry_key(&navigation, "back / forward"), "Alt+H / Alt+L");
    assert_eq!(entry_key(&clipboard, "toggle selection"), "t");
    assert_eq!(entry_key(&clipboard, "select all"), "A");
}

#[test]
fn chooser_help_adds_choose_and_relabels_quit() {
    let kb = KeyBindings::default();
    let keys = HelpKeys::new(&kb, HelpMode::Chooser);
    let clipboard = clipboard_entries(&keys);
    let view_entries = entries([
        keys.action(&kb.quit, "cancel chooser"),
        keys.action(&kb.quit_without_cd, "cancel chooser"),
    ]);

    assert_eq!(
        double_click_action(HelpMode::Chooser),
        "enter folder / choose"
    );
    assert_eq!(entry_key(&navigation_entries(&keys), "choose"), "Enter");
    assert!(!has_action(&clipboard, "confirm selection"));
    assert_eq!(entry_key(&view_entries, "cancel chooser"), "q");
    assert!(
        !has_action(&navigation_entries(&keys), "enter folder / open"),
        "chooser help should hide actions whose bindings are fully shadowed"
    );
    assert!(
        !has_action(&view_entries, "quit"),
        "chooser help should describe quit keys as chooser cancellation"
    );
}

#[test]
fn chooser_help_removes_choose_key_from_normal_action_labels() {
    let kb = KeyBindings::from_toml_str(
        r#"[keys]
nav_right = []
open_or_enter = ["enter", "l", "right"]
"#,
    );

    let keys = HelpKeys::new(&kb, HelpMode::Chooser);
    let navigation = navigation_entries(&keys);

    assert_eq!(entry_key(&navigation, "choose"), "Enter");
    assert_eq!(entry_key(&navigation, "enter folder / open"), "l/→");
}

#[test]
fn chooser_help_gives_choose_precedence_over_quit() {
    let kb = KeyBindings::from_toml_str(
        r#"[keys]
choose = "q"
"#,
    );
    let keys = HelpKeys::new(&kb, HelpMode::Chooser);
    let navigation = navigation_entries(&keys);
    let view_entries = entries([
        keys.action(&kb.quit, "cancel chooser"),
        keys.action(&kb.quit_without_cd, "cancel chooser"),
    ]);

    assert_eq!(entry_key(&navigation, "choose"), "q");
    assert_eq!(
        entry_keys(&view_entries, "cancel chooser"),
        vec!["Q".to_string()]
    );
}

#[test]
fn preview_scroll_help_drops_chooser_shadowed_keys() {
    let kb = KeyBindings::from_toml_str(
        r#"[keys]
choose = "K"
"#,
    );
    let keys = HelpKeys::new(&kb, HelpMode::Chooser);

    assert_eq!(keys.key(&kb.scroll_preview_up), "Shift+↑");
    assert_eq!(keys.key(&kb.scroll_preview_down), "J/Shift+↓");

    let kb = KeyBindings::from_toml_str(
        r#"[keys]
choose = ["K", "shift+up"]
"#,
    );
    let keys = HelpKeys::new(&kb, HelpMode::Chooser);

    assert_eq!(keys.key(&kb.scroll_preview_up), "");
    assert_eq!(keys.key(&kb.scroll_preview_down), "J/Shift+↓");
}

#[test]
fn preview_scroll_help_uses_standard_uppercase_notation() {
    let kb = KeyBindings::default();
    let keys = HelpKeys::new(&kb, HelpMode::Normal);

    assert_eq!(keys.key(&kb.scroll_preview_up), "K/Shift+↑");
    assert_eq!(keys.key(&kb.scroll_preview_down), "J/Shift+↓");
    assert_eq!(keys.key(&kb.scroll_preview_left), "H/Shift+←");
    assert_eq!(keys.key(&kb.scroll_preview_right), "L/Shift+→");

    let kb = KeyBindings::from_toml_str(
        r#"[keys]
scroll_preview_left = "A"
scroll_preview_right = "B"
"#,
    );
    let keys = HelpKeys::new(&kb, HelpMode::Normal);

    assert_eq!(keys.key(&kb.scroll_preview_left), "A");
    assert_eq!(keys.key(&kb.scroll_preview_right), "B");
}
