use super::super::App;
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
    std::env::temp_dir().join(format!("elio-state-{label}-{unique}"))
}

#[test]
fn startup_focus_selects_and_scrolls_entry_without_status_history_or_multi_selection() {
    let root = temp_path("startup-focus");
    fs::create_dir_all(&root).expect("temp directory should be created");
    for index in 0..8 {
        fs::write(root.join(format!("file-{index}.txt")), format!("{index}"))
            .expect("file should be created");
    }
    let target = root.join("file-6.txt");

    let app = App::new_at_startup(root.clone(), Some(target.clone()), false)
        .expect("app should initialize");

    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(target.as_path())
    );
    assert_eq!(app.file_browser.scroll_row, app.file_browser.selected);
    assert!(app.file_browser.selected_paths.is_empty());
    assert!(app.file_browser.directory_history.back.is_empty());
    assert!(app.file_browser.directory_history.forward.is_empty());
    assert_eq!(app.status_message(), "");

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn startup_focus_can_reveal_hidden_targets_without_persisted_config() {
    let root = temp_path("startup-hidden-focus");
    fs::create_dir_all(&root).expect("temp directory should be created");
    let visible = root.join("visible.txt");
    let hidden = root.join(".env");
    fs::write(&visible, "visible").expect("visible file should be created");
    fs::write(&hidden, "secret").expect("hidden file should be created");

    let app = App::new_at_startup(root.clone(), Some(hidden.clone()), true)
        .expect("app should initialize");

    assert!(app.file_browser.show_hidden);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(hidden.as_path())
    );
    assert!(
        app.file_browser
            .entries
            .iter()
            .any(|entry| entry.path == hidden)
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}
