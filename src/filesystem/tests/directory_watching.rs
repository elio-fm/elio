use super::*;

#[test]
fn hidden_paths_are_ignored_when_dotfiles_are_hidden() {
    assert!(!event_affects_visible_entries(
        &[PathBuf::from("/tmp/.secret")],
        false,
    ));
}

#[test]
fn visible_paths_trigger_reload_when_dotfiles_are_hidden() {
    assert!(event_affects_visible_entries(
        &[PathBuf::from("/tmp/file.txt")],
        false,
    ));
}

#[test]
fn empty_path_events_force_rescan() {
    assert!(event_affects_visible_entries(&[], false));
}
