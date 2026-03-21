use super::super::*;
use super::helpers::{temp_path, wait_for_background_preview};
use std::fs;

#[test]
fn preview_horizontal_scroll_works_in_list_view() {
    let root = temp_path("preview-horizontal-list");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.rs");
    fs::write(
        &file_path,
        "fn main() { let preview_line = \"this line is intentionally long for horizontal preview scrolling\"; }\n",
    )
    .expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 12,
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollRight,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll right should be handled");
    assert!(app.process_pending_scroll());
    assert_eq!(app.preview_state.horizontal_scroll, 2);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_scroll_resets_when_reselecting_a_file() {
    let root = temp_path("preview-scroll-restore");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let long = root.join("a.txt");
    let other = root.join("b.txt");
    let contents = (0..24)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&long, contents).expect("failed to write long text file");
    fs::write(&other, "short\ntext").expect("failed to write other text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 40,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.preview_state.scroll = 5;
    app.sync_preview_scroll();
    assert_eq!(app.preview_state.scroll, 5);

    app.select_index(1);
    app.select_index(0);

    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(long.as_path())
    );
    assert_eq!(app.preview_state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_horizontal_scroll_resets_when_reselecting_code() {
    let root = temp_path("preview-horizontal-restore");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let code = root.join("a.rs");
    let other = root.join("b.txt");
    fs::write(
        &code,
        "fn main() { let preview_line = \"this line is intentionally long for horizontal preview scrolling\"; }\n",
    )
    .expect("failed to write code file");
    fs::write(&other, "short\ntext").expect("failed to write other text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.view_mode = ViewMode::List;
    app.select_index(0);
    app.set_frame_state(FrameState {
        preview_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 12,
        ..FrameState::default()
    });

    app.preview_state.horizontal_scroll = 3;
    app.sync_preview_scroll();
    assert_eq!(app.preview_state.horizontal_scroll, 3);

    app.select_index(1);
    app.select_index(0);

    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(code.as_path())
    );
    assert_eq!(app.preview_state.horizontal_scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
