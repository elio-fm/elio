use super::super::{DirectoryHistoryMode, FileBrowserState};
use std::path::PathBuf;

fn browser(cwd: &str) -> FileBrowserState {
    FileBrowserState::new(PathBuf::from(cwd), false, 0, false)
}

#[test]
fn directory_history_records_back_and_forward_transitions() {
    let mut browser = browser("/next");
    browser.apply_directory_history(
        DirectoryHistoryMode::PushCurrent,
        PathBuf::from("/previous"),
        Some(PathBuf::from("/previous/item")),
    );

    assert_eq!(browser.directory_history.back.len(), 1);
    assert!(browser.directory_history.forward.is_empty());

    browser.apply_directory_history(DirectoryHistoryMode::GoBack, PathBuf::from("/next"), None);
    assert!(browser.directory_history.back.is_empty());
    assert_eq!(browser.directory_history.forward.len(), 1);
}

#[test]
fn directory_escape_chooses_the_shallowest_affected_parent() {
    let browser = browser("/work/project/src");
    let paths = vec![
        PathBuf::from("/work/project/src/file.rs"),
        PathBuf::from("/work/project"),
    ];

    assert_eq!(
        browser.current_directory_escape_for_paths(&paths),
        Some(PathBuf::from("/work"))
    );
}
