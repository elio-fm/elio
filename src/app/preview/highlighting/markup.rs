use super::common::{
    next_non_whitespace_char, scan_quoted_segment, split_block_comment_segments, styled_text,
};
use crate::appearance;
use ratatui::{style::Modifier, text::Span};

pub(super) fn highlight_markup_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    in_block_comment: &mut bool,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    for (is_comment, segment) in split_markup_segments(line, in_block_comment) {
        if is_comment {
            spans.push(styled_text(segment, palette.comment, Modifier::ITALIC));
        } else {
            spans.extend(highlight_markup_segment(segment, palette));
        }
    }

    spans
}

pub(super) fn highlight_css_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    in_block_comment: &mut bool,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    for (is_comment, segment) in split_block_comment_segments(line, in_block_comment, "/*", "*/") {
        if is_comment {
            spans.push(styled_text(segment, palette.comment, Modifier::ITALIC));
        } else {
            spans.extend(highlight_css_segment(segment, palette));
        }
    }

    spans
}

fn highlight_markup_segment(
    input: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < input.len() {
        let Some(offset) = input[index..].find('<') else {
            spans.push(Span::raw(input[index..].to_string()));
            break;
        };
        let tag_start = index + offset;
        if tag_start > index {
            spans.push(Span::raw(input[index..tag_start].to_string()));
        }

        let tag_end = scan_markup_tag_end(input, tag_start);
        spans.extend(highlight_markup_tag(&input[tag_start..tag_end], palette));
        index = tag_end;
    }

    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }
    spans
}

fn highlight_markup_tag(tag: &str, palette: appearance::CodePreviewPalette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    let opener = if tag.starts_with("</") {
        "</"
    } else if tag.starts_with("<?") {
        "<?"
    } else if tag.starts_with("<!") {
        "<!"
    } else {
        "<"
    };
    spans.push(styled_text(opener, palette.operator, Modifier::empty()));
    index += opener.len();

    let name_start = index;
    while index < tag.len() {
        let ch = tag[index..].chars().next().unwrap_or('>');
        if ch.is_whitespace() || matches!(ch, '/' | '>' | '?' | '=') {
            break;
        }
        index += ch.len_utf8();
    }
    if index > name_start {
        let name = &tag[name_start..index];
        let color = if opener == "<!" {
            palette.keyword
        } else {
            palette.tag
        };
        let modifier = if opener == "<!" {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        spans.push(styled_text(name, color, modifier));
    }

    while index < tag.len() {
        let ch = tag[index..].chars().next().unwrap_or('>');
        if ch.is_whitespace() {
            let start = index;
            while index < tag.len() {
                let current = tag[index..].chars().next().unwrap_or('>');
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            spans.push(Span::raw(tag[start..index].to_string()));
            continue;
        }

        if tag[index..].starts_with("/>") || tag[index..].starts_with("?>") {
            spans.push(styled_text(
                &tag[index..index + 2],
                palette.operator,
                Modifier::empty(),
            ));
            index += 2;
            continue;
        }

        if matches!(ch, '>' | '/' | '?' | '=') {
            let end = index + ch.len_utf8();
            spans.push(styled_text(
                &tag[index..end],
                palette.operator,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        if matches!(ch, '"' | '\'') {
            let end = scan_quoted_segment(tag, index);
            spans.push(styled_text(
                &tag[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        let start = index;
        while index < tag.len() {
            let current = tag[index..].chars().next().unwrap_or('>');
            if current.is_whitespace() || matches!(current, '/' | '>' | '?' | '=' | '"' | '\'') {
                break;
            }
            index += current.len_utf8();
        }
        spans.push(styled_text(
            &tag[start..index],
            palette.parameter,
            Modifier::BOLD,
        ));
    }

    spans
}

fn scan_markup_tag_end(input: &str, start: usize) -> usize {
    let mut index = start + 1;
    let mut quote = '\0';

    while let Some(ch) = input[index..].chars().next() {
        if quote != '\0' {
            if ch == quote {
                quote = '\0';
            }
            index += ch.len_utf8();
            continue;
        }

        if matches!(ch, '"' | '\'') {
            quote = ch;
            index += ch.len_utf8();
            continue;
        }

        index += ch.len_utf8();
        if ch == '>' {
            return index;
        }
    }

    input.len()
}

fn highlight_css_segment(
    input: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < input.len() {
        let ch = input[index..].chars().next().unwrap_or(' ');
        if ch.is_whitespace() {
            let start = index;
            while index < input.len() {
                let current = input[index..].chars().next().unwrap_or(' ');
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            spans.push(Span::raw(input[start..index].to_string()));
            continue;
        }

        if matches!(ch, '"' | '\'') {
            let end = scan_quoted_segment(input, index);
            spans.push(styled_text(
                &input[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        if ch.is_ascii_digit() {
            let start = index;
            index += ch.len_utf8();
            while index < input.len() {
                let current = input[index..].chars().next().unwrap_or(' ');
                if current.is_ascii_alphanumeric() || matches!(current, '.' | '%' | '-' | '_') {
                    index += current.len_utf8();
                } else {
                    break;
                }
            }
            spans.push(styled_text(
                &input[start..index],
                palette.constant,
                Modifier::empty(),
            ));
            continue;
        }

        if "{}[]():;,@".contains(ch) {
            let end = index + ch.len_utf8();
            let color = if ch == '@' {
                palette.keyword
            } else {
                palette.operator
            };
            let modifier = if ch == '@' {
                Modifier::BOLD
            } else {
                Modifier::empty()
            };
            spans.push(styled_text(&input[index..end], color, modifier));
            index = end;
            continue;
        }

        let start = index;
        while index < input.len() {
            let current = input[index..].chars().next().unwrap_or(' ');
            if current.is_whitespace() || "{}[]():;,@\"'".contains(current) {
                break;
            }
            index += current.len_utf8();
        }
        let token = &input[start..index];
        let color = if token.starts_with('@') {
            palette.keyword
        } else if next_non_whitespace_char(input, index) == Some(':') {
            palette.parameter
        } else if next_non_whitespace_char(input, index) == Some('(') {
            palette.function
        } else if token.starts_with('.') || token.starts_with('#') {
            palette.tag
        } else {
            palette.fg
        };
        let modifier = if color == palette.parameter || color == palette.keyword {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        spans.push(styled_text(token, color, modifier));
    }

    spans
}

fn split_markup_segments<'a>(line: &'a str, in_block_comment: &mut bool) -> Vec<(bool, &'a str)> {
    split_block_comment_segments(line, in_block_comment, "<!--", "-->")
}
