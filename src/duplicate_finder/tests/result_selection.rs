use super::*;
use crate::duplicate_finder::{DuplicateFile, DuplicateGroup};

fn state() -> DuplicateFinderSession {
    let mut state = DuplicateFinderSession::new(PathBuf::from("root"));
    state.groups = vec![DuplicateGroup {
        id: 1,
        size: 1,
        files: ["a", "b", "c"]
            .into_iter()
            .map(|name| DuplicateFile {
                path: PathBuf::from("root").join(name),
                name: name.to_string(),
                relative: name.to_string(),
                size: 1,
                modified: None,
            })
            .collect(),
    }];
    state
}

#[test]
fn focus_selection_and_scroll_are_clamped() {
    let mut state = state();
    state.move_selection(20);
    assert_eq!(state.selected, 2);
    assert!(state.sync_scroll(2));
    assert_eq!(state.scroll, 1);
}

#[test]
fn action_paths_prefer_selected_rows_over_focus() {
    let mut state = state();
    state.selected = 2;
    state.selected_paths.insert(PathBuf::from("root/b"));
    state.selected_paths.insert(PathBuf::from("root/a"));

    assert_eq!(
        state.action_paths(),
        [PathBuf::from("root/a"), PathBuf::from("root/b")]
    );
}
