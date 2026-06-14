use super::super::*;
use super::helpers::{
    OpenInSystemCaptureGuard, read_open_capture, temp_path, wait_for_directory_load,
};
use std::fs;

fn left_click(column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn entry_hit(index: usize, row: u16) -> EntryHit {
    EntryHit {
        rect: Rect {
            x: 0,
            y: row,
            width: 20,
            height: 1,
        },
        index,
    }
}

#[test]
fn double_click_opens_clicked_file_not_multi_selection() {
    let root = temp_path("mouse-double-click-file-selection");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let alpha = root.join("alpha.txt");
    let beta = root.join("beta.txt");
    let gamma = root.join("gamma.txt");
    fs::write(&alpha, "alpha").expect("failed to write alpha");
    fs::write(&beta, "beta").expect("failed to write beta");
    fs::write(&gamma, "gamma").expect("failed to write gamma");
    let capture = root.join("capture.txt");
    let _capture_guard = OpenInSystemCaptureGuard::install(capture.clone());

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.navigation.selected_paths.insert(alpha.clone());
    app.navigation.selected_paths.insert(gamma.clone());
    let beta_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == beta)
        .expect("beta should be visible");
    app.set_frame_state(FrameState {
        entry_hits: vec![entry_hit(beta_index, 1)],
        ..FrameState::default()
    });

    app.handle_event(left_click(1, 1))
        .expect("first click should focus clicked file");
    assert!(!capture.exists());

    app.handle_event(left_click(1, 1))
        .expect("second click should open clicked file");

    let opened = read_open_capture(&capture);
    let opened: Vec<_> = opened.lines().map(str::to_owned).collect();
    assert_eq!(opened, vec![beta.display().to_string()]);
    assert_eq!(app.status, "Opened beta.txt");

    fs::remove_dir_all(root).ok();
}

#[test]
fn double_click_enters_clicked_directory_not_multi_selection() {
    let root = temp_path("mouse-double-click-dir-selection");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let child = root.join("child");
    let selected = root.join("selected.txt");
    fs::create_dir_all(&child).expect("failed to create child dir");
    fs::write(&selected, "selected").expect("failed to write selected file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);
    app.navigation.selected_paths.insert(selected);
    let child_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == child)
        .expect("child should be visible");
    app.set_frame_state(FrameState {
        entry_hits: vec![entry_hit(child_index, 1)],
        ..FrameState::default()
    });

    app.handle_event(left_click(1, 1))
        .expect("first click should focus clicked directory");
    assert_eq!(app.navigation.cwd, root);

    app.handle_event(left_click(1, 1))
        .expect("second click should enter clicked directory");
    wait_for_directory_load(&mut app);

    assert_eq!(app.navigation.cwd, child);

    fs::remove_dir_all(root).ok();
}
