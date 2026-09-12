use super::mutation_test_support::*;

#[test]
fn confirm_rename_renames_selected_entry_and_queues_reselect() {
    let root = temp_path("rename-success");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("report.txt"), "draft").expect("failed to write source file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.open_rename_prompt();
    let overlay = app
        .file_operations
        .rename
        .as_mut()
        .expect("rename overlay should be open");
    assert_eq!(overlay.original_name, "report.txt");
    assert_eq!(overlay.cursor_col, 6);
    overlay.input = "summary.txt".to_string();

    app.confirm_rename().expect("rename should succeed");

    assert!(app.file_operations.rename.is_none());
    assert!(!root.join("report.txt").exists());
    assert!(root.join("summary.txt").is_file());

    let (status, reselect_path) = take_pending_status(&mut app);
    assert_eq!(status, "Renamed \"report.txt\" → \"summary.txt\"");
    assert_eq!(reselect_path, Some(root.join("summary.txt")));

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cursor_before_extension_skips_hidden_file_prefix_dot() {
    assert_eq!(rename::cursor_before_extension(".env"), 4);
    assert_eq!(rename::cursor_before_extension("report.txt"), 6);
    assert_eq!(rename::cursor_before_extension("archive.tar.gz"), 11);
}
