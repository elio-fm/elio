use super::*;

fn group(names: &[&str]) -> DuplicateGroup {
    DuplicateGroup {
        id: 1,
        size: 10,
        files: names
            .iter()
            .map(|name| DuplicateFile {
                path: PathBuf::from("root").join(name),
                name: (*name).to_string(),
                relative: (*name).to_string(),
                size: 10,
                modified: None,
            })
            .collect(),
    }
}

#[test]
fn rows_preserve_group_context_when_scrolled() {
    let mut finder = DuplicateFinderState::new(PathBuf::from("root"));
    finder.groups = vec![group(&["a", "b", "c"]), group(&["d", "e"])];
    finder.scroll = 1;
    finder.selected = 2;

    let rows = finder.rows(3);

    assert_eq!(
        rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        ["b", "c", "d"]
    );
    assert_eq!(rows[0].group_rank, 1);
    assert!(!rows[0].group_first);
    assert!(rows[1].focused);
    assert!(rows[2].group_first);
}

#[test]
fn removing_paths_drops_empty_groups_and_clamps_empty_state() {
    let mut finder = DuplicateFinderState::new(PathBuf::from("root"));
    finder.groups = vec![group(&["a"])];
    finder.selected = 4;
    finder.scroll = 3;

    finder.remove_paths(&[PathBuf::from("root/a")]);

    assert!(finder.groups.is_empty());
    assert_eq!(finder.selected, 0);
    assert_eq!(finder.scroll, 0);
}
