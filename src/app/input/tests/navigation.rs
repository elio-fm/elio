use super::super::*;
use super::helpers::{cleanup_app_temp_root, temp_path, wait_for_directory_load};
use std::{fs, path::PathBuf};

fn app_in_child_with_parent_selection(label: &str) -> (PathBuf, PathBuf, PathBuf, App) {
    let root = temp_path(label);
    let child = root.join("child");
    let selected = root.join("selected.txt");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(&selected, "selected").expect("failed to write selected file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.navigation.selected_paths.insert(selected.clone());
    app.set_dir(child.clone())
        .expect("entering child should succeed");
    wait_for_directory_load(&mut app);

    (root, child, selected, app)
}

fn assert_clear_selection_after_directory_change(label: &str, key: KeyEvent) {
    let (root, child, _, mut app) = app_in_child_with_parent_selection(label);

    assert_eq!(app.navigation.cwd, child);
    assert_eq!(app.selection_count(), 1);

    app.handle_event(Event::Key(key))
        .expect("key should clear selection");

    assert_eq!(app.selection_count(), 0);

    cleanup_app_temp_root(app, root);
}

fn folder_with_child_file(label: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = temp_path(label);
    let folder = root.join("folder");
    let child = folder.join("child.txt");
    fs::create_dir_all(&folder).expect("failed to create folder");
    fs::write(&child, "child").expect("failed to write child");
    (root, folder, child)
}

#[test]
fn right_arrow_does_not_open_selected_file_in_list_view() {
    let root = temp_path("right-file");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("note.txt");
    fs::write(&file_path, "hello").expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )))
    .expect("right arrow should be handled");

    assert_eq!(app.navigation.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(file_path.as_path())
    );
    assert_eq!(app.status_message(), "");

    cleanup_app_temp_root(app, root);
}

#[test]
fn right_arrow_enters_selected_directory_in_list_view() {
    let root = temp_path("right-dir");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create temp dirs");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )))
    .expect("right arrow should be handled");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);

    cleanup_app_temp_root(app, root);
}

#[test]
fn right_arrow_enters_focused_directory_even_when_selection_exists() {
    let root = temp_path("right-dir-with-selection");
    let child = root.join("child");
    let file = root.join("selected.txt");
    fs::create_dir_all(&child).expect("failed to create temp dirs");
    fs::write(&file, "hello").expect("failed to write selected file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.navigation.selected_paths.insert(file.clone());
    app.select_index(0);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )))
    .expect("right arrow should be handled");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);
    assert!(app.navigation.selected_paths.contains(&file));

    cleanup_app_temp_root(app, root);
}

#[test]
fn selection_persists_after_entering_and_leaving_directory() {
    let (root, child, selected, mut app) =
        app_in_child_with_parent_selection("persistent-selection-nav");

    assert_eq!(app.navigation.cwd, child);
    assert!(app.navigation.selected_paths.contains(&selected));

    app.go_parent().expect("going parent should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, root);
    assert!(app.navigation.selected_paths.contains(&selected));

    cleanup_app_temp_root(app, root);
}

#[test]
fn esc_clears_selection_after_directory_change() {
    assert_clear_selection_after_directory_change(
        "persistent-selection-esc",
        KeyEvent::from(KeyCode::Esc),
    );
}

#[test]
fn ctrl_c_clears_selection_after_directory_change() {
    assert_clear_selection_after_directory_change(
        "persistent-selection-ctrl-c",
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    );
}

#[test]
fn select_all_extends_cross_directory_selection() {
    let root = temp_path("persistent-selection-select-all");
    let child = root.join("child");
    let selected = root.join("selected.txt");
    let beta = child.join("beta.txt");
    let gamma = child.join("gamma.txt");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(&selected, "selected").expect("failed to write selected file");
    fs::write(&beta, "beta").expect("failed to write beta");
    fs::write(&gamma, "gamma").expect("failed to write gamma");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.navigation.selected_paths.insert(selected.clone());
    app.set_dir(child.clone())
        .expect("entering child should succeed");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('a'),
        KeyModifiers::CONTROL,
    )))
    .expect("Ctrl+A should select all visible entries");

    assert!(app.navigation.selected_paths.contains(&selected));
    assert!(app.navigation.selected_paths.contains(&beta));
    assert!(app.navigation.selected_paths.contains(&gamma));
    assert_eq!(app.selection_count(), 3);

    cleanup_app_temp_root(app, root);
}

