use super::super::FileBrowserState;
use super::super::ViewMode;
use crate::filesystem::Entry;
use std::path::PathBuf;

#[test]
fn startup_view_mode_defaults_to_list() {
    assert_eq!(ViewMode::from_start_in_grid(false), ViewMode::List);
}

#[test]
fn startup_view_mode_can_start_in_grid() {
    assert_eq!(ViewMode::from_start_in_grid(true), ViewMode::Grid);
}

#[test]
fn grid_navigation_preserves_the_selected_column() {
    let mut browser = FileBrowserState::new(PathBuf::from("/tmp"), true, 0, false);
    browser.entries = (0..8).map(|_| Entry::default()).collect();
    browser.selected = 1;

    assert_eq!(browser.grid_selection_offset(1, 3), Some(4));
    assert_eq!(browser.grid_selection_offset(2, 3), Some(7));
    assert_eq!(browser.grid_selection_offset(3, 3), None);
}
