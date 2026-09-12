use super::super::{App, ScreenRegions};
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
    std::env::temp_dir().join(format!("elio-screen-regions-{label}-{unique}"))
}

#[test]
fn visible_row_changes_do_not_refresh_code_previews() {
    let root = temp_path("code-preview-resize");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("main.rs"), "fn main() {}\n").expect("failed to write code file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let initial_preview_token = app.preview.state.token;

    app.set_screen_regions(ScreenRegions {
        preview_rows_visible: 12,
        preview_cols_visible: 80,
        ..ScreenRegions::default()
    });

    assert_eq!(app.preview.state.token, initial_preview_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn visible_row_changes_do_not_refresh_plain_text_previews() {
    let root = temp_path("text-preview-resize");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("notes.txt"), "plain text\n").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let initial_preview_token = app.preview.state.token;

    app.set_screen_regions(ScreenRegions {
        preview_rows_visible: 12,
        preview_cols_visible: 80,
        ..ScreenRegions::default()
    });

    assert_eq!(app.preview.state.token, initial_preview_token);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
