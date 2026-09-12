use super::mutation_test_support::*;

#[cfg(unix)]
#[test]
fn editor_bulk_rename_file_preserves_selection_order() {
    let _env_guard = env_lock();
    let _visual_guard = EnvVarGuard::set_path("VISUAL", Path::new("true"));

    let root = temp_path("editor-bulk-rename-order");
    let nested = root.join("ui-workspace");
    let angular = nested.join("angular.json");
    let analysis = root.join("analysis.ipynb");
    let analytics = root.join("analytics.sqlite3");
    fs::create_dir_all(&nested).expect("failed to create nested dir");
    fs::write(&angular, "{}").expect("failed to write angular.json");
    fs::write(&analysis, "analysis").expect("failed to write analysis");
    fs::write(&analytics, "analytics").expect("failed to write analytics");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(angular.clone());
    app.file_browser.selected_paths.insert(analysis.clone());
    app.file_browser.selected_paths.insert(analytics.clone());

    app.open_editor_bulk_rename()
        .expect("editor bulk rename should open");

    let task = app
        .pending_terminal_task
        .take()
        .expect("expected pending editor task");
    let PendingTerminalTask::EditorBulkRename { session, .. } = task else {
        panic!("expected editor bulk rename task");
    };
    let edited = fs::read_to_string(&session.temp_path).expect("failed to read temp rename file");
    assert_eq!(
        edited,
        "ui-workspace/angular.json\nanalysis.ipynb\nanalytics.sqlite3\n"
    );
    let _ = fs::remove_file(&session.temp_path);

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn invalid_editor_bulk_rename_aborts_without_review_overlay() {
    let root = temp_path("editor-bulk-rename-invalid");
    let file = root.join("alpha.txt");
    fs::create_dir_all(&root).expect("failed to create root dir");
    fs::write(&file, "alpha").expect("failed to write alpha");

    let temp_file = root.join("rename.txt");
    fs::write(&temp_file, "../outside.txt\n").expect("failed to write edited rename file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(file.clone());
    let session = BulkRenameEditorSession {
        root: root.clone(),
        temp_path: temp_file.clone(),
        expected_temp_owner: None,
        items: vec![BulkRenameItem {
            path: file.clone(),
            original_name: "alpha.txt".to_string(),
            is_dir: false,
        }],
    };
    let status = std::os::unix::process::ExitStatusExt::from_raw(0);

    app.finish_editor_bulk_rename(session, Ok(status))
        .expect("editor rename finish should succeed");

    assert!(app.file_operations.bulk_rename.is_none());
    assert!(app.file_operations.editor_rename_confirm.is_none());
    assert!(app.file_browser.selected_paths.contains(&file));
    assert_eq!(
        app.status_message(),
        "Editor rename aborted: line 1: Path cannot contain . or .."
    );
    assert!(file.exists());
    assert!(!temp_file.exists());

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn finish_editor_bulk_rename_opens_confirmation_with_relative_paths() {
    let root = temp_path("editor-bulk-rename-review");
    let left = root.join("left");
    let right = root.join("right");
    let alpha = left.join("alpha.txt");
    let beta = right.join("beta.txt");
    fs::create_dir_all(&left).expect("failed to create left dir");
    fs::create_dir_all(&right).expect("failed to create right dir");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");

    let temp_file = root.join("rename.txt");
    fs::write(
        &temp_file,
        "left/renamed-alpha.txt\nright/renamed-beta.txt\n",
    )
    .expect("failed to write edited rename file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let session = BulkRenameEditorSession {
        root: root.clone(),
        temp_path: temp_file.clone(),
        expected_temp_owner: None,
        items: vec![
            BulkRenameItem {
                path: alpha.clone(),
                original_name: "alpha.txt".to_string(),
                is_dir: false,
            },
            BulkRenameItem {
                path: beta.clone(),
                original_name: "beta.txt".to_string(),
                is_dir: false,
            },
        ],
    };
    let status = std::os::unix::process::ExitStatusExt::from_raw(0);

    app.finish_editor_bulk_rename(session, Ok(status))
        .expect("editor rename finish should succeed");

    let overlay = app
        .file_operations
        .editor_rename_confirm
        .as_ref()
        .expect("editor rename confirmation should open");
    assert_eq!(overlay.root, root);
    assert_eq!(
        overlay.new_names,
        vec!["left/renamed-alpha.txt", "right/renamed-beta.txt"]
    );
    assert_eq!(
        app.file_operations.editor_rename_confirm_title(),
        "Confirm 2 renames?"
    );
    assert!(app.file_operations.editor_rename_confirmed());
    assert_eq!(app.status_message(), "");
    assert!(!temp_file.exists());

    app.duplicate_finder.session = Some(DuplicateFinderSession {
        cwd: root.clone(),
        groups: vec![crate::duplicate_finder::DuplicateGroup {
            id: 1,
            size: 5,
            files: vec![
                crate::duplicate_finder::DuplicateFile {
                    path: alpha.clone(),
                    name: "alpha.txt".to_string(),
                    relative: "left/alpha.txt".to_string(),
                    size: 5,
                    modified: None,
                },
                crate::duplicate_finder::DuplicateFile {
                    path: beta.clone(),
                    name: "beta.txt".to_string(),
                    relative: "right/beta.txt".to_string(),
                    size: 5,
                    modified: None,
                },
            ],
        }],
        stats: crate::duplicate_finder::DuplicateScanStats::default(),
        selected: 0,
        scroll: 0,
        selected_paths: [beta.clone()].into_iter().collect(),
        loading: false,
        partial: false,
        error: None,
        preview_visible: true,
        preview_path: Some(alpha.clone()),
    });

    app.confirm_editor_rename()
        .expect("confirmation should rename files");
    assert!(left.join("renamed-alpha.txt").is_file());
    assert!(right.join("renamed-beta.txt").is_file());
    assert!(!alpha.exists());
    assert!(!beta.exists());

    let duplicate_overlay = app
        .duplicate_finder
        .session
        .as_ref()
        .expect("duplicate overlay should stay open");
    assert_eq!(
        duplicate_overlay.groups[0].files[0].path,
        left.join("renamed-alpha.txt")
    );
    assert_eq!(
        duplicate_overlay.groups[0].files[0].name,
        "renamed-alpha.txt"
    );
    assert_eq!(
        duplicate_overlay.groups[0].files[0].relative,
        "left/renamed-alpha.txt"
    );
    assert!(
        duplicate_overlay
            .selected_paths
            .contains(&right.join("renamed-beta.txt"))
    );
    assert!(!duplicate_overlay.selected_paths.contains(&beta));
    assert_eq!(
        duplicate_overlay.preview_path,
        Some(left.join("renamed-alpha.txt"))
    );

    let (status, reselect_path) = take_pending_status(&mut app);
    assert_eq!(status, "Renamed 2 items");
    assert_eq!(reselect_path, Some(right.join("renamed-beta.txt")));

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
