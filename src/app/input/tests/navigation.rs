use super::super::*;
use super::helpers::{temp_path, wait_for_directory_load};
use std::fs;

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
    assert_eq!(app.status_message(), "Press Enter to open files");

    fs::remove_dir_all(root).expect("failed to remove temp root");
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

    fs::remove_dir_all(root).expect("failed to remove temp root");
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

    fs::remove_dir_all(root).expect("failed to remove temp root");
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

    fs::remove_dir_all(root).expect("failed to remove temp root");
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

    fs::remove_dir_all(root).expect("failed to remove temp root");
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

    fs::remove_dir_all(root).expect("failed to remove temp root");
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

    fs::remove_dir_all(root).expect("failed to remove temp root");
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

    fs::remove_dir_all(root).expect("failed to remove temp root");
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

    fs::remove_dir_all(root).expect("failed to remove temp root");
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

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
