use super::*;
use ratatui::{
    style::{Color, Style},
    text::Span,
};

#[test]
fn wrapped_preview_lines_cache_by_width() {
    let preview = PreviewContent::new(
        PreviewKind::Text,
        vec![Line::from("alpha beta gamma delta epsilon")],
    );

    let first = preview.wrapped_lines(8);
    let second = preview.wrapped_lines(8);

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(preview.visual_line_count(8), first.len());
}

#[test]
fn wrapped_preview_lines_preserve_text_and_styles() {
    let preview = PreviewContent::new(
        PreviewKind::Text,
        vec![Line::from(vec![
            Span::styled("abcdef", Style::default().fg(Color::Red)),
            Span::styled("ghij", Style::default().fg(Color::Blue)),
        ])],
    );

    let wrapped = preview.wrapped_lines(6);

    assert_eq!(wrapped.len(), 2);
    assert_eq!(wrapped[0].to_string(), "abcdef");
    assert_eq!(wrapped[1].to_string(), "ghij");
    assert_eq!(wrapped[0].spans[0].style.fg, Some(Color::Red));
    assert_eq!(wrapped[1].spans[0].style.fg, Some(Color::Blue));
}

#[test]
fn wrapped_preview_lines_cap_visual_depth() {
    let preview = PreviewContent::new(PreviewKind::Text, vec![Line::from("a ".repeat(2_000))]);

    let wrapped = preview.wrapped_lines(4);
    let expected = format!("first {PREVIEW_WRAPPED_LINE_LIMIT} wrapped");

    assert_eq!(wrapped.len(), PREVIEW_WRAPPED_LINE_LIMIT);
    assert_eq!(
        preview.wrapped_truncation_note(4).as_deref(),
        Some(expected.as_str())
    );
}

#[test]
fn preview_line_coverage_tracks_pending_and_total_counts() {
    let mut preview = PreviewContent::new(PreviewKind::Text, vec![Line::from("alpha")])
        .with_line_coverage(5, None, true);

    assert!(preview.needs_total_line_count());
    assert_eq!(
        preview.line_coverage,
        Some(PreviewLineCoverage {
            shown_lines: 5,
            total_lines: None,
            total_lines_pending: false,
            partial: true,
        })
    );

    preview.set_total_line_count_pending(true);
    assert_eq!(
        preview.line_coverage,
        Some(PreviewLineCoverage {
            shown_lines: 5,
            total_lines: None,
            total_lines_pending: true,
            partial: true,
        })
    );

    preview.apply_total_line_count(3);
    assert_eq!(
        preview.line_coverage,
        Some(PreviewLineCoverage {
            shown_lines: 5,
            total_lines: Some(5),
            total_lines_pending: false,
            partial: true,
        })
    );
    assert!(!preview.needs_total_line_count());
}
