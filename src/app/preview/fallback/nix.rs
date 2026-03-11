use super::common::{scan_string, styled_text};
use crate::appearance;
use ratatui::{style::Modifier, text::Span};

pub(super) fn highlight_nix_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let (body, comment) = split_nix_comment(line);
    let bytes = body.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if ch.is_whitespace() {
            let start = index;
            while index < bytes.len() && (bytes[index] as char).is_whitespace() {
                index += 1;
            }
            spans.push(Span::raw(body[start..index].to_string()));
            continue;
        }

        if body[index..].starts_with("''") {
            let end = scan_nix_multiline_string(body, index);
            spans.push(styled_text(
                &body[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        if ch == '"' {
            let end = scan_string(body, index, ch);
            spans.push(styled_text(
                &body[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        if ch.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len() {
                let current = bytes[index] as char;
                if current.is_ascii_alphanumeric() || matches!(current, '.' | '_' | '+' | '-') {
                    index += 1;
                } else {
                    break;
                }
            }
            spans.push(styled_text(
                &body[start..index],
                palette.constant,
                Modifier::empty(),
            ));
            continue;
        }

        if is_nix_word_start(ch) {
            let start = index;
            index += 1;
            while index < bytes.len() {
                let current = bytes[index] as char;
                if is_nix_word_continue(current) {
                    index += 1;
                } else {
                    break;
                }
            }
            let token = &body[start..index];
            let next = next_non_whitespace(body, index);
            let color = if is_nix_keyword(token) {
                palette.keyword
            } else if matches!(token, "true" | "false" | "null") {
                palette.constant
            } else if next == Some('=') || next == Some(':') {
                palette.parameter
            } else {
                palette.fg
            };
            let modifier = if color == palette.keyword {
                Modifier::BOLD
            } else {
                Modifier::empty()
            };
            spans.push(styled_text(token, color, modifier));
            continue;
        }

        let end = consume_nix_operator(body, index);
        spans.push(styled_text(
            &body[index..end],
            palette.operator,
            Modifier::empty(),
        ));
        index = end;
    }

    if let Some(comment) = comment {
        if !body.is_empty() {
            spans.push(Span::raw(" ".to_string()));
        }
        spans.push(styled_text(comment, palette.comment, Modifier::ITALIC));
    }

    spans
}

fn split_nix_comment(input: &str) -> (&str, Option<&str>) {
    let mut quote = '\0';
    let mut escape = false;
    let mut index = 0usize;

    while index < input.len() {
        let ch = input[index..].chars().next().expect("valid utf-8 char");
        if input[index..].starts_with("''") {
            index = scan_nix_multiline_string(input, index);
            continue;
        }

        if quote != '\0' {
            if escape {
                escape = false;
                index += ch.len_utf8();
                continue;
            }
            if ch == '\\' {
                escape = true;
                index += ch.len_utf8();
                continue;
            }
            if ch == quote {
                quote = '\0';
            }
            index += ch.len_utf8();
            continue;
        }

        if ch == '"' {
            quote = ch;
            index += ch.len_utf8();
            continue;
        }

        if ch == '#' {
            return (&input[..index], Some(&input[index..]));
        }

        index += ch.len_utf8();
    }

    (input, None)
}

fn scan_nix_multiline_string(input: &str, start: usize) -> usize {
    let search = start + 2;
    input[search..]
        .find("''")
        .map(|offset| search + offset + 2)
        .unwrap_or(input.len())
}

fn next_non_whitespace(input: &str, start: usize) -> Option<char> {
    input[start..]
        .chars()
        .find(|current| !current.is_whitespace())
}

fn consume_nix_operator(input: &str, start: usize) -> usize {
    const TWO_CHAR: [&str; 7] = ["//", "++", "->", "==", "!=", "&&", "||"];
    for token in TWO_CHAR {
        if input[start..].starts_with(token) {
            return start + token.len();
        }
    }
    start
        + input[start..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1)
}

fn is_nix_word_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '-'
}

fn is_nix_word_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '\'' | '.')
}

fn is_nix_keyword(token: &str) -> bool {
    matches!(
        token,
        "let"
            | "in"
            | "if"
            | "then"
            | "else"
            | "with"
            | "rec"
            | "inherit"
            | "assert"
            | "or"
            | "import"
    )
}
