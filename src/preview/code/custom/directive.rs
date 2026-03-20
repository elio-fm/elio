use super::{looks_numeric, scan_quoted_segment, styled_text};
use crate::ui::theme;
use ratatui::{style::Modifier, text::Span};

pub(super) fn highlight_directive_conf_line(
    line: &str,
    palette: theme::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len().saturating_sub(trimmed.len())];

    if trimmed.is_empty() {
        return vec![Span::raw(line.to_string())];
    }

    if trimmed.starts_with('#') || trimmed.starts_with(';') {
        return vec![
            Span::raw(indent.to_string()),
            styled_text(trimmed, palette.comment, Modifier::ITALIC),
        ];
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return vec![
            Span::raw(indent.to_string()),
            styled_text(trimmed, palette.r#type, Modifier::BOLD),
        ];
    }

    let key_end = scan_directive_key_end(trimmed);
    if key_end == 0 {
        return highlight_directive_value_fragment(line, palette);
    }

    let mut spans = vec![
        Span::raw(indent.to_string()),
        styled_text(&trimmed[..key_end], palette.function, Modifier::BOLD),
    ];
    let mut index = key_end;

    while let Some(ch) = trimmed[index..].chars().next() {
        if ch.is_whitespace() {
            let start = index;
            index += ch.len_utf8();
            while let Some(current) = trimmed[index..].chars().next() {
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            spans.push(Span::raw(trimmed[start..index].to_string()));
            continue;
        }
        break;
    }

    if trimmed[index..].starts_with('=') {
        spans.push(styled_text("=", palette.operator, Modifier::empty()));
        index += 1;
        while let Some(ch) = trimmed[index..].chars().next() {
            if !ch.is_whitespace() {
                break;
            }
            let start = index;
            index += ch.len_utf8();
            while let Some(current) = trimmed[index..].chars().next() {
                if !current.is_whitespace() {
                    break;
                }
                index += current.len_utf8();
            }
            spans.push(Span::raw(trimmed[start..index].to_string()));
        }
    }

    if index < trimmed.len() {
        spans.extend(highlight_directive_value_fragment(
            &trimmed[index..],
            palette,
        ));
    }

    spans
}

fn scan_directive_key_end(input: &str) -> usize {
    let mut index = 0usize;
    while let Some(ch) = input[index..].chars().next() {
        if ch.is_whitespace() || matches!(ch, '=' | '#' | ';' | '"' | '\'') {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn highlight_directive_value_fragment(
    input: &str,
    palette: theme::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0usize;

    while index < input.len() {
        let ch = input[index..].chars().next().unwrap_or(' ');

        if ch.is_whitespace() {
            let start = index;
            index += ch.len_utf8();
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
            let end = scan_quoted_segment(input, index);
            spans.push(styled_text(
                &input[index..end],
                palette.string,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        if let Some(end) = scan_hex_color(input, index) {
            spans.push(styled_text(
                &input[index..end],
                palette.constant,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        if is_comment_start(input, index) {
            spans.push(styled_text(
                &input[index..],
                palette.comment,
                Modifier::ITALIC,
            ));
            break;
        }

        if matches!(ch, '[' | ']' | '{' | '}' | '(' | ')' | ',' | ':' | '=') {
            let end = index + ch.len_utf8();
            spans.push(styled_text(
                &input[index..end],
                palette.operator,
                Modifier::empty(),
            ));
            index = end;
            continue;
        }

        let start = index;
        index += ch.len_utf8();
        while let Some(current) = input[index..].chars().next() {
            if current.is_whitespace()
                || matches!(current, '[' | ']' | '{' | '}' | '(' | ')' | ',' | ':' | '=')
                || matches!(current, '"' | '\'')
                || current == '#'
            {
                break;
            }
            index += current.len_utf8();
        }
        spans.push(highlight_directive_token(&input[start..index], palette));
    }

    spans
}

fn highlight_directive_token(token: &str, palette: theme::CodePreviewPalette) -> Span<'static> {
    let color = if is_directive_keyword(token) {
        palette.keyword
    } else if looks_numeric(token) {
        palette.constant
    } else if looks_path_like(token) {
        palette.string
    } else {
        palette.fg
    };

    styled_text(token, color, Modifier::empty())
}

fn is_directive_keyword(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "auto"
            | "disabled"
            | "enabled"
            | "false"
            | "inherit"
            | "no"
            | "none"
            | "null"
            | "off"
            | "on"
            | "true"
            | "yes"
    )
}

fn looks_path_like(token: &str) -> bool {
    token.starts_with("~/")
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with('/')
        || token.starts_with("file:")
}

fn scan_hex_color(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.get(start).copied()? != b'#' {
        return None;
    }

    let mut index = start + 1;
    while matches!(
        bytes.get(index).copied(),
        Some(b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F')
    ) {
        index += 1;
    }

    let digits = index.saturating_sub(start + 1);
    if !matches!(digits, 3 | 4 | 6 | 8) {
        return None;
    }

    if input[index..]
        .chars()
        .next()
        .is_some_and(|ch| !ch.is_whitespace() && !matches!(ch, ',' | ';' | ')' | ']' | '}'))
    {
        return None;
    }

    Some(index)
}

fn is_comment_start(input: &str, index: usize) -> bool {
    input[index..].starts_with('#')
        || input[index..].starts_with(';')
        || (input[index..].starts_with("//")
            && !input[..index].chars().last().is_some_and(|ch| ch == ':'))
}
