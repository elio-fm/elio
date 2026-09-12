use super::super::FileBrowserState;
use std::path::PathBuf;

#[test]
fn changing_directories_clears_git_status_before_refresh() {
    let first = PathBuf::from("/tmp/elio-git-first");
    let second = PathBuf::from("/tmp/elio-git-second");
    let mut browser = FileBrowserState::new(first.clone(), false, 0, false);

    let (token, cwd) = browser.begin_git_status_refresh();
    assert_eq!(cwd, first);
    assert!(browser.apply_git_status(token, cwd, Some("main".to_string()), true));
    assert_eq!(browser.git_branch(), Some("main"));
    assert!(browser.git_dirty());

    browser.cwd = second.clone();
    let (_, cwd) = browser.begin_git_status_refresh();
    assert_eq!(cwd, second);
    assert_eq!(browser.git_branch(), None);
    assert!(!browser.git_dirty());
}

#[test]
fn stale_git_status_results_are_ignored() {
    let cwd = PathBuf::from("/tmp/elio-git-status");
    let mut browser = FileBrowserState::new(cwd.clone(), false, 0, false);

    let (stale_token, _) = browser.begin_git_status_refresh();
    let (current_token, _) = browser.begin_git_status_refresh();

    assert!(!browser.apply_git_status(stale_token, cwd.clone(), Some("stale".into()), true));
    assert_eq!(browser.git_branch(), None);
    assert!(!browser.git_dirty());

    assert!(browser.apply_git_status(current_token, cwd, Some("main".into()), false));
    assert_eq!(browser.git_branch(), Some("main"));
}
