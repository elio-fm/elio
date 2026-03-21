use super::super::*;
use super::helpers::*;

#[test]
fn wrapped_text_header_reports_visual_cap_compactly() {
    let root = temp_path("wrapped-text-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let text = root.join("long.txt");
    fs::write(&text, "word ".repeat(2_000)).expect("failed to write text");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_frame_state(FrameState {
        preview_rows_visible: 8,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    let header = app
        .preview_header_detail(8)
        .expect("header detail should be present");

    assert!(header.contains("1 lines"));
    assert!(header.contains("first 240 wrapped"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn narrow_code_header_prefers_compact_subtype_and_drops_low_priority_notes() {
    let root = temp_path("narrow-code-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let source = root.join("main.rs");
    let contents = (1..=1_500)
        .map(|index| {
            format!(
                "fn line_{index}() {{ println!(\"{}\"); }}",
                "word ".repeat(20)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&source, contents).expect("failed to write source");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);
    let header = app
        .preview_header_detail_for_width(8, 20)
        .expect("header detail should be present");

    assert_eq!(header, "Rust • 240 shown");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn byte_truncated_code_header_upgrades_to_exact_total_lines_after_background_count() {
    let root = temp_path("byte-truncated-code-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let source = root.join("main.rs");
    let contents = (1..=1_500)
        .map(|index| {
            format!(
                "fn line_{index}() {{ println!(\"{}\"); }}",
                "word ".repeat(20)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&source, contents).expect("failed to write source");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);
    assert_eq!(
        app.preview_header_detail_for_width(8, 40).as_deref(),
        Some("Rust • 240 lines shown")
    );

    wait_for_preview_header(&mut app, 8, 40, "Rust • 240 / 1,500 lines shown");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn source_truncated_text_header_prefers_line_limit_over_wrapped_cap_note() {
    let root = temp_path("source-truncated-text-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let text = root.join("long.txt");
    let contents = (1..=300)
        .map(|index| format!("line {index} {}", "word ".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&text, contents).expect("failed to write text");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_frame_state(FrameState {
        preview_rows_visible: 8,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    let header = app
        .preview_header_detail(8)
        .expect("header detail should be present");

    assert!(header.contains("300 lines"));
    assert!(header.contains("showing first 240 lines"));
    assert!(!header.contains("wrapped"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
