use super::super::FileBrowserState;
use crate::fs::Entry;
use std::path::PathBuf;

#[test]
fn visible_directory_count_range_respects_grid_viewport() {
    let mut browser = FileBrowserState::new(PathBuf::from("/tmp"), false, 0, false);
    browser.entries = (0..6).map(|_| Entry::default()).collect();
    browser.scroll_row = 1;

    assert_eq!(browser.visible_entry_indices(2, 2), vec![2, 3, 4, 5]);
}
