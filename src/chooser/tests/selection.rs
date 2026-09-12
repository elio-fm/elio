use super::super::{ChooserExit, ChooserState};
use std::path::{Path, PathBuf};

#[test]
fn disabled_chooser_ignores_confirmation_and_cancellation() {
    let mut chooser = ChooserState::default();

    assert!(!chooser.confirm_path(Path::new("/cwd"), Path::new("item")));
    assert!(!chooser.cancel());
    assert_eq!(chooser.exit(), None);
}

#[test]
fn focused_relative_path_is_resolved_against_current_directory() {
    let mut chooser = ChooserState::default();
    chooser.enable();

    assert!(chooser.confirm_selection(Path::new("/cwd"), Some(Path::new("item")), Vec::new(),));
    assert_eq!(
        chooser.exit(),
        Some(&ChooserExit::Confirmed(vec![PathBuf::from("/cwd/item")]))
    );
}

#[test]
fn selected_paths_are_absolute_sorted_and_deduplicated() {
    let mut chooser = ChooserState::default();
    chooser.enable();

    assert!(chooser.confirm_selection(
        Path::new("/cwd"),
        Some(Path::new("ignored")),
        vec![
            PathBuf::from("zeta"),
            PathBuf::from("/absolute"),
            PathBuf::from("zeta"),
        ],
    ));
    assert_eq!(
        chooser.exit(),
        Some(&ChooserExit::Confirmed(vec![
            PathBuf::from("/absolute"),
            PathBuf::from("/cwd/zeta"),
        ]))
    );
}
