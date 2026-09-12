use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-local-filter-{label}-{unique}"))
}

fn entry_names(app: &App) -> Vec<String> {
    app.file_browser
        .entries
        .iter()
        .map(|entry| entry.name.clone())
        .collect()
}

#[test]
fn local_filter_hides_non_matching_current_directory_entries() {
    let root = temp_path("matches");
    fs::create_dir_all(root.join("src")).expect("directory should be created");
    fs::write(root.join("src-main.rs"), "").expect("file should be created");
    fs::write(root.join("readme.md"), "").expect("file should be created");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    let all_count = app.file_browser.entries.len();
    app.open_local_filter();
    app.handle_local_filter_key(KeyEvent::from(KeyCode::Char('s')))
        .expect("filter input should succeed");
    app.handle_local_filter_key(KeyEvent::from(KeyCode::Char('r')))
        .expect("filter input should succeed");
    app.handle_local_filter_key(KeyEvent::from(KeyCode::Char('c')))
        .expect("filter input should succeed");

    assert_eq!(
        entry_names(&app),
        vec!["src".to_string(), "src-main.rs".to_string()]
    );
    assert_eq!(app.file_browser.unfiltered_entries.len(), all_count);
    assert_eq!(
        app.selected_entry().map(|entry| entry.name.as_str()),
        Some("src")
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn local_filter_preserves_matching_selection_and_only_prompt_escape_restores_entries() {
    let root = temp_path("selection");
    fs::create_dir_all(&root).expect("directory should be created");
    fs::write(root.join("alpha.txt"), "").expect("file should be created");
    fs::write(root.join("beta.txt"), "").expect("file should be created");
    fs::write(root.join("alphabet.txt"), "").expect("file should be created");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    let beta_index = app
        .file_browser
        .entries
        .iter()
        .position(|entry| entry.name == "beta.txt")
        .expect("beta should exist");
    app.set_selected(beta_index);
    app.open_local_filter();
    for ch in "beta".chars() {
        app.handle_local_filter_key(KeyEvent::from(KeyCode::Char(ch)))
            .expect("filter input should succeed");
    }

    assert_eq!(entry_names(&app), vec!["beta.txt".to_string()]);
    assert_eq!(
        app.selected_entry().map(|entry| entry.name.as_str()),
        Some("beta.txt")
    );

    app.handle_local_filter_key(KeyEvent::from(KeyCode::Enter))
        .expect("enter should leave filter editing mode");
    assert!(!app.local_filter_is_editing());
    assert_eq!(app.local_filter_query(), "beta");
    assert_eq!(entry_names(&app), vec!["beta.txt".to_string()]);

    app.handle_event(crossterm::event::Event::Key(KeyEvent::from(KeyCode::Esc)))
        .expect("normal escape should keep inactive filter");
    assert!(!app.local_filter_is_editing());
    assert_eq!(app.local_filter_query(), "beta");
    assert_eq!(entry_names(&app), vec!["beta.txt".to_string()]);

    app.open_local_filter();
    app.handle_local_filter_key(KeyEvent::from(KeyCode::Esc))
        .expect("prompt escape should clear filter");
    assert!(!app.local_filter_is_editing());
    assert_eq!(app.local_filter_query(), "");
    assert_eq!(app.file_browser.entries.len(), 3);

    fs::remove_dir_all(root).expect("temp directory should be removed");
}
