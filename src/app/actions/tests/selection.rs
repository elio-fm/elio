use super::helpers::temp_path;
use crate::app::App;
use std::fs;

#[test]
fn selection_summary_is_compact_for_files() {
    let root = temp_path("selection-summary-file");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("note.txt"), "hello").expect("failed to write file");

    let app = App::new_at(root.clone()).expect("failed to create app");
    assert_eq!(app.selection_summary(), "1/1  note.txt");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn selection_summary_marks_directories_with_trailing_slash() {
    let root = temp_path("selection-summary-dir");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create temp dirs");

    let app = App::new_at(root.clone()).expect("failed to create app");
    assert_eq!(app.selection_summary(), "1/1  child/");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
