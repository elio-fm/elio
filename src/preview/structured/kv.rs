use super::{LINE_LIMIT, StructuredPreview, styled};
use crate::{file_info::StructuredFormat, preview::appearance as theme};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};

pub(super) fn render_dotenv_preview(text: &str) -> StructuredPreview {
    let palette = theme::code_preview_palette();
    let entries = parse_dotenv_entries(text);
    let key_width = entries
        .iter()
        .filter_map(|entry| match entry {
            DotenvEntry::Binding { key, .. } => Some(key.len()),
            _ => None,
        })
        .max()
        .unwrap_or(0)
        .min(32);

    let mut lines = Vec::new();
    let mut truncated = false;
    for entry in entries {
        if lines.len() >= LINE_LIMIT {
            truncated = true;
            break;
        }

        let line = match entry {
            DotenvEntry::Blank => Line::from(""),
            DotenvEntry::Comment(comment) => {
                Line::from(vec![styled(&comment, palette.comment, Modifier::ITALIC)])
            }
            DotenvEntry::Binding { key, value } => Line::from(vec![
                styled(
                    &format!("{key:width$}", width = key_width),
                    palette.function,
                    Modifier::BOLD,
                ),
                styled(" = ", palette.operator, Modifier::empty()),
                styled(&value, palette.string, Modifier::empty()),
            ]),
            DotenvEntry::Raw(raw) => Line::from(vec![Span::raw(raw)]),
        };
        lines.push(line);
    }

    StructuredPreview {
        lines: if lines.is_empty() {
            vec![Line::from("File is empty")]
        } else {
            lines
        },
        detail: StructuredFormat::Dotenv.detail_label(),
        truncation_note: truncated.then(|| format!("showing first {LINE_LIMIT} lines")),
    }
}

#[derive(Clone, Debug)]
enum DotenvEntry {
    Blank,
    Comment(String),
    Binding { key: String, value: String },
    Raw(String),
}

fn parse_dotenv_entries(text: &str) -> Vec<DotenvEntry> {
    text.lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return DotenvEntry::Blank;
            }
            if trimmed.starts_with('#') {
                return DotenvEntry::Comment(trimmed.to_string());
            }

            let body = trimmed.strip_prefix("export ").unwrap_or(trimmed);
            if let Some((key, value)) = body.split_once('=') {
                return DotenvEntry::Binding {
                    key: key.trim().to_string(),
                    value: value.trim().to_string(),
                };
            }

            DotenvEntry::Raw(line.to_string())
        })
        .collect()
}
