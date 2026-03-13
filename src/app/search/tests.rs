use super::super::*;
use crossterm::event::{KeyCode, KeyEvent};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-searching-{label}-{unique}"))
}

#[test]
fn opening_search_restarts_index_when_cache_missing_even_if_loading() {
    let root = temp_path("restarts-index");
    fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create temp tree");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.search_loading = true;
    let previous_token = app.search_token;

    app.open_fuzzy_finder(SearchScope::Folders)
        .expect("failed to open search");

    assert!(app.search_loading);
    assert!(app.search_token > previous_token);

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn opening_search_uses_hidden_candidates_even_when_browser_hides_dotfiles() {
    let root = temp_path("hidden-cache");
    fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create temp tree");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.show_hidden = false;
    app.search_cache = Some(SearchCache {
        cwd: root.clone(),
        scope: SearchScope::Folders,
        candidates: Arc::new(vec![crate::fs::search::SearchCandidate {
            path: root.join(".hidden-root/needle"),
            name: "needle".to_string(),
            name_key: "needle".to_string(),
            relative: ".hidden-root/needle".to_string(),
            relative_key: ".hidden-root/needle".to_string(),
            is_dir: true,
        }]),
    });

    app.open_fuzzy_finder(SearchScope::Folders)
        .expect("failed to open search");

    assert_eq!(app.search_candidate_count(), 1);
    assert_eq!(
        app.search_rows(10).first().map(|row| row.relative.as_str()),
        Some(".hidden-root/needle")
    );

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn refining_query_rechecks_full_candidate_set() {
    let root = temp_path("query-refine");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let mut candidates = Vec::new();
    for index in 0..300 {
        let name = format!("f{index:03}");
        candidates.push(crate::fs::search::SearchCandidate {
            path: root.join(&name),
            name: name.clone(),
            name_key: name.clone(),
            relative: name.clone(),
            relative_key: name,
            is_dir: true,
        });
    }
    candidates.push(crate::fs::search::SearchCandidate {
        path: root.join("fastfetch"),
        name: "fastfetch".to_string(),
        name_key: "fastfetch".to_string(),
        relative: "fastfetch".to_string(),
        relative_key: "fastfetch".to_string(),
        is_dir: true,
    });

    app.search = Some(SearchOverlay {
        scope: SearchScope::Folders,
        query: "f".to_string(),
        query_cursor: 1,
        candidates: Arc::new(candidates),
        matches: Vec::new(),
        cached_matches: HashMap::from([(String::new(), (0..301).collect())]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });
    app.refresh_search_matches("");
    let fastfetch_index = app
        .search
        .as_ref()
        .and_then(|search| {
            search
                .candidates
                .iter()
                .position(|candidate| candidate.relative == "fastfetch")
        })
        .expect("fastfetch candidate should exist");
    assert!(
        !app.search
            .as_ref()
            .expect("search should be open")
            .matches
            .contains(&fastfetch_index)
    );

    if let Some(search) = &mut app.search {
        search.query = "fastfetch".to_string();
    }
    app.refresh_search_matches("f");

    let search = app.search.as_ref().expect("search should be open");
    assert_eq!(search.matches.first().copied(), Some(fastfetch_index));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn search_query_cursor_inserts_and_deletes_in_place() {
    let root = temp_path("cursor-edit");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.search = Some(SearchOverlay {
        scope: SearchScope::Folders,
        query: "fatch".to_string(),
        query_cursor: 2,
        candidates: Arc::new(Vec::new()),
        matches: Vec::new(),
        cached_matches: HashMap::from([(String::new(), Vec::new())]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });

    app.handle_search_key(KeyEvent::from(KeyCode::Char('s')))
        .expect("typing should work");
    assert_eq!(app.search_query(), "fastch");
    assert_eq!(app.search_query_cursor(), 3);

    app.handle_search_key(KeyEvent::from(KeyCode::Left))
        .expect("moving cursor should work");
    app.handle_search_key(KeyEvent::from(KeyCode::Delete))
        .expect("delete should work");
    assert_eq!(app.search_query(), "fatch");
    assert_eq!(app.search_query_cursor(), 2);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn search_rows_ignore_stale_match_indexes() {
    let root = temp_path("stale-match-indexes");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.search = Some(SearchOverlay {
        scope: SearchScope::Folders,
        query: String::new(),
        query_cursor: 0,
        candidates: Arc::new(Vec::new()),
        matches: vec![3],
        cached_matches: HashMap::from([(String::new(), vec![3])]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });

    assert!(app.search_rows(10).is_empty());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_search_selection_keeps_overlay_open_when_reveal_fails() {
    let root = temp_path("reveal-fails");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let missing = root.join("missing/file.txt");
    app.search = Some(SearchOverlay {
        scope: SearchScope::Files,
        query: "missing".to_string(),
        query_cursor: 7,
        candidates: Arc::new(vec![crate::fs::search::SearchCandidate {
            path: missing,
            name: "file.txt".to_string(),
            name_key: "file.txt".to_string(),
            relative: "missing/file.txt".to_string(),
            relative_key: "missing/file.txt".to_string(),
            is_dir: false,
        }]),
        matches: vec![0],
        cached_matches: HashMap::from([(String::new(), vec![0])]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });

    assert!(app.confirm_search_selection().is_err());
    assert!(app.search.is_some());
    assert_eq!(app.cwd, root);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
