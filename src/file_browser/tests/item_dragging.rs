use super::super::FileBrowserState;
use crate::filesystem::Entry;
use std::path::PathBuf;

fn browser_with_entries() -> (FileBrowserState, PathBuf, PathBuf, PathBuf) {
    let alpha = PathBuf::from("/tmp/elio-drag/alpha.txt");
    let beta = PathBuf::from("/tmp/elio-drag/beta.txt");
    let gamma = PathBuf::from("/tmp/elio-drag/gamma.txt");
    let mut browser = FileBrowserState::new(PathBuf::from("/tmp/elio-drag"), false, 0, false);
    browser.entries = [&alpha, &beta, &gamma]
        .into_iter()
        .map(|path| Entry {
            path: path.clone(),
            name: path
                .file_name()
                .expect("test path should have a name")
                .to_string_lossy()
                .into_owned(),
            ..Entry::default()
        })
        .collect();
    browser.selected = 1;
    (browser, alpha, beta, gamma)
}

#[test]
fn drag_without_selection_exports_focused_item() {
    let (browser, _, beta, _) = browser_with_entries();

    assert_eq!(browser.drag_export_paths(), vec![beta]);
}

#[test]
fn drag_with_no_items_exports_nothing() {
    let mut browser = FileBrowserState::new(PathBuf::from("/tmp/elio-drag"), false, 0, false);
    browser.entries.clear();

    assert!(browser.drag_export_paths().is_empty());
}

#[test]
fn dragging_selected_item_snapshots_the_selection() {
    let (mut browser, alpha, _, gamma) = browser_with_entries();
    browser.selected_paths.insert(gamma.clone());
    browser.selected_paths.insert(alpha.clone());
    browser.remember_drag_candidate(gamma.clone());
    browser.selected_paths.clear();

    assert_eq!(browser.take_drag_export_paths(None), vec![alpha, gamma]);
}

#[test]
fn dragging_unselected_item_exports_only_that_item() {
    let (mut browser, _, beta, gamma) = browser_with_entries();
    browser.selected_paths.insert(gamma);
    browser.remember_drag_candidate(beta.clone());

    assert_eq!(browser.take_drag_export_paths(None), vec![beta]);
}

#[test]
fn suppressed_drag_exports_nothing_until_cleared() {
    let (mut browser, _, beta, _) = browser_with_entries();
    browser.remember_drag_candidate(beta.clone());
    browser.suppress_drag_until_button_up();

    assert!(
        browser
            .take_drag_export_paths(Some(beta.clone()))
            .is_empty()
    );

    browser.clear_drag_state();
    browser.remember_drag_candidate(beta.clone());
    assert_eq!(browser.take_drag_export_paths(None), vec![beta]);
}
