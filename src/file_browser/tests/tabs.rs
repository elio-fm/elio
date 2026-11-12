use super::super::{BrowserTabs, FileBrowserState};
use std::path::PathBuf;

fn browser(path: &str) -> FileBrowserState {
    FileBrowserState::new(PathBuf::from(path), false, 2, false)
}

#[test]
fn closing_the_active_tab_selects_the_previous_tab_or_the_next_first_tab() {
    let mut tabs = BrowserTabs::new();
    let mut active = browser("/one");
    tabs.open_after_active(&mut active, browser("/two"));
    tabs.open_after_active(&mut active, browser("/three"));

    assert_eq!(tabs.active_index(), 2);
    assert!(tabs.close(&mut active, 2));
    assert_eq!(active.cwd, PathBuf::from("/two"));
    assert_eq!(tabs.active_index(), 1);

    assert!(tabs.activate(&mut active, 0));
    assert!(tabs.close(&mut active, 0));
    assert_eq!(active.cwd, PathBuf::from("/two"));
    assert_eq!(tabs.active_index(), 0);
}

#[test]
fn closing_an_inactive_tab_keeps_the_active_browser_and_renumbers_it() {
    let mut tabs = BrowserTabs::new();
    let mut active = browser("/one");
    tabs.open_after_active(&mut active, browser("/two"));
    tabs.open_after_active(&mut active, browser("/three"));

    assert!(!tabs.close(&mut active, 0));
    assert_eq!(active.cwd, PathBuf::from("/three"));
    assert_eq!(tabs.active_index(), 1);
    assert_eq!(tabs.len(), 2);
}

#[test]
fn opening_at_the_end_preserves_existing_tab_positions() {
    let mut tabs = BrowserTabs::new();
    let mut active = browser("/one");
    tabs.open_after_active(&mut active, browser("/two"));
    tabs.open_after_active(&mut active, browser("/three"));
    assert!(tabs.activate(&mut active, 0));

    tabs.open_at_end(&mut active, browser("/four"));

    assert_eq!(tabs.active_index(), 3);
    assert_eq!(
        tabs.cwd_at(&active, 0),
        Some(PathBuf::from("/one").as_path())
    );
    assert_eq!(
        tabs.cwd_at(&active, 1),
        Some(PathBuf::from("/two").as_path())
    );
    assert_eq!(
        tabs.cwd_at(&active, 2),
        Some(PathBuf::from("/three").as_path())
    );
    assert_eq!(
        tabs.cwd_at(&active, 3),
        Some(PathBuf::from("/four").as_path())
    );
}

#[test]
fn cycling_wraps_and_a_single_tab_cannot_be_closed() {
    let mut tabs = BrowserTabs::new();
    let mut active = browser("/one");
    assert!(!tabs.close(&mut active, 0));
    tabs.open_after_active(&mut active, browser("/two"));

    assert!(tabs.cycle(&mut active, 1));
    assert_eq!(active.cwd, PathBuf::from("/one"));
    assert!(tabs.cycle(&mut active, -1));
    assert_eq!(active.cwd, PathBuf::from("/two"));
}

#[test]
fn tab_capacity_is_limited_to_ten() {
    let mut tabs = BrowserTabs::new();
    let mut active = browser("/one");
    while tabs.can_open() {
        tabs.open_after_active(&mut active, browser("/next"));
    }

    assert_eq!(tabs.len(), 10);
    assert!(!tabs.can_open());
}
