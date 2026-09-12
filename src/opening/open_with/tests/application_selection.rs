use super::*;

fn application(name: &str, terminal: bool, default: bool) -> OpenWithApplication {
    OpenWithApplication {
        display_name: name.to_string(),
        application_id: None,
        program: name.to_lowercase(),
        args: Vec::new(),
        is_default: default,
        requires_terminal: terminal,
    }
}

#[test]
fn rows_assign_available_shortcuts_and_descriptive_labels() {
    let selection = ApplicationSelection::new(
        vec![
            application("GUI", false, true),
            application("Editor", true, false),
        ],
        &['1'],
    );

    assert_eq!(selection.rows[0].shortcut, Some('2'));
    assert_eq!(selection.rows[0].label, "GUI (default)");
    assert_eq!(selection.rows[1].label, "Editor (terminal)");
}

#[test]
fn selection_and_shortcut_lookup_are_clamped() {
    let mut selection = ApplicationSelection::new(
        vec![
            application("A", false, false),
            application("B", false, false),
        ],
        &[],
    );

    selection.move_selection(20);
    assert_eq!(selection.selected, 1);
    assert_eq!(selection.row_index_for_shortcut('1'), Some(0));
    assert_eq!(
        selection
            .application_at(1)
            .map(|app| app.display_name.as_str()),
        Some("B")
    );
}
