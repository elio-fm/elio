use super::super::SelectedPaths;
use std::path::PathBuf;

#[test]
fn selected_paths_tracks_nested_conflicts_without_scanning_selected_siblings() {
    let mut selected = SelectedPaths::default();
    let siblings: Vec<PathBuf> = (0..1_000)
        .map(|index| PathBuf::from(format!("/tmp/elio-selection/file-{index}")))
        .collect();

    for path in siblings.iter().cloned() {
        assert!(selected.insert(path));
    }
    assert_eq!(selected.len(), siblings.len());
    assert!(selected.has_nesting_conflict(PathBuf::from("/tmp/elio-selection").as_path()));
    assert!(!selected.has_nesting_conflict(PathBuf::from("/tmp/other").as_path()));

    assert!(selected.remove(&siblings[0]));
    assert!(!selected.contains(&siblings[0]));
    assert_eq!(selected.len(), siblings.len() - 1);
    assert!(selected.has_nesting_conflict(PathBuf::from("/tmp/elio-selection").as_path()));

    selected.clear();
    assert!(selected.is_empty());
    assert!(!selected.has_nesting_conflict(PathBuf::from("/tmp/elio-selection").as_path()));
}

#[test]
fn selected_paths_rejects_parent_child_mixes() {
    let mut selected = SelectedPaths::default();
    let parent = PathBuf::from("/tmp/elio-selection");
    let child = parent.join("child");
    let sibling = PathBuf::from("/tmp/other");

    assert!(selected.insert(child.clone()));
    assert!(!selected.insert(parent.clone()));
    assert!(selected.insert(sibling));

    assert!(selected.remove(&child));
    assert!(selected.insert(parent.clone()));
    assert!(!selected.insert(parent.join("nested")));
}
