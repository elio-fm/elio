use super::super::*;
use super::helpers::*;
use crate::preview::default_code_preview_line_limit;

#[test]
fn wrapped_text_header_reports_visual_cap_compactly() {
    let root = temp_path("wrapped-text-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let text = root.join("long.txt");
    // At preview_cols_visible=20, "word " (5 chars) wraps 4 per line = 5_000 words → 1_250
    // wrapped lines, which exceeds default_code_preview_line_limit() of 800.
    fs::write(&text, "word ".repeat(5_000)).expect("failed to write text");

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
    assert!(header.contains(&format!(
        "first {} wrapped",
        default_code_preview_line_limit()
    )));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn narrow_code_header_prefers_compact_subtype_and_drops_low_priority_notes() {
    let root = temp_path("narrow-code-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let source = root.join("main.rs");
    // Lines are ~67 chars: fits 800 within 64 KiB (line cap hits first),
    // but 1500 lines exceed 64 KiB (file is byte-truncated overall).
    let contents = (1..=1_500)
        .map(|index| {
            format!(
                "fn line_{index}() {{ println!(\"{}\"); }}",
                "word ".repeat(8)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&source, contents).expect("failed to write source");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    // Use wait_for_preview_header to handle incremental rendering (initial
    // fast render followed by a silent extension to the full line limit).
    wait_for_preview_header(
        &mut app,
        8,
        20,
        &format!("Rust • {} shown", default_code_preview_line_limit()),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn byte_truncated_code_header_upgrades_to_exact_total_lines_after_background_count() {
    let root = temp_path("byte-truncated-code-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let source = root.join("main.rs");
    // Lines are ~67 chars: fits 800 within 64 KiB (line cap hits first),
    // but 1500 lines exceed 64 KiB (file is byte-truncated overall).
    let contents = (1..=1_500)
        .map(|index| {
            format!(
                "fn line_{index}() {{ println!(\"{}\"); }}",
                "word ".repeat(8)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&source, contents).expect("failed to write source");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    // Wait for the full extension render (incremental: initial fast paint → extension).
    wait_for_preview_header(
        &mut app,
        8,
        40,
        &format!("Rust • {} lines shown", default_code_preview_line_limit()),
    );

    wait_for_preview_header(
        &mut app,
        8,
        40,
        &format!(
            "Rust • {} / 1,500 lines shown",
            default_code_preview_line_limit()
        ),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn source_truncated_text_header_prefers_line_limit_over_wrapped_cap_note() {
    let root = temp_path("source-truncated-text-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let text = root.join("long.txt");
    let total_lines = default_code_preview_line_limit() + 40;
    // Short lines so all total_lines fit within 64 KiB (no byte truncation),
    // but total_lines exceeds the line cap so line truncation fires.
    let contents = (1..=total_lines)
        .map(|index| format!("line {index} {}", "word ".repeat(3)))
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

    assert!(header.contains(&format!("{total_lines} lines")));
    assert!(header.contains(&format!(
        "showing first {} lines",
        default_code_preview_line_limit()
    )));
    assert!(!header.contains("wrapped"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
