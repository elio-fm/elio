use super::*;

fn candidate(name: &str) -> SearchCandidate {
    SearchCandidate {
        path: PathBuf::from(name),
        name: name.to_string(),
        name_key: name.to_lowercase(),
        relative: name.to_string(),
        relative_key: name.to_lowercase(),
        is_dir: false,
        symlink: None,
    }
}

#[test]
fn selection_and_scroll_stay_inside_search_results() {
    let candidates = Arc::new(vec![candidate("a"), candidate("b"), candidate("c")]);
    let mut search = SearchState::new(
        SearchScope::Files,
        candidates,
        SearchIndexStats::default(),
        false,
    );

    search.move_selection(2);
    assert!(search.sync_scroll(2));
    assert_eq!(search.selected, 2);
    assert_eq!(search.scroll, 1);

    search.select_index(usize::MAX);
    assert_eq!(search.selected_path(), Some(PathBuf::from("c")));
}

#[test]
fn streamed_candidates_update_the_active_query() {
    let mut search = SearchState::new(
        SearchScope::Files,
        Arc::new(vec![candidate("alpha")]),
        SearchIndexStats::default(),
        true,
    );
    search.query = "beta".to_string();
    search.refresh_matches("");

    search.append_candidates(vec![candidate("beta")]);

    assert_eq!(search.match_count(), 1);
    assert_eq!(search.selected_path(), Some(PathBuf::from("beta")));
}