#[test]
fn selecting_child_inside_selected_folder_is_blocked() {
    let (root, folder, child) = folder_with_child_file("selection-blocks-selected-parent");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    let folder_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == folder)
        .expect("folder should be visible");
    app.select_index(folder_index);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(' '))))
        .expect("space should select folder");

    app.set_dir(folder.clone())
        .expect("entering folder should succeed");
    wait_for_directory_load(&mut app);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(' '))))
        .expect("space should reject nested child selection");

    assert_eq!(app.status_message(), "Cannot select nested paths");
    assert!(app.navigation.selected_paths.contains(&folder));
    assert!(!app.navigation.selected_paths.contains(&child));
    assert_eq!(app.selection_count(), 1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(child.as_path())
    );

    cleanup_app_temp_root(app, root);
}

#[test]
fn selecting_folder_containing_selected_child_is_blocked() {
    let (root, folder, child) = folder_with_child_file("selection-blocks-selected-child");

    let mut app = App::new_at(folder.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(' '))))
        .expect("space should select child");

    app.go_parent().expect("going parent should succeed");
    wait_for_directory_load(&mut app);
    let folder_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == folder)
        .expect("folder should be visible");
    app.select_index(folder_index);
    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char(' '))))
        .expect("space should reject nested folder selection");

    assert_eq!(app.status_message(), "Cannot select nested paths");
    assert!(app.navigation.selected_paths.contains(&child));
    assert!(!app.navigation.selected_paths.contains(&folder));
    assert_eq!(app.selection_count(), 1);

    cleanup_app_temp_root(app, root);
}

#[test]
fn select_all_skips_entries_inside_selected_folder() {
    let root = temp_path("selection-select-all-skips-nested");
    let folder = root.join("folder");
    let alpha = folder.join("alpha.txt");
    let beta = folder.join("beta.txt");
    fs::create_dir_all(&folder).expect("failed to create folder");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.navigation.selected_paths.insert(folder.clone());
    app.set_dir(folder.clone())
        .expect("entering folder should succeed");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('a'),
        KeyModifiers::CONTROL,
    )))
    .expect("Ctrl+A should skip nested entries");

    assert_eq!(app.status_message(), "Cannot select nested paths");
    assert!(app.navigation.selected_paths.contains(&folder));
    assert!(!app.navigation.selected_paths.contains(&alpha));
    assert!(!app.navigation.selected_paths.contains(&beta));
    assert_eq!(app.selection_count(), 1);

    cleanup_app_temp_root(app, root);
}

#[test]
fn enter_uses_focused_entry_when_selection_is_offscreen() {
    let root = temp_path("persistent-selection-enter-offscreen");
    let child = root.join("child");
    let grandchild = child.join("grandchild");
    let selected = root.join("selected.txt");
    fs::create_dir_all(&grandchild).expect("failed to create grandchild dir");
    fs::write(&selected, "selected").expect("failed to write selected file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.navigation.selected_paths.insert(selected.clone());
    app.set_dir(child.clone())
        .expect("entering child should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);
    assert!(app.navigation.selected_paths.contains(&selected));
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(grandchild.as_path())
    );

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .expect("Enter should open focused directory");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, grandchild);
    assert!(app.navigation.selected_paths.contains(&selected));

    cleanup_app_temp_root(app, root);
}

#[test]
fn enter_uses_focused_directory_when_selection_is_visible() {
    let root = temp_path("persistent-selection-enter-visible-directory");
    let child = root.join("child");
    let selected = root.join("selected.txt");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(&selected, "selected").expect("failed to write selected file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.navigation.selected_paths.insert(selected.clone());
    let child_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == child)
        .expect("child should be visible");
    app.select_index(child_index);

    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .expect("Enter should enter focused directory");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);
    assert!(app.navigation.selected_paths.contains(&selected));

    cleanup_app_temp_root(app, root);
}

#[test]
fn left_arrow_in_list_view_reselects_previous_directory_in_parent() {
    let root = temp_path("left-parent-selection");
    let alpha = root.join("alpha");
    let child = root.join("child");
    fs::create_dir_all(&alpha).expect("failed to create alpha dir");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(1);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)))
        .expect("left arrow should be handled");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(child.as_path())
    );

    cleanup_app_temp_root(app, root);
}

#[test]
fn go_back_reselects_previous_directory_in_parent() {
    let root = temp_path("history-back-selection");
    let alpha = root.join("alpha");
    let child = root.join("child");
    fs::create_dir_all(&alpha).expect("failed to create alpha dir");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(1);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.go_back().expect("go back should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, root);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(child.as_path())
    );

    cleanup_app_temp_root(app, root);
}

