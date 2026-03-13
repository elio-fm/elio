use super::common::{consume_operator, scan_string, styled_text};
use crate::appearance;
use ratatui::{style::Modifier, text::Span};

pub(super) fn highlight_c_like_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    in_block_comment: &mut bool,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    for (is_comment, segment) in split_c_like_segments(line, in_block_comment) {
        if is_comment {
            spans.push(styled_text(segment, palette.comment, Modifier::ITALIC));
        } else {
            spans.extend(highlight_c_like_segment(segment, palette));
        }
    }

    spans
}

pub(super) fn highlight_brace_like_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
    in_block_comment: &mut bool,
) -> Vec<Span<'static>> {
    highlight_c_like_line(line, palette, in_block_comment)
}

fn highlight_c_like_segment(
    input: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let trimmed = input.trim_start();
    let indent = &input[..input.len().saturating_sub(trimmed.len())];
    let mut spans = vec![Span::raw(indent.to_string())];

    if let Some(stripped) = trimmed.strip_prefix('#') {
        spans.push(styled_text("#", palette.r#macro, Modifier::BOLD));
        let directive = stripped.trim_start();
        let directive_indent = &stripped[..stripped.len().saturating_sub(directive.len())];
        if !directive_indent.is_empty() {
            spans.push(Span::raw(directive_indent.to_string()));
        }
        let mut index = 0usize;
        while let Some(ch) = directive[index..].chars().next() {
            if ch.is_ascii_alphabetic() {
                index += ch.len_utf8();
            } else {
                break;
            }
        }
        if index > 0 {
            spans.push(styled_text(
                &directive[..index],
                palette.r#macro,
                Modifier::BOLD,
            ));
            spans.extend(highlight_c_like_tokens(
                &directive[index..],
                palette,
                Some(&directive[..index]),
            ));
        } else {
            spans.extend(highlight_c_like_tokens(directive, palette, None));
        }
        return spans;
    }

    spans.extend(highlight_c_like_tokens(trimmed, palette, None));
    spans
}

fn highlight_c_like_tokens(
    input: &str,
    palette: appearance::CodePreviewPalette,
    directive: Option<&str>,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;
    let mut last_word: Option<String> = None;

    while index < input.len() {
        let ch = input[index..].chars().next().unwrap_or(' ');
        if ch.is_whitespace() {
            let start = index;
            while let Some(current) = input[index..].chars().next() {
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            spans.push(Span::raw(input[start..index].to_string()));
            continue;
        }

        if matches!(ch, '"' | '\'') {
            let end = scan_string(input, index, ch);
            spans.push(styled_text(
                &input[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            last_word = None;
            continue;
        }

        if directive == Some("include")
            && ch == '<'
            && let Some(close) = input[index + 1..].find('>')
        {
            let end = index + close + 2;
            spans.push(styled_text(
                &input[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            last_word = None;
            continue;
        }

        if ch == '$' {
            let start = index;
            index += ch.len_utf8();
            while let Some(current) = input[index..].chars().next() {
                if current.is_ascii_alphanumeric() || current == '_' {
                    index += current.len_utf8();
                } else {
                    break;
                }
            }
            spans.push(styled_text(
                &input[start..index],
                palette.parameter,
                Modifier::empty(),
            ));
            last_word = None;
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            index += ch.len_utf8();
            while let Some(current) = input[index..].chars().next() {
                if current.is_ascii_alphanumeric() || current == '_' {
                    index += current.len_utf8();
                } else {
                    break;
                }
            }
            let token = &input[start..index];
            let next = input[index..]
                .chars()
                .find(|current| !current.is_whitespace());
            let color = if is_brace_like_type_keyword(token) {
                palette.r#type
            } else if is_brace_like_keyword(token) {
                palette.keyword
            } else if matches!(
                last_word.as_deref(),
                Some(
                    "struct"
                        | "enum"
                        | "union"
                        | "class"
                        | "interface"
                        | "trait"
                        | "impl"
                        | "type"
                        | "namespace"
                        | "package"
                        | "module"
                        | "protocol"
                        | "actor"
                        | "object"
                )
            ) {
                palette.r#type
            } else if token
                .chars()
                .all(|current| current.is_ascii_uppercase() || current == '_')
            {
                palette.r#macro
            } else if next == Some('(') && !is_brace_like_control_like(token) {
                palette.function
            } else {
                palette.fg
            };
            let modifier = if color == palette.keyword || color == palette.r#macro {
                Modifier::BOLD
            } else {
                Modifier::empty()
            };
            spans.push(styled_text(token, color, modifier));
            last_word = Some(token.to_string());
            continue;
        }

        if ch.is_ascii_digit() {
            let start = index;
            index += ch.len_utf8();
            while let Some(current) = input[index..].chars().next() {
                if current.is_ascii_alphanumeric() || matches!(current, '.' | '_' | 'x' | 'X') {
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
            last_word = None;
            continue;
        }

        let end = consume_operator(input, index);
        spans.push(styled_text(
            &input[index..end],
            palette.operator,
            Modifier::empty(),
        ));
        index = end;
        last_word = None;
    }

    spans
}

fn split_c_like_segments<'a>(line: &'a str, in_block_comment: &mut bool) -> Vec<(bool, &'a str)> {
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    while cursor < line.len() {
        if *in_block_comment {
            let comment_start = cursor;
            if let Some(offset) = line[cursor..].find("*/") {
                let end = cursor + offset + 2;
                segments.push((true, &line[comment_start..end]));
                *in_block_comment = false;
                cursor = end;
            } else {
                segments.push((true, &line[comment_start..]));
                return segments;
            }
            continue;
        }

        let code_start = cursor;
        let mut index = cursor;
        let mut quote = '\0';
        let mut escape = false;

        while index < line.len() {
            let ch = line[index..].chars().next().expect("valid utf-8 char");
            let next = index + ch.len_utf8();

            if quote != '\0' {
                if escape {
                    escape = false;
                    index = next;
                    continue;
                }
                if ch == '\\' {
                    escape = true;
                    index = next;
                    continue;
                }
                if ch == quote {
                    quote = '\0';
                }
                index = next;
                continue;
            }

            if matches!(ch, '"' | '\'') {
                quote = ch;
                index = next;
                continue;
            }

            if ch == '/'
                && let Some(next_char) = line[next..].chars().next()
            {
                if next_char == '/' {
                    if code_start < index {
                        segments.push((false, &line[code_start..index]));
                    }
                    segments.push((true, &line[index..]));
                    return segments;
                }

                if next_char == '*' {
                    if code_start < index {
                        segments.push((false, &line[code_start..index]));
                    }

                    let comment_start = index;
                    let search_start = next + next_char.len_utf8();
                    if let Some(offset) = line[search_start..].find("*/") {
                        let end = search_start + offset + 2;
                        segments.push((true, &line[comment_start..end]));
                        cursor = end;
                    } else {
                        segments.push((true, &line[comment_start..]));
                        *in_block_comment = true;
                        return segments;
                    }
                    break;
                }
            }

            index = next;
        }

        if cursor == index {
            segments.push((false, &line[cursor..]));
            return segments;
        }

        if index >= line.len() {
            segments.push((false, &line[code_start..]));
            return segments;
        }
    }

    if segments.is_empty() {
        segments.push((false, line));
    }
    segments
}

fn is_brace_like_keyword(token: &str) -> bool {
    matches!(
        token,
        "if" | "else"
            | "switch"
            | "case"
            | "default"
            | "for"
            | "while"
            | "do"
            | "break"
            | "continue"
            | "return"
            | "goto"
            | "sizeof"
            | "typedef"
            | "static"
            | "extern"
            | "const"
            | "volatile"
            | "inline"
            | "restrict"
            | "struct"
            | "enum"
            | "union"
            | "class"
            | "namespace"
            | "template"
            | "using"
            | "new"
            | "delete"
            | "try"
            | "catch"
            | "throw"
            | "throws"
            | "public"
            | "private"
            | "protected"
            | "final"
            | "override"
            | "abstract"
            | "extends"
            | "implements"
            | "interface"
            | "package"
            | "import"
            | "export"
            | "fn"
            | "let"
            | "mut"
            | "impl"
            | "trait"
            | "pub"
            | "use"
            | "mod"
            | "crate"
            | "super"
            | "self"
            | "Self"
            | "ref"
            | "move"
            | "where"
            | "match"
            | "async"
            | "await"
            | "unsafe"
            | "dyn"
            | "type"
            | "as"
            | "loop"
            | "yield"
            | "defer"
            | "func"
            | "select"
            | "go"
            | "fallthrough"
            | "chan"
            | "range"
            | "var"
            | "val"
            | "when"
            | "is"
            | "in"
            | "object"
            | "data"
            | "sealed"
            | "open"
            | "operator"
            | "infix"
            | "reified"
            | "suspend"
            | "protocol"
            | "guard"
            | "actor"
            | "nonisolated"
            | "associatedtype"
    )
}

fn is_brace_like_type_keyword(token: &str) -> bool {
    matches!(
        token,
        "void"
            | "char"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "signed"
            | "unsigned"
            | "bool"
            | "size_t"
            | "ssize_t"
            | "uint8_t"
            | "uint16_t"
            | "uint32_t"
            | "uint64_t"
            | "int8_t"
            | "int16_t"
            | "int32_t"
            | "int64_t"
            | "auto"
            | "byte"
            | "boolean"
            | "String"
            | "str"
            | "usize"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "f32"
            | "f64"
            | "Result"
            | "Option"
            | "Vec"
    )
}

fn is_brace_like_control_like(token: &str) -> bool {
    matches!(
        token,
        "if" | "for"
            | "while"
            | "switch"
            | "return"
            | "sizeof"
            | "match"
            | "catch"
            | "when"
            | "guard"
    )
}
