use super::common::{consume_operator, next_non_whitespace_char, scan_string, styled_text};
use crate::appearance;
use ratatui::{style::Modifier, text::Span};

pub(super) fn highlight_js_like_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let (body, comment) = split_line_comment(line);
    let bytes = body.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;
    let mut last_word: Option<String> = None;

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

        if matches!(ch, '"' | '\'' | '`') {
            let end = scan_string(body, index, ch);
            spans.push(styled_text(
                &body[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            last_word = None;
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                let current = bytes[index] as char;
                if current.is_ascii_alphanumeric() || current == '_' || current == '$' {
                    index += 1;
                } else {
                    break;
                }
            }
            let token = &body[start..index];
            let next = next_non_whitespace_char(body, index);
            let color = if is_js_keyword(token) {
                palette.keyword
            } else if matches!(
                last_word.as_deref(),
                Some("function")
                    | Some("class")
                    | Some("interface")
                    | Some("type")
                    | Some("enum")
                    | Some("namespace")
            ) {
                if matches!(
                    last_word.as_deref(),
                    Some("class" | "interface" | "type" | "enum" | "namespace")
                ) {
                    palette.r#type
                } else {
                    palette.function
                }
            } else if next == Some('(') {
                palette.function
            } else if token
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_uppercase())
            {
                palette.r#type
            } else {
                palette.fg
            };
            spans.push(styled_text(token, color, Modifier::empty()));
            last_word = Some(token.to_string());
            continue;
        }

        if ch.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len() {
                let current = bytes[index] as char;
                if current.is_ascii_alphanumeric() || matches!(current, '.' | '_') {
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
            last_word = None;
            continue;
        }

        let end = consume_operator(body, index);
        let token = &body[index..end];
        let color = if token == "=>"
            || token == "::"
            || token == "?."
            || token == "??"
            || token == "&&"
            || token == "||"
        {
            palette.operator
        } else if token == "<" || token == ">" || token == "</" || token == "/>" {
            palette.tag
        } else {
            palette.operator
        };
        spans.push(styled_text(token, color, Modifier::empty()));
        index = end;
        last_word = None;
    }

    if let Some(comment) = comment {
        if !body.is_empty() {
            spans.push(Span::raw(" ".to_string()));
        }
        spans.push(styled_text(comment, palette.comment, Modifier::ITALIC));
    }

    spans
}

fn split_line_comment(input: &str) -> (&str, Option<&str>) {
    let bytes = input.as_bytes();
    let mut index = 0usize;
    let mut quote = '\0';
    let mut escape = false;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if quote != '\0' {
            if escape {
                escape = false;
                index += 1;
                continue;
            }
            if ch == '\\' {
                escape = true;
                index += 1;
                continue;
            }
            if ch == quote {
                quote = '\0';
            }
            index += 1;
            continue;
        }

        if matches!(ch, '"' | '\'' | '`') {
            quote = ch;
            index += 1;
            continue;
        }

        if ch == '/'
            && bytes
                .get(index + 1)
                .is_some_and(|next| *next as char == '/')
        {
            return (&input[..index], Some(&input[index..]));
        }

        index += 1;
    }

    (input, None)
}

fn is_js_keyword(token: &str) -> bool {
    matches!(
        token,
        "const"
            | "let"
            | "var"
            | "function"
            | "return"
            | "export"
            | "import"
            | "from"
            | "default"
            | "if"
            | "else"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "for"
            | "while"
            | "do"
            | "try"
            | "catch"
            | "finally"
            | "throw"
            | "new"
            | "class"
            | "extends"
            | "async"
            | "await"
            | "typeof"
            | "instanceof"
            | "in"
            | "of"
            | "this"
            | "super"
            | "interface"
            | "type"
            | "enum"
            | "implements"
            | "namespace"
            | "public"
            | "private"
            | "protected"
            | "readonly"
            | "as"
            | "declare"
            | "satisfies"
            | "infer"
            | "keyof"
    )
}
