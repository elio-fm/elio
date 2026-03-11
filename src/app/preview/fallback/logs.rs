use super::common::{looks_numeric, split_unquoted_once, styled_text};
use crate::appearance;
use ratatui::{style::Color, style::Modifier, text::Span};

pub(super) fn highlight_log_line(
    line: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let trimmed = line.trim_start();
    let indent = &line[..line.len().saturating_sub(trimmed.len())];
    spans.push(Span::raw(indent.to_string()));

    let mut rest = trimmed;
    if let Some((timestamp, remaining)) = split_log_timestamp(rest) {
        spans.push(styled_text(timestamp, palette.comment, Modifier::empty()));
        rest = remaining;
        if let Some((whitespace, remaining)) = split_leading_whitespace(rest) {
            spans.push(Span::raw(whitespace.to_string()));
            rest = remaining;
        }
    }

    if let Some((level, remaining)) = split_log_level(rest) {
        spans.push(styled_text(
            level,
            log_level_color(level, palette),
            Modifier::BOLD,
        ));
        rest = remaining;
        if let Some(space_end) = rest
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
            .map(|(index, _)| index)
        {
            spans.push(Span::raw(rest[..space_end].to_string()));
            rest = &rest[space_end..];
        } else {
            spans.push(Span::raw(rest.to_string()));
            return spans;
        }
    }

    spans.extend(highlight_log_message(rest, palette));
    spans
}

fn highlight_log_message(
    line: &str,
    palette: appearance::CodePreviewPalette,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();

    for token in line.split_inclusive(char::is_whitespace) {
        let word = token.trim_end_matches(char::is_whitespace);
        let suffix = &token[word.len()..];

        if word.is_empty() {
            current.push_str(token);
            continue;
        }

        let styled = if let Some((left, right)) = split_unquoted_once(word, '=') {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            spans.push(styled_text(left, palette.parameter, Modifier::BOLD));
            spans.push(styled_text("=", palette.operator, Modifier::empty()));
            spans.extend(super::data::highlight_value_fragment(right, palette));
            if !suffix.is_empty() {
                spans.push(Span::raw(suffix.to_string()));
            }
            continue;
        } else if looks_numeric(word.trim_matches(['[', ']', '(', ')', ',', ';'])) {
            Some(styled_text(word, palette.constant, Modifier::empty()))
        } else if word.starts_with('[') && word.ends_with(']') {
            Some(styled_text(word, palette.r#type, Modifier::empty()))
        } else if word.ends_with(':') && word.len() > 1 {
            Some(styled_text(word, palette.function, Modifier::empty()))
        } else {
            None
        };

        if let Some(span) = styled {
            if !current.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut current)));
            }
            spans.push(span);
            if !suffix.is_empty() {
                spans.push(Span::raw(suffix.to_string()));
            }
        } else {
            current.push_str(token);
        }
    }

    if !current.is_empty() {
        spans.push(Span::raw(current));
    }

    spans
}

fn split_log_timestamp(input: &str) -> Option<(&str, &str)> {
    let mut end = 0usize;
    let mut separators = 0usize;

    for (index, ch) in input.char_indices() {
        if ch.is_ascii_digit() || matches!(ch, '-' | ':' | 'T' | 'Z' | '.' | '+' | '/' | ',') {
            end = index + ch.len_utf8();
            if matches!(ch, '-' | ':' | 'T' | '/') {
                separators += 1;
            }
            continue;
        }
        break;
    }

    if end == 0 || separators < 2 {
        return None;
    }

    Some((&input[..end], &input[end..]))
}

fn split_leading_whitespace(input: &str) -> Option<(&str, &str)> {
    let end = input
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    if end == 0 {
        None
    } else {
        Some((&input[..end], &input[end..]))
    }
}

fn split_log_level(input: &str) -> Option<(&str, &str)> {
    let trimmed = input.trim_start();
    let offset = input.len().saturating_sub(trimmed.len());
    let mut chars = trimmed.char_indices();
    let (start, first) = chars.next()?;

    let (level, consumed) = if first == '[' {
        let end = trimmed.find(']')?;
        (&trimmed[start..=end], end + 1)
    } else {
        let end = trimmed
            .char_indices()
            .find(|(_, ch)| ch.is_whitespace() || matches!(ch, ':' | ',' | ';'))
            .map(|(index, _)| index)
            .unwrap_or(trimmed.len());
        (&trimmed[..end], end)
    };

    let normalized = level
        .trim_matches(|ch| matches!(ch, '[' | ']'))
        .to_ascii_uppercase();
    if !matches!(
        normalized.as_str(),
        "TRACE" | "DEBUG" | "INFO" | "NOTICE" | "WARN" | "WARNING" | "ERROR" | "ERR" | "FATAL"
    ) {
        return None;
    }

    Some((
        &input[offset..offset + consumed],
        &input[offset + consumed..],
    ))
}

fn log_level_color(level: &str, palette: appearance::CodePreviewPalette) -> Color {
    match level
        .trim_matches(|ch| matches!(ch, '[' | ']'))
        .to_ascii_uppercase()
        .as_str()
    {
        "TRACE" => palette.comment,
        "DEBUG" => palette.constant,
        "INFO" | "NOTICE" => palette.function,
        "WARN" | "WARNING" => palette.keyword,
        "ERROR" | "ERR" | "FATAL" => palette.invalid,
        _ => palette.fg,
    }
}
