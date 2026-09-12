use super::super::preview_header::*;

fn header_segment(weight: u32, full: &str, compact: Option<&str>) -> PreviewHeaderSegment {
    PreviewHeaderSegment::new(weight, full.to_string(), compact.map(str::to_string))
}

#[test]
fn fitted_preview_header_prefers_compact_type_and_drops_auxiliary_notes() {
    let detail = header_segment(HEADER_SCORE_DETAIL, "Rust source file", Some("Rust"));
    let lines = header_segment(HEADER_SCORE_CONTEXT, "300 lines", Some("300l"));
    let truncated = header_segment(
        HEADER_SCORE_AUXILIARY,
        "truncated to 64 KiB",
        Some("64 KiB cap"),
    );

    let fitted = fit_preview_header_segments(&[detail, lines, truncated], 20);

    assert_eq!(fitted.as_deref(), Some("Rust • 300 lines"));
}

#[test]
fn fitted_preview_header_keeps_navigation_before_optional_title() {
    let navigation = header_segment(HEADER_SCORE_NAVIGATION, "Section 2/14", Some("2/14"));
    let detail = header_segment(HEADER_SCORE_DETAIL, "EPUB ebook", Some("EPUB"));
    let title = header_segment(
        HEADER_SCORE_TITLE,
        "The Boy From The Wastes",
        Some("The Boy From The…"),
    );

    let fitted = fit_preview_header_segments(&[navigation, detail, title], 14);

    assert_eq!(fitted.as_deref(), Some("2/14 • EPUB"));
}

#[test]
fn compact_preview_header_note_shortens_common_truncation_phrases() {
    let line_limit = crate::preview::default_code_preview_line_limit();
    let note = format!("truncated to 64 KiB  •  showing first {line_limit} lines");
    let expected = format!("64 KiB cap • {line_limit}-line cap");
    assert_eq!(
        compact_preview_header_note(&note).as_deref(),
        Some(expected.as_str())
    );
}

#[test]
fn compact_preview_header_note_shortens_directory_items_shown() {
    let line_limit = crate::preview::default_code_preview_line_limit();
    let note = format!("{line_limit} items shown");
    let expected = format!("{line_limit} shown");
    assert_eq!(
        compact_preview_header_note(&note).as_deref(),
        Some(expected.as_str())
    );
}

#[test]
fn compact_preview_header_label_shortens_comic_rar_archive() {
    assert_eq!(
        compact_preview_header_label("Comic RAR archive").as_deref(),
        Some("CBR")
    );
}

#[test]
fn fitted_preview_header_clamps_fallback_segment_when_nothing_fits() {
    let detail = header_segment(HEADER_SCORE_DETAIL, "Rust source file", Some("Rust"));
    let line_limit = crate::preview::default_code_preview_line_limit();
    let full = format!("{line_limit} lines shown");
    let compact = format!("{line_limit} shown");
    let lines = header_segment(HEADER_SCORE_CONTEXT, full.as_str(), Some(compact.as_str()));

    let fitted = fit_preview_header_segments(&[detail, lines], 3);

    assert_eq!(fitted.as_deref(), Some("Ru…"));
}

#[test]
fn fitted_preview_header_prefers_directory_item_count_over_items_shown_when_narrow() {
    let detail = header_segment(
        HEADER_SCORE_DETAIL,
        "4,240 items • 90 MB",
        Some("4,240 items"),
    );
    let truncation = header_segment(HEADER_SCORE_AUXILIARY, "800 items shown", Some("800 shown"));

    let fitted = fit_preview_header_segments(&[detail, truncation], 11);

    assert_eq!(fitted.as_deref(), Some("4,240 items"));
}
