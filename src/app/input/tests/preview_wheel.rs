use super::super::*;
use super::helpers::{
    temp_path, wait_for_background_preview, write_binary_zip_entries, write_epub_fixture,
};
use std::{fs, thread, time::Duration};

#[test]
fn high_frequency_preview_wheel_scrolls_preview_after_entries_scroll() {
    let root = temp_path("wheel-hf-preview-after-entries");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), name).expect("failed to write temp file");
    }
    let long_file = root.join("long.txt");
    let contents = (0..60)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&long_file, &contents).expect("failed to write long file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    let long_index = app
        .navigation
        .entries
        .iter()
        .position(|e| e.path == long_file)
        .expect("long.txt should be in entries");
    app.select_index(long_index);

    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_panel: Some(Rect {
            x: 40,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_rows_visible: 16,
        preview_cols_visible: 38,
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 8,
        },
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("entry scroll should be handled");
    assert_eq!(app.input.last_wheel_target, Some(WheelTarget::Entries));

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 45,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("hover on preview should be handled");
    assert_eq!(app.input.last_wheel_target, Some(WheelTarget::Preview));

    let before_scroll = app.preview.state.scroll;
    let before_selected = app.navigation.selected;
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 45,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview scroll should be handled");

    assert_eq!(
        app.navigation.selected, before_selected,
        "entry selection must not change when scrolling preview"
    );
    assert!(
        app.preview.state.scroll > before_scroll,
        "preview must have scrolled"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn high_frequency_preview_wheel_scrolls_preview_without_prior_moved_event() {
    let root = temp_path("wheel-hf-preview-no-moved");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let long_file = root.join("long.txt");
    let contents = (0..60)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&long_file, &contents).expect("failed to write long file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.input.last_wheel_target = Some(WheelTarget::Entries);

    let long_index = app
        .navigation
        .entries
        .iter()
        .position(|e| e.path == long_file)
        .expect("long.txt should be in entries");
    app.select_index(long_index);

    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_panel: Some(Rect {
            x: 40,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_rows_visible: 16,
        preview_cols_visible: 38,
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 8,
        },
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    let before_scroll = app.preview.state.scroll;
    let before_selected = app.navigation.selected;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 45,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview scroll should be handled");

    assert_eq!(
        app.navigation.selected, before_selected,
        "entry selection must not change when scrolling preview"
    );
    assert!(
        app.preview.state.scroll > before_scroll,
        "preview must have scrolled without a prior Moved event"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn hover_panel_routes_scroll_when_event_coords_are_outside_panels() {
    let root = temp_path("wheel-hover-panel-routing");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let long_file = root.join("long.txt");
    let contents = (0..60)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&long_file, &contents).expect("failed to write long file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.input.wheel_profile = WheelProfile::HighFrequency;
    app.input.last_wheel_target = Some(WheelTarget::Entries);
    app.input.hover_panel = None;

    let long_index = app
        .navigation
        .entries
        .iter()
        .position(|e| e.path == long_file)
        .expect("long.txt should be in entries");
    app.select_index(long_index);

    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_panel: Some(Rect {
            x: 40,
            y: 0,
            width: 40,
            height: 20,
        }),
        preview_rows_visible: 16,
        preview_cols_visible: 38,
        metrics: ViewMetrics {
            cols: 1,
            rows_visible: 8,
        },
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 45,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("moved event should be handled");
    assert_eq!(app.input.hover_panel, Some(WheelTarget::Preview));

    let before_scroll = app.preview.state.scroll;
    let before_selected = app.navigation.selected;
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("scroll should be handled");

    assert_eq!(
        app.navigation.selected, before_selected,
        "entry selection must not change"
    );
    assert!(
        app.preview.state.scroll > before_scroll,
        "hover_panel should have routed scroll to preview despite wrong coords"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_uses_last_focused_panel_when_coordinates_miss() {
    let root = temp_path("preview-wheel-focus");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.txt");
    let contents = (0..40)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&file_path, contents).expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    let file_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == file_path)
        .expect("long file should be visible");
    app.select_index(file_index);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview click should be handled");
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 80,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("wheel should fall back to last focused preview panel");

    assert!(app.process_pending_scroll());
    assert!(app.preview.state.scroll > 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_follows_hovered_panel_without_click() {
    let root = temp_path("preview-wheel-hover");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.txt");
    let contents = (0..40)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&file_path, contents).expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    let file_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == file_path)
        .expect("long file should be visible");
    app.select_index(file_index);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview hover should be handled");
    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 80,
        row: 20,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("wheel should fall back to hovered preview panel");

    assert!(app.process_pending_scroll());
    assert!(app.preview.state.scroll > 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_uses_preview_column_when_row_is_unreliable() {
    let root = temp_path("preview-wheel-column-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let file_path = root.join("long.txt");
    let contents = (0..40)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&file_path, contents).expect("failed to write temp file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    let file_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == file_path)
        .expect("long file should be visible");
    app.select_index(file_index);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 20,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("wheel should use preview column fallback");

    assert!(app.process_pending_scroll());
    assert!(app.preview.state.scroll > 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_steps_comic_pages_instead_of_scrolling_summary_text() {
    let root = temp_path("preview-wheel-comic-pages");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    write_binary_zip_entries(
        &archive,
        &[
            ("1.jpg", b"page-one"),
            ("2.jpg", b"page-two"),
            ("3.jpg", b"page-three"),
        ],
    );

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    let archive_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == archive)
        .expect("archive should be visible");
    app.select_index(archive_index);
    wait_for_background_preview(&mut app);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 20,
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should be handled");

    assert_eq!(
        app.preview
            .state
            .content
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert!(app.pending_preview_refresh_timer().is_some());
    assert_eq!(app.preview.state.scroll, 0);

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    wait_for_background_preview(&mut app);

    assert_eq!(
        app.preview
            .state
            .content
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert_eq!(app.preview.state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_preview_wheel_clears_pending_entry_scroll_before_page_turns() {
    let root = temp_path("preview-wheel-comic-clears-entry-scroll");
    fs::create_dir_all(&root).expect("failed to create temp root");
    write_binary_zip_entries(
        &root.join("a.cbz"),
        &[("1.jpg", b"a-one"), ("2.jpg", b"a-two")],
    );
    fs::write(root.join("b.txt"), "next entry").expect("failed to write temp text");
    fs::write(root.join("c.txt"), "another entry").expect("failed to write temp text");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);
    wait_for_background_preview(&mut app);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    app.input.wheel_scroll.vertical.pending = 3;

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should be handled");

    assert_eq!(app.navigation.selected, 0);
    assert_eq!(app.input.wheel_scroll.vertical.pending, 0);
    assert_eq!(
        app.current_preview_request_options().comic_page_index(),
        Some(1)
    );

    let _ = app.process_pending_scroll();
    assert_eq!(app.navigation.selected, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_steps_cbr_pages_instead_of_scrolling_summary_text() {
    let root = temp_path("preview-wheel-cbr-pages");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbr");
    write_binary_zip_entries(
        &archive,
        &[
            ("1.jpg", b"page-one"),
            ("2.jpg", b"page-two"),
            ("3.jpg", b"page-three"),
        ],
    );

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);
    wait_for_background_preview(&mut app);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_rows_visible: 6,
        preview_cols_visible: 20,
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should be handled");

    assert_eq!(
        app.preview
            .state
            .content
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert!(app.pending_preview_refresh_timer().is_some());
    assert_eq!(app.preview.state.scroll, 0);

    thread::sleep(HIGH_FREQUENCY_PREVIEW_REFRESH_DELAY + Duration::from_millis(20));
    assert!(app.process_preview_refresh_timers());
    wait_for_background_preview(&mut app);

    assert_eq!(
        app.preview
            .state
            .content
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert_eq!(app.preview.state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_scrolls_epub_section_before_advancing_to_next_section() {
    let root = temp_path("preview-wheel-epub-sections");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("story.epub");
    let long_body = (1..=30)
        .map(|index| format!("<p>Paragraph {index} {} </p>", "word ".repeat(20)))
        .collect::<Vec<_>>()
        .join("");
    write_epub_fixture(
        &archive,
        &[
            ("Opening", long_body.as_str()),
            ("Second Step", "<p>Second chapter text.</p>"),
        ],
    );

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    app.select_index(0);
    wait_for_background_preview(&mut app);
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 24,
            height: 8,
        }),
        preview_rows_visible: 4,
        preview_cols_visible: 24,
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should be handled");

    assert!(app.process_pending_scroll());
    assert!(app.preview.state.scroll > 0);
    assert_eq!(app.preview.state.content.ebook_section_index, Some(0));

    let max_scroll = app
        .preview_total_lines(app.input.frame_state.preview_cols_visible.max(1))
        .saturating_sub(app.input.frame_state.preview_rows_visible.max(1));
    app.preview.state.scroll = max_scroll;
    app.sync_preview_scroll();

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should advance the section at the bottom boundary");

    assert_eq!(app.preview.state.scroll, 0);
    assert_eq!(app.preview.state.content.ebook_section_index, Some(1));
    assert_eq!(app.preview.state.content.ebook_section_count, Some(2));
    assert!(
        app.preview_header_detail(10)
            .as_deref()
            .is_some_and(|detail| detail.contains("Section 2/2"))
    );

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview.state.content.ebook_section_index, Some(1));
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("Second chapter text."))
    );
    assert_eq!(app.preview.state.scroll, 0);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_wheel_advances_full_height_epub_image_without_hidden_scroll() {
    let root = temp_path("preview-wheel-epub-full-height-image");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("story.epub");
    fs::write(&archive, b"epub placeholder").expect("failed to write epub placeholder");
    let page = root.join("cover.png");
    fs::write(&page, b"image placeholder").expect("failed to write image placeholder");
    let page_size = fs::metadata(&page)
        .expect("page metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.navigation.view_mode = ViewMode::List;
    let epub_index = app
        .navigation
        .entries
        .iter()
        .position(|entry| entry.path == archive)
        .expect("epub should be visible");
    app.select_index(epub_index);
    app.sync_epub_preview_selection();
    app.preview.state.content = crate::preview::PreviewContent::new(
        crate::preview::PreviewKind::Document,
        vec![ratatui::text::Line::from("Hidden cover context")],
    )
    .with_ebook_section(0, 2, Some("Cover".to_string()))
    .with_preview_visual(crate::preview::PreviewVisual {
        kind: crate::preview::PreviewVisualKind::PageImage,
        layout: crate::preview::PreviewVisualLayout::FullHeight,
        path: page,
        size: page_size,
        modified: None,
    });
    app.apply_current_epub_preview_metadata();
    app.set_frame_state(FrameState {
        entries_panel: Some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        }),
        preview_panel: Some(Rect {
            x: 21,
            y: 0,
            width: 24,
            height: 8,
        }),
        preview_rows_visible: 0,
        preview_cols_visible: 24,
        ..FrameState::default()
    });

    app.handle_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 22,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }))
    .expect("preview wheel should be handled");

    assert_eq!(app.preview.state.scroll, 0);
    assert_eq!(app.preview.state.content.ebook_section_index, Some(1));
    assert_eq!(app.preview.state.content.ebook_section_count, Some(2));
    assert!(
        app.preview_header_detail(10)
            .as_deref()
            .is_some_and(|detail| detail.contains("Section 2/2"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
