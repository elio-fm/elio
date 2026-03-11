use super::common::{consume_operator, next_non_whitespace_char, scan_string, styled_text};
use crate::appearance;
use ratatui::{style::Modifier, text::Span};

#[derive(Default)]
pub(super) struct PythonState {
    multiline_string: Option<&'static str>,
}

pub(super) fn highlight_python_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    state: &mut PythonState,
) -> Vec<Span<'static>> {
    if let Some(spans) = highlight_multiline_string_line(line, palette, state) {
        return spans;
    }

    if let Some(rest) = line.strip_prefix("#!") {
        return vec![
            styled_text("#!", palette.r#macro, Modifier::BOLD),
            styled_text(rest, palette.string, Modifier::empty()),
        ];
    }

    let bytes = line.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;
    let mut expect_name: Option<NameKind> = None;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if ch.is_whitespace() {
            let start = index;
            while index < bytes.len() && (bytes[index] as char).is_whitespace() {
                index += 1;
            }
            spans.push(Span::raw(line[start..index].to_string()));
            continue;
        }

        if ch == '#' {
            spans.push(styled_text(
                &line[index..],
                palette.comment,
                Modifier::ITALIC,
            ));
            break;
        }

        if ch == '@' {
            let end = scan_python_decorator(line, index);
            spans.push(styled_text(
                &line[index..end],
                palette.r#macro,
                Modifier::BOLD,
            ));
            index = end;
            expect_name = None;
            continue;
        }

        if let Some(end) = scan_python_string_end(line, index, state) {
            spans.push(styled_text(
                &line[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            expect_name = None;
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                let current = bytes[index] as char;
                if current.is_ascii_alphanumeric() || current == '_' {
                    index += 1;
                } else {
                    break;
                }
            }

            let token = &line[start..index];
            let next = next_non_whitespace_char(line, index);
            let (color, modifier) = if is_python_keyword(token) {
                let color = palette.keyword;
                expect_name = match token {
                    "def" => Some(NameKind::Function),
                    "class" => Some(NameKind::Type),
                    _ => None,
                };
                (color, Modifier::BOLD)
            } else if is_python_constant(token) {
                expect_name = None;
                (palette.constant, Modifier::empty())
            } else if token == "self" || token == "cls" {
                expect_name = None;
                (palette.parameter, Modifier::empty())
            } else if let Some(name_kind) = expect_name.take() {
                match name_kind {
                    NameKind::Function => (palette.function, Modifier::empty()),
                    NameKind::Type => (palette.r#type, Modifier::empty()),
                }
            } else if next == Some('(') && !is_python_control_like(token) {
                expect_name = None;
                (palette.function, Modifier::empty())
            } else if token
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_uppercase())
            {
                expect_name = None;
                (palette.r#type, Modifier::empty())
            } else {
                expect_name = None;
                (palette.fg, Modifier::empty())
            };

            spans.push(styled_text(token, color, modifier));
            continue;
        }

        if ch.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len() {
                let current = bytes[index] as char;
                if current.is_ascii_alphanumeric() || matches!(current, '.' | '_' | 'j') {
                    index += 1;
                } else {
                    break;
                }
            }
            spans.push(styled_text(
                &line[start..index],
                palette.constant,
                Modifier::empty(),
            ));
            expect_name = None;
            continue;
        }

        let end = consume_operator(line, index);
        spans.push(styled_text(
            &line[index..end],
            palette.operator,
            Modifier::empty(),
        ));
        index = end;
        expect_name = None;
    }

    spans
}

fn highlight_multiline_string_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    state: &mut PythonState,
) -> Option<Vec<Span<'static>>> {
    let delimiter = state.multiline_string?;
    if let Some(offset) = line.find(delimiter) {
        let end = offset + delimiter.len();
        state.multiline_string = None;
        let mut spans = vec![styled_text(&line[..end], palette.string, Modifier::empty())];
        if end < line.len() {
            spans.extend(highlight_python_line(&line[end..], palette, state));
        }
        return Some(spans);
    }

    Some(vec![styled_text(line, palette.string, Modifier::empty())])
}

fn scan_python_string_end(input: &str, start: usize, state: &mut PythonState) -> Option<usize> {
    let (prefix_len, quote, delimiter) = python_string_start(input, start)?;
    let content_start = start + prefix_len;
    if delimiter.len() == 3 {
        let search_start = content_start + delimiter.len();
        if let Some(offset) = input[search_start..].find(delimiter) {
            return Some(search_start + offset + delimiter.len());
        }
        state.multiline_string = Some(delimiter);
        return Some(input.len());
    }
    Some(scan_string(input, content_start, quote))
}

fn python_string_start(input: &str, start: usize) -> Option<(usize, char, &'static str)> {
    for prefix_len in [2usize, 1, 0] {
        if start + prefix_len >= input.len() {
            continue;
        }
        let prefix = &input[start..start + prefix_len];
        if !prefix.chars().all(is_python_string_prefix_char) {
            continue;
        }
        let quote = input[start + prefix_len..].chars().next()?;
        if !matches!(quote, '"' | '\'') {
            continue;
        }
        let quote_len = quote.len_utf8();
        let triple_start = start + prefix_len + quote_len;
        if input
            .get(triple_start..)
            .is_some_and(|rest| rest.starts_with(quote))
            && input
                .get(triple_start + quote_len..)
                .is_some_and(|rest| rest.starts_with(quote))
        {
            let delimiter = if quote == '"' { "\"\"\"" } else { "'''" };
            return Some((prefix_len, quote, delimiter));
        }
        return Some((prefix_len, quote, if quote == '"' { "\"" } else { "'" }));
    }
    None
}

fn scan_python_decorator(input: &str, start: usize) -> usize {
    let mut index = start + 1;
    while index < input.len() {
        let ch = input[index..].chars().next().unwrap_or(' ');
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    index
}

fn is_python_string_prefix_char(ch: char) -> bool {
    matches!(ch, 'r' | 'R' | 'b' | 'B' | 'u' | 'U' | 'f' | 'F')
}

fn is_python_keyword(token: &str) -> bool {
    matches!(
        token,
        "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "case"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "match"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "try"
            | "while"
            | "with"
            | "yield"
    )
}

fn is_python_constant(token: &str) -> bool {
    matches!(token, "True" | "False" | "None" | "Ellipsis")
}

fn is_python_control_like(token: &str) -> bool {
    matches!(
        token,
        "if" | "for" | "while" | "with" | "return" | "yield" | "match" | "except"
    )
}

#[derive(Clone, Copy)]
enum NameKind {
    Function,
    Type,
}
