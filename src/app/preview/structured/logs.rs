use super::{LINE_LIMIT, StructuredPreview, styled};
use crate::{appearance, file_facts::StructuredFormat};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};
use std::collections::BTreeMap;

pub(super) fn render_log_preview(text: &str) -> StructuredPreview {
    let palette = appearance::code_preview_palette();
    let entries = text.lines().map(parse_log_line).collect::<Vec<_>>();
    let mut counts = BTreeMap::new();
    for entry in &entries {
        if let Some(level) = &entry.level {
            *counts.entry(level.clone()).or_insert(0usize) += 1;
        }
    }

    let mut lines = Vec::new();
    if !counts.is_empty() {
        let mut spans = vec![
            styled("levels", palette.parameter, Modifier::BOLD),
            styled(": ", palette.operator, Modifier::empty()),
        ];
        for (index, (level, count)) in counts.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  ".to_string()));
            }
            spans.push(styled(
                level,
                log_level_color(level, palette),
                Modifier::BOLD,
            ));
            spans.push(Span::raw(format!(" {count}")));
        }
        lines.push(Line::from(spans));
        lines.push(Line::from(""));
    }

    let mut truncated = false;
    for entry in entries {
        if lines.len() >= LINE_LIMIT {
            truncated = true;
            break;
        }

        let mut summary = Vec::new();
        if let Some(timestamp) = entry.timestamp {
            summary.push(styled(&timestamp, palette.comment, Modifier::empty()));
            summary.push(Span::raw("  ".to_string()));
        }
        if let Some(level) = entry.level {
            summary.push(styled(
                &level,
                log_level_color(&level, palette),
                Modifier::BOLD,
            ));
            summary.push(Span::raw("  ".to_string()));
        }
        summary.push(Span::raw(entry.message));
        lines.push(Line::from(summary));

        for (key, value) in entry.fields {
            if lines.len() >= LINE_LIMIT {
                truncated = true;
                break;
            }
            lines.push(Line::from(vec![
                Span::raw("  ".to_string()),
                styled(&key, palette.parameter, Modifier::BOLD),
                styled(": ", palette.operator, Modifier::empty()),
                Span::raw(value),
            ]));
        }
    }

    StructuredPreview {
        lines: if lines.is_empty() {
            vec![Line::from("File is empty")]
        } else {
            lines
        },
        detail: StructuredFormat::Log.detail_label(),
        truncation_note: truncated.then(|| format!("showing first {LINE_LIMIT} lines")),
    }
}

#[derive(Clone, Debug, Default)]
struct ParsedLogLine {
    timestamp: Option<String>,
    level: Option<String>,
    message: String,
    fields: Vec<(String, String)>,
}

fn parse_log_line(line: &str) -> ParsedLogLine {
    let mut entry = ParsedLogLine::default();
    let mut tokens = line.split_whitespace().peekable();

    if let Some(token) = tokens.peek().copied()
        && looks_like_timestamp(token)
    {
        entry.timestamp = Some(token.to_string());
        tokens.next();
    }

    if let Some(token) = tokens.peek().copied()
        && is_log_level(token)
    {
        entry.level = Some(token.to_string());
        tokens.next();
    }

    let mut message_parts = Vec::new();
    for token in tokens {
        if let Some((key, value)) = token.split_once('=') {
            entry.fields.push((key.to_string(), value.to_string()));
        } else {
            message_parts.push(token.to_string());
        }
    }

    entry.message = if message_parts.is_empty() {
        line.to_string()
    } else {
        message_parts.join(" ")
    };
    entry
}

fn is_log_level(token: &str) -> bool {
    matches!(
        token,
        "TRACE" | "DEBUG" | "INFO" | "WARN" | "WARNING" | "ERROR" | "FATAL"
    )
}

fn looks_like_timestamp(token: &str) -> bool {
    token.len() >= 8
        && token.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        && token.contains(':')
        && (token.contains('-') || token.contains('/'))
}

fn log_level_color(level: &str, palette: appearance::CodePreviewPalette) -> ratatui::style::Color {
    match level {
        "TRACE" | "DEBUG" => palette.comment,
        "INFO" => palette.function,
        "WARN" | "WARNING" => palette.constant,
        "ERROR" | "FATAL" => palette.invalid,
        _ => palette.keyword,
    }
}
