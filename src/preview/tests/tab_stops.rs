use super::*;
use ratatui::style::Color;

#[test]
fn tabs_advance_to_the_next_stop_using_display_columns() {
    for (input, expected) in [
        ("\tX", "    X"),
        ("a\tX", "a   X"),
        ("ab\tX", "ab  X"),
        ("abc\tX", "abc X"),
        ("abcd\tX", "abcd    X"),
        ("a\tX\tY", "a   X   Y"),
        (" \t\tX\t", "        X   "),
        ("界\tX", "界  X"),
        ("e\u{301}\tX", "e\u{301}   X"),
        ("👩‍💻\tX", "👩‍💻  X"),
        ("\u{1b}\tX", "^[  X"),
        ("a    X", "a    X"),
    ] {
        let mut spans = [Span::raw(input)];
        expand_tabs_in_spans(&mut spans, 4);
        assert_eq!(spans[0].content, expected, "{input:?}");
    }
}

#[test]
fn tab_alignment_and_styles_survive_every_span_boundary() {
    let input = "a界e\u{301}👩‍💻\tX\tY";
    let expected = "a界e\u{301}👩‍💻  X   Y";
    let left_style = Style::default().fg(Color::Red);
    let right_style = Style::default().fg(Color::Blue);
    for split in input
        .char_indices()
        .map(|(index, _)| index)
        .chain([input.len()])
    {
        let mut spans = [
            Span::styled(input[..split].to_owned(), left_style),
            Span::styled(input[split..].to_owned(), right_style),
        ];
        expand_tabs_in_spans(&mut spans, 4);
        assert_eq!(
            spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>(),
            expected
        );
        assert_eq!(spans[0].style, left_style);
        assert_eq!(spans[1].style, right_style);
    }
}

#[test]
fn preview_lines_reset_tab_columns_before_width_measurement_and_wrapping() {
    let preview = crate::preview::PreviewContent::new(
        crate::preview::PreviewKind::Text,
        vec![Line::from("abc\tX"), Line::from("a\tX")],
    );
    for line in preview.lines.iter() {
        assert_eq!(line.width(), 5);
    }
    let wrapped = preview.wrapped_lines(5);
    assert_eq!(wrapped.len(), 2);
    assert_eq!(wrapped[0].to_string(), "abc X");
    assert_eq!(wrapped[1].to_string(), "a   X");
}
