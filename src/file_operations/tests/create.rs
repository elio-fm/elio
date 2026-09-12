use super::mutation_test_support::*;

#[test]
fn confirm_create_creates_files_and_folders_and_reselects_last_created_path() {
    let root = temp_path("create-success");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.open_create_prompt();
    let overlay = app
        .file_operations
        .create
        .as_mut()
        .expect("create overlay should be open");
    overlay.lines = vec!["notes.txt".to_string(), "/docs/".to_string()];
    overlay.line_errors = vec![None; overlay.lines.len()];

    app.confirm_create().expect("create should succeed");

    assert!(app.file_operations.create.is_none());
    assert!(root.join("notes.txt").is_file());
    assert!(root.join("docs").is_dir());

    let (status, reselect_path) = take_pending_status(&mut app);
    assert_eq!(status, "Created 1 file and 1 folder");
    assert_eq!(reselect_path, Some(root.join("docs")));

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_create_reports_duplicate_names_after_dir_marker_normalization() {
    let root = temp_path("create-duplicates");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.open_create_prompt();
    let overlay = app
        .file_operations
        .create
        .as_mut()
        .expect("create overlay should be open");
    overlay.lines = vec!["logs/".to_string(), "/logs".to_string()];
    overlay.line_errors = vec![None; overlay.lines.len()];

    app.confirm_create()
        .expect("create validation should succeed");

    let overlay = app
        .file_operations
        .create
        .as_ref()
        .expect("create overlay should stay open");
    assert_eq!(overlay.cursor_line, 1);
    assert_eq!(
        overlay.line_errors[1].as_deref(),
        Some("\"logs\" appears more than once")
    );
    assert!(!root.join("logs").exists());
    assert!(app.file_browser.directory_runtime.pending_load.is_none());

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
