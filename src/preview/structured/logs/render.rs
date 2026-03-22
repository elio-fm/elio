use super::super::{LINE_LIMIT, StructuredPreview, styled};
use super::types::{ParsedLogDocument, ParsedLogEntry};
use crate::{file_info::StructuredFormat, ui::theme};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};
use std::collections::BTreeMap;

pub(super) fn render_parsed_log(document: ParsedLogDocument) -> StructuredPreview {
    let palette = theme::code_preview_palette();
    let mut counts = BTreeMap::new();
    for entry in &document.entries {
        if let Some(level) = &entry.level {
            *counts.entry(level.clone()).or_insert(0usize) += 1;
        }
    }

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        styled("format", palette.parameter, Modifier::BOLD),
        styled(": ", palette.operator, Modifier::empty()),
        Span::raw(document.source.label().to_string()),
        Span::raw("  ".to_string()),
        styled("entries", palette.parameter, Modifier::BOLD),
        styled(": ", palette.operator, Modifier::empty()),
        Span::raw(document.entries.len().to_string()),
    ]));

    if let Some((first, last)) = time_range(&document.entries) {
        let mut spans = vec![
            styled("range", palette.parameter, Modifier::BOLD),
            styled(": ", palette.operator, Modifier::empty()),
            styled(&first, palette.comment, Modifier::empty()),
        ];
        if first != last {
            spans.push(Span::raw("  ->  ".to_string()));
            spans.push(styled(&last, palette.comment, Modifier::empty()));
        }
        lines.push(Line::from(spans));
    }

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
    }

    if !lines.is_empty() {
        lines.push(Line::from(""));
    }

    let mut truncated = false;
    for entry in document.entries {
        if lines.len() >= LINE_LIMIT {
            truncated = true;
            break;
        }
        lines.push(render_entry_summary(&entry, palette));

        for (key, value) in entry.fields {
            if lines.len() >= LINE_LIMIT {
                truncated = true;
                break;
            }
            lines.push(Line::from(vec![
                Span::raw("  ".to_string()),
                styled(&key, palette.parameter, Modifier::BOLD),
                styled(": ", palette.operator, Modifier::empty()),
                Span::raw(truncate_display(&value, 96)),
            ]));
        }
        if truncated {
            break;
        }

        for continuation in entry.continuations {
            if lines.len() >= LINE_LIMIT {
                truncated = true;
                break;
            }
            lines.push(Line::from(vec![
                Span::raw("  ".to_string()),
                styled("│", palette.comment, Modifier::empty()),
                Span::raw(" ".to_string()),
                styled(
                    &truncate_display(&continuation, 116),
                    palette.comment,
                    Modifier::empty(),
                ),
            ]));
        }
    }

    StructuredPreview {
        lines,
        detail: StructuredFormat::Log.detail_label(),
        truncation_note: truncated.then(|| format!("showing first {LINE_LIMIT} lines")),
    }
}

fn render_entry_summary(
    entry: &ParsedLogEntry,
    palette: theme::CodePreviewPalette,
) -> Line<'static> {
    let mut spans = Vec::new();
    if let Some(timestamp) = &entry.timestamp {
        spans.push(styled(timestamp, palette.comment, Modifier::empty()));
        spans.push(Span::raw("  ".to_string()));
    }
    if let Some(level) = &entry.level {
        spans.push(styled(
            level,
            log_level_color(level, palette),
            Modifier::BOLD,
        ));
        spans.push(Span::raw("  ".to_string()));
    }
    spans.push(Span::raw(truncate_display(&entry.message, 116)));
    Line::from(spans)
}

fn time_range(entries: &[ParsedLogEntry]) -> Option<(String, String)> {
    let first = entries.iter().find_map(|entry| entry.timestamp.clone())?;
    let last = entries
        .iter()
        .rev()
        .find_map(|entry| entry.timestamp.clone())
        .unwrap_or_else(|| first.clone());
    Some((first, last))
}

pub(super) fn truncate_display(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    let kept = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    format!("{kept}…")
}

fn log_level_color(level: &str, palette: theme::CodePreviewPalette) -> ratatui::style::Color {
    match level {
        "TRACE" | "DEBUG" => palette.comment,
        "INFO" => palette.function,
        "WARN" => palette.constant,
        "ERROR" | "FATAL" => palette.invalid,
        _ => palette.keyword,
    }
}
