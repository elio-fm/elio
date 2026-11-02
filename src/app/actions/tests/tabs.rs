use super::super::super::App;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root() -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("elio-tabs-{unique}"));
    fs::create_dir_all(&root).expect("temporary root should be created");
    root.canonicalize().unwrap_or(root)
}

#[test]
fn new_tab_keeps_the_current_directory_and_closing_returns_to_the_previous_tab() {
    let root = temp_root();
    let mut app = App::new_at(root.clone()).expect("app should load temporary root");
    app.file_browser.show_hidden = true;
    app.new_tab().expect("new tab should open");

    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.tabs.active_index(), 1);
    assert_eq!(app.file_browser.cwd, root);
    assert!(app.file_browser.show_hidden);

    app.close_tab(1).expect("active tab should close");
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.tabs.active_index(), 0);
    assert_eq!(app.file_browser.cwd, root);

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("temporary root should be removed");
}