#[test]
fn go_forward_reselects_previous_directory_in_parent() {
    let root = temp_path("history-forward-selection");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);
    app.go_back().expect("go back should succeed");
    wait_for_directory_load(&mut app);

    app.go_forward().expect("go forward should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);
    assert!(app.selected_entry().is_none());

    cleanup_app_temp_root(app, root);
}

#[test]
fn go_forward_restores_last_selected_entry_in_directory() {
    let root = temp_path("history-forward-restore-selection");
    let child = root.join("child");
    let alpha = child.join("alpha.txt");
    let beta = child.join("beta.txt");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(beta.as_path())
    );

    app.go_back().expect("go back should succeed");
    wait_for_directory_load(&mut app);

    app.go_forward().expect("go forward should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(beta.as_path())
    );

    cleanup_app_temp_root(app, root);
}

#[test]
fn reopening_directory_restores_last_selected_entry() {
    let root = temp_path("reopen-directory-selection");
    let child = root.join("child");
    let alpha = child.join("alpha.txt");
    let beta = child.join("beta.txt");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(beta.as_path())
    );

    app.go_parent().expect("go parent should succeed");
    wait_for_directory_load(&mut app);
    app.open_selected()
        .expect("reopening selected directory should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(beta.as_path())
    );

    cleanup_app_temp_root(app, root);
}

#[test]
fn reopening_directory_restores_scroll_position() {
    let root = temp_path("reopen-directory-scroll");
    let child = root.join("child");
    fs::create_dir_all(&child).expect("failed to create child dir");
    for index in 0..8 {
        fs::write(child.join(format!("file-{index}.txt")), format!("{index}"))
            .expect("failed to write file");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.set_frame_state(FrameState {
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 3,
        },
        ..FrameState::default()
    });
    app.select_index(0);
    app.open_selected()
        .expect("opening selected directory should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(6);
    assert_eq!(app.navigation.scroll_row, 4);

    app.go_parent().expect("go parent should succeed");
    wait_for_directory_load(&mut app);
    app.open_selected()
        .expect("reopening selected directory should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);
    assert_eq!(app.navigation.selected, 6);
    assert_eq!(app.navigation.scroll_row, 4);

    cleanup_app_temp_root(app, root);
}

#[test]
fn reopening_parent_restores_last_selected_child_directory() {
    let root = temp_path("reopen-parent-selection");
    let home = root.join("home");
    let aaa = home.join("aaa");
    let regueiro = home.join("regueiro");
    fs::create_dir_all(&aaa).expect("failed to create aaa dir");
    fs::create_dir_all(&regueiro).expect("failed to create regueiro dir");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);
    app.open_selected().expect("opening home should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(regueiro.as_path())
    );

    app.open_selected()
        .expect("opening regueiro should succeed");
    wait_for_directory_load(&mut app);
    app.go_parent().expect("go parent to home should succeed");
    wait_for_directory_load(&mut app);
    app.go_parent().expect("go parent to root should succeed");
    wait_for_directory_load(&mut app);

    app.open_selected().expect("reopening home should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, home);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(regueiro.as_path())
    );

    cleanup_app_temp_root(app, root);
}

#[test]
fn reopening_parent_restores_scroll_position() {
    let root = temp_path("reopen-parent-scroll");
    let home = root.join("home");
    let child_paths = (0..8)
        .map(|index| home.join(format!("child-{index}")))
        .collect::<Vec<_>>();
    for child in &child_paths {
        fs::create_dir_all(child).expect("failed to create child dir");
    }

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.set_frame_state(FrameState {
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 3,
        },
        ..FrameState::default()
    });
    app.select_index(0);
    app.open_selected().expect("opening home should succeed");
    wait_for_directory_load(&mut app);

    app.select_index(6);
    assert_eq!(app.navigation.scroll_row, 4);

    app.open_selected()
        .expect("opening remembered child should succeed");
    wait_for_directory_load(&mut app);
    app.go_parent().expect("go parent to home should succeed");
    wait_for_directory_load(&mut app);
    app.go_parent().expect("go parent to root should succeed");
    wait_for_directory_load(&mut app);

    app.open_selected().expect("reopening home should succeed");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, home);
    assert_eq!(app.navigation.selected, 6);
    assert_eq!(app.navigation.scroll_row, 4);

    cleanup_app_temp_root(app, root);
}
