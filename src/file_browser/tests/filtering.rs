use super::super::FileBrowserState;
use crate::filesystem::Entry;
use std::path::PathBuf;

fn entry(name: &str) -> Entry {
    Entry {
        path: PathBuf::from(name),
        name: name.to_string(),
        name_key: name.to_string(),
        ..Entry::default()
    }
}

#[test]
fn filtering_matches_names_without_losing_the_selected_item() {
    let mut browser = FileBrowserState::new(PathBuf::from("/tmp"), false, 0, false);
    browser.unfiltered_entries = vec![entry("alpha.txt"), entry("beta.txt"), entry("alphabet.txt")];
    browser.entries = browser.unfiltered_entries.clone();
    browser.selected = 1;
    browser.local_filter.query = "BETA".to_string();

    browser.apply_local_filter_preserving_selection();

    assert_eq!(browser.entries.len(), 1);
    assert_eq!(browser.entries[0].name, "beta.txt");
    assert_eq!(browser.selected, 0);
}
