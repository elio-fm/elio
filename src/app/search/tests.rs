use super::super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    let path = std::env::temp_dir().join(format!("elio-searching-{label}-{unique}"));
    std::fs::create_dir_all(&path).ok();
    path.canonicalize().unwrap_or(path)
}

fn base_cache_entry(pool: Vec<usize>) -> SearchMatchCacheEntry {
    super::build_base_search_cache_entry(pool)
}

#[test]
fn opening_search_restarts_index_when_cache_missing_even_if_loading() {
    let root = temp_path("restarts-index");
    fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create temp tree");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.jobs.search_loading = true;
    let previous_token = app.jobs.search_token;

    app.open_fuzzy_finder(SearchScope::Folders)
        .expect("failed to open search");

    assert!(app.jobs.search_loading);
    assert!(app.jobs.search_token > previous_token);

    fs::remove_dir_all(root).expect("failed to remove temp tree");
}

#[test]
fn opening_search_ignores_hidden_cache_when_browser_hides_dotfiles() {
    let root = temp_path("hidden-cache-mismatch");
    fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create temp tree");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.show_hidden = false;
    app.jobs.search_cache = Some(SearchCache {
        cwd: root.clone(),
        scope: SearchScope::Folders,
        show_hidden: true,
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

    assert_eq!(app.search_candidate_count(), 0);
    assert!(app.search_is_loading());

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

    app.overlays.search = Some(SearchOverlay {
        scope: SearchScope::Folders,
        query: "f".to_string(),
        query_cursor: 1,
        candidates: Arc::new(candidates),
        matches: Vec::new(),
        cached_matches: HashMap::from([(String::new(), base_cache_entry((0..301).collect()))]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });
    app.refresh_search_matches("");
    let fastfetch_index = app
        .overlays
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
        !app.overlays
            .search
            .as_ref()
            .expect("search should be open")
            .matches
            .contains(&fastfetch_index)
    );

    if let Some(search) = &mut app.overlays.search {
        search.query = "fastfetch".to_string();
    }
    app.refresh_search_matches("f");

    let search = app.overlays.search.as_ref().expect("search should be open");
    assert_eq!(search.matches.first().copied(), Some(fastfetch_index));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn search_query_cursor_inserts_and_deletes_in_place() {
    let root = temp_path("cursor-edit");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.overlays.search = Some(SearchOverlay {
        scope: SearchScope::Folders,
        query: "fatch".to_string(),
        query_cursor: 2,
        candidates: Arc::new(Vec::new()),
        matches: Vec::new(),
        cached_matches: HashMap::from([(String::new(), base_cache_entry(Vec::new()))]),
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
fn search_query_ctrl_arrows_move_across_word_boundaries() {
    let root = temp_path("cursor-word-move");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.overlays.search = Some(SearchOverlay {
        scope: SearchScope::Folders,
        query: "foo bar/baz".to_string(),
        query_cursor: "foo bar/baz".chars().count(),
        candidates: Arc::new(Vec::new()),
        matches: Vec::new(),
        cached_matches: HashMap::from([(String::new(), base_cache_entry(Vec::new()))]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });

    app.handle_search_key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL))
        .expect("ctrl-left should work");
    assert_eq!(app.search_query_cursor(), 8);

    app.handle_search_key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL))
        .expect("ctrl-left should work");
    assert_eq!(app.search_query_cursor(), 4);

    app.handle_search_key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL))
        .expect("ctrl-left should work");
    assert_eq!(app.search_query_cursor(), 0);

    app.handle_search_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL))
        .expect("ctrl-right should work");
    assert_eq!(app.search_query_cursor(), 4);

    app.handle_search_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL))
        .expect("ctrl-right should work");
    assert_eq!(app.search_query_cursor(), 8);

    app.handle_search_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL))
        .expect("ctrl-right should work");
    assert_eq!(app.search_query_cursor(), 11);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn search_query_ctrl_backspace_and_delete_remove_word_units() {
    let root = temp_path("cursor-word-delete");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.overlays.search = Some(SearchOverlay {
        scope: SearchScope::Folders,
        query: "foo bar/baz".to_string(),
        query_cursor: 8,
        candidates: Arc::new(Vec::new()),
        matches: Vec::new(),
        cached_matches: HashMap::from([(String::new(), base_cache_entry(Vec::new()))]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });

    app.handle_search_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL))
        .expect("ctrl-backspace should work");
    assert_eq!(app.search_query(), "foo baz");
    assert_eq!(app.search_query_cursor(), 4);

    app.handle_search_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::CONTROL))
        .expect("ctrl-delete should work");
    assert_eq!(app.search_query(), "foo ");
    assert_eq!(app.search_query_cursor(), 4);

    app.handle_search_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::CONTROL))
        .expect("ctrl-delete at end should work");
    assert_eq!(app.search_query(), "foo ");
    assert_eq!(app.search_query_cursor(), 4);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn search_query_terminal_fallback_word_delete_bindings_work() {
    let root = temp_path("cursor-word-delete-fallbacks");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.overlays.search = Some(SearchOverlay {
        scope: SearchScope::Folders,
        query: "foo bar/baz".to_string(),
        query_cursor: 8,
        candidates: Arc::new(Vec::new()),
        matches: Vec::new(),
        cached_matches: HashMap::from([(String::new(), base_cache_entry(Vec::new()))]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });

    app.handle_search_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL))
        .expect("ctrl-h should work as a backspace fallback");
    assert_eq!(app.search_query(), "foo baz");
    assert_eq!(app.search_query_cursor(), 4);

    app.handle_search_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL))
        .expect("ctrl-d should be ignored");
    assert_eq!(app.search_query(), "foo baz");
    assert_eq!(app.search_query_cursor(), 4);

    if let Some(search) = &mut app.overlays.search {
        search.query = "foo bar baz".to_string();
        search.query_cursor = 4;
    }

    app.handle_search_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT))
        .expect("alt-d should work as a delete fallback");
    assert_eq!(app.search_query(), "foo baz");
    assert_eq!(app.search_query_cursor(), 4);

    if let Some(search) = &mut app.overlays.search {
        search.query = "foo bar".to_string();
        search.query_cursor = 7;
    }

    app.handle_search_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))
        .expect("ctrl-w should work as a backward word delete fallback");
    assert_eq!(app.search_query(), "foo ");
    assert_eq!(app.search_query_cursor(), 4);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn search_rows_ignore_stale_match_indexes() {
    let root = temp_path("stale-match-indexes");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.overlays.search = Some(SearchOverlay {
        scope: SearchScope::Folders,
        query: String::new(),
        query_cursor: 0,
        candidates: Arc::new(Vec::new()),
        matches: vec![3],
        cached_matches: HashMap::from([(String::new(), base_cache_entry(vec![3]))]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });

    assert!(app.search_rows(10).is_empty());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_search_selection_selects_file_already_in_current_directory() {
    let root = temp_path("search-select-current-file");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let alpha = root.join("alpha.txt");
    let beta = root.join("beta.txt");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(alpha.as_path())
    );
    app.overlays.search = Some(SearchOverlay {
        scope: SearchScope::Files,
        query: "beta".to_string(),
        query_cursor: 4,
        candidates: Arc::new(vec![crate::fs::search::SearchCandidate {
            path: beta.clone(),
            name: "beta.txt".to_string(),
            name_key: "beta.txt".to_string(),
            relative: "beta.txt".to_string(),
            relative_key: "beta.txt".to_string(),
            is_dir: false,
        }]),
        matches: vec![0],
        cached_matches: HashMap::from([(String::new(), base_cache_entry(vec![0]))]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });

    app.confirm_search_selection()
        .expect("search selection should succeed");

    assert!(app.overlays.search.is_none());
    assert_eq!(app.navigation.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(beta.as_path())
    );
    assert_eq!(app.status_message(), "Located beta.txt");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_search_selection_keeps_overlay_open_when_reveal_fails() {
    let root = temp_path("reveal-fails");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let missing = root.join("missing/file.txt");
    app.overlays.search = Some(SearchOverlay {
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
        cached_matches: HashMap::from([(String::new(), base_cache_entry(vec![0]))]),
        selected: 0,
        scroll: 0,
        loading: false,
        error: None,
    });

    assert!(app.confirm_search_selection().is_err());
    assert!(app.overlays.search.is_some());
    assert_eq!(app.navigation.cwd, root);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
