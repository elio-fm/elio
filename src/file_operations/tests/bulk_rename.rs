use super::mutation_test_support::*;

#[test]
fn confirm_bulk_rename_renames_changed_entries_and_skips_unchanged_rows() {
    let root = temp_path("bulk-rename-success");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("alpha.txt"), "alpha").expect("failed to write alpha");
    fs::write(root.join("beta.txt"), "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser
        .selected_paths
        .insert(root.join("alpha.txt"));
    app.file_browser
        .selected_paths
        .insert(root.join("beta.txt"));
    app.open_bulk_rename_prompt();

    let overlay = app
        .file_operations
        .bulk_rename
        .as_mut()
        .expect("bulk rename overlay should be open");
    assert_eq!(overlay.new_names, vec!["alpha.txt", "beta.txt"]);
    overlay.new_names[0] = "gamma.txt".to_string();

    app.confirm_bulk_rename()
        .expect("bulk rename should succeed");

    assert!(app.file_operations.bulk_rename.is_none());
    assert!(root.join("gamma.txt").is_file());
    assert!(root.join("beta.txt").is_file());
    assert!(!root.join("alpha.txt").exists());
    assert!(app.file_browser.selected_paths.is_empty());

    let (status, reselect_path) = take_pending_status(&mut app);
    assert_eq!(status, "Renamed \"alpha.txt\" → \"gamma.txt\"");
    assert_eq!(reselect_path, Some(root.join("gamma.txt")));

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_bulk_rename_reports_duplicate_destination_names() {
    let root = temp_path("bulk-rename-duplicates");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("alpha.txt"), "alpha").expect("failed to write alpha");
    fs::write(root.join("beta.txt"), "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser
        .selected_paths
        .insert(root.join("alpha.txt"));
    app.file_browser
        .selected_paths
        .insert(root.join("beta.txt"));
    app.open_bulk_rename_prompt();

    let overlay = app
        .file_operations
        .bulk_rename
        .as_mut()
        .expect("bulk rename overlay should be open");
    overlay.new_names = vec!["shared.txt".to_string(), "shared.txt".to_string()];

    app.confirm_bulk_rename()
        .expect("bulk rename validation should succeed");

    let overlay = app
        .file_operations
        .bulk_rename
        .as_ref()
        .expect("bulk rename overlay should stay open");
    assert_eq!(overlay.cursor_line, 1);
    assert_eq!(
        overlay.line_errors[1].as_deref(),
        Some("\"shared.txt\" appears more than once")
    );
    assert!(root.join("alpha.txt").is_file());
    assert!(root.join("beta.txt").is_file());
    assert!(app.file_browser.directory_runtime.pending_load.is_none());

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn confirm_bulk_rename_apply_failure_keeps_review_context() {
    let root = temp_path("bulk-rename-apply-failure");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let missing = root.join("missing.txt");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(missing.clone());
    app.file_operations.bulk_rename = Some(BulkRenameOverlay {
        items: vec![BulkRenameItem {
            path: missing.clone(),
            original_name: "missing.txt".to_string(),
            is_dir: false,
        }],
        new_names: vec!["renamed.txt".to_string()],
        root: None,
        cursor_line: 0,
        cursor_col: 0,
        preferred_col: 0,
        line_errors: vec![None],
    });

    app.confirm_bulk_rename()
        .expect("apply failure should be reported as status");

    assert!(app.file_operations.bulk_rename.is_some());
    assert!(app.file_browser.selected_paths.contains(&missing));
    assert!(
        app.status_message()
            .starts_with("Could not rename \"missing.txt\":")
    );
    assert!(app.file_browser.directory_runtime.pending_load.is_none());

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn bulk_rename_uses_selection_from_multiple_directories() {
    let root = temp_path("bulk-rename-cross-directory-selection");
    let child = root.join("child");
    let alpha = root.join("alpha.txt");
    let beta = child.join("beta.txt");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(alpha.clone());
    app.file_browser.selected_paths.insert(beta.clone());
    app.open_bulk_rename_prompt();

    let overlay = app
        .file_operations
        .bulk_rename
        .as_mut()
        .expect("bulk rename overlay should be open");
    assert_eq!(overlay.new_names, vec!["alpha.txt", "beta.txt"]);
    overlay.new_names = vec![
        "alpha-renamed.txt".to_string(),
        "beta-renamed.txt".to_string(),
    ];

    app.confirm_bulk_rename()
        .expect("bulk rename should succeed");

    assert!(!alpha.exists());
    assert!(!beta.exists());
    assert!(root.join("alpha-renamed.txt").is_file());
    assert!(child.join("beta-renamed.txt").is_file());
    assert!(app.file_browser.selected_paths.is_empty());

    let (status, reselect_path) = take_pending_status(&mut app);
    assert_eq!(status, "Renamed 2 items");
    assert_eq!(reselect_path, Some(child.join("beta-renamed.txt")));

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn bulk_rename_allows_same_new_name_in_different_directories() {
    let root = temp_path("bulk-rename-same-name-different-dirs");
    let left = root.join("left");
    let right = root.join("right");
    let left_file = left.join("old.txt");
    let right_file = right.join("old.txt");
    fs::create_dir_all(&left).expect("failed to create left dir");
    fs::create_dir_all(&right).expect("failed to create right dir");
    fs::write(&left_file, "left").expect("failed to write left file");
    fs::write(&right_file, "right").expect("failed to write right file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(left_file);
    app.file_browser.selected_paths.insert(right_file);
    app.open_bulk_rename_prompt();

    let overlay = app
        .file_operations
        .bulk_rename
        .as_mut()
        .expect("bulk rename overlay should be open");
    overlay.new_names = vec!["new.txt".to_string(), "new.txt".to_string()];

    app.confirm_bulk_rename()
        .expect("bulk rename should succeed");

    assert!(app.file_operations.bulk_rename.is_none());
    assert!(left.join("new.txt").is_file());
    assert!(right.join("new.txt").is_file());

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[cfg(unix)]
#[test]
fn bulk_rename_refuses_selection_containing_trash_item() {
    let _env_guard = env_lock();
    let root = temp_path("bulk-rename-refuses-trash-selection");
    #[cfg(target_os = "macos")]
    let home = root.join("home");
    #[cfg(not(target_os = "macos"))]
    let data_home = root.join("data");
    #[cfg(target_os = "macos")]
    let trash_files = home.join(".Trash");
    #[cfg(not(target_os = "macos"))]
    let trash_files = data_home.join("Trash/files");
    let normal_dir = root.join("normal");
    let normal = normal_dir.join("normal.txt");
    let trashed = trash_files.join("trashed.txt");
    fs::create_dir_all(&trash_files).expect("failed to create trash files dir");
    fs::create_dir_all(&normal_dir).expect("failed to create normal dir");
    fs::write(&normal, "normal").expect("failed to write normal file");
    fs::write(&trashed, "trashed").expect("failed to write trashed file");

    #[cfg(target_os = "macos")]
    let _home = EnvVarGuard::set_path("HOME", &home);
    #[cfg(not(target_os = "macos"))]
    let _xdg_data_home = EnvVarGuard::set_path("XDG_DATA_HOME", &data_home);

    let mut app = App::new_at(normal_dir.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(normal);
    app.file_browser.selected_paths.insert(trashed);
    app.open_bulk_rename_prompt();

    assert!(!app.file_operations.bulk_rename_is_open());
    assert_eq!(app.status_message(), "Cannot rename items from Trash");

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn bulk_rename_selected_parent_of_current_directory_reloads_parent() {
    let root = temp_path("bulk-rename-parent-selection");
    let parent = root.join("parent");
    let child_file = parent.join("child.txt");
    fs::create_dir_all(&parent).expect("failed to create parent dir");
    fs::write(&child_file, "child").expect("failed to write child file");

    let mut app = App::new_at(parent.clone()).expect("failed to create app");
    app.file_browser.selected_paths.insert(parent.clone());
    app.open_bulk_rename_prompt();

    assert!(app.file_operations.bulk_rename_is_open());
    assert_eq!(app.file_operations.bulk_rename_item_count(), 1);

    let overlay = app
        .file_operations
        .bulk_rename
        .as_mut()
        .expect("bulk rename overlay should be open");
    overlay.new_names[0] = "renamed".to_string();

    app.confirm_bulk_rename()
        .expect("bulk rename should succeed");

    let load = app
        .file_browser
        .directory_runtime
        .pending_load
        .as_ref()
        .expect("rename should queue a directory reload");
    assert_eq!(load.target_cwd, root);
    assert_eq!(load.reselect_path, Some(root.join("renamed")));
    assert!(!parent.exists());
    assert!(root.join("renamed/child.txt").is_file());

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
