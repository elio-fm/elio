use super::*;
use crate::ui::theme;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::{fs::File, io::Read, path::Path};

pub(super) struct TextPreview {
    pub text: String,
    pub bytes_truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Utf16Endian {
    Little,
    Big,
}

pub(super) fn render_plain_text_preview(text: &str) -> Vec<Line<'static>> {
    let palette = theme::palette();
    let mut rendered = Vec::new();

    for line in collect_preview_lines(text) {
        rendered.push(Line::from(Span::styled(
            super::expand_tabs(&line),
            Style::default().fg(palette.text),
        )));
    }

    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
}

pub(super) fn count_source_lines(text: &str) -> usize {
    text.lines().count().max(1)
}

pub(super) fn finalize_text_preview(
    mut preview: PreviewContent,
    source_line_count: usize,
    bytes_truncated: bool,
    truncation_note: Option<String>,
) -> PreviewContent {
    if !bytes_truncated {
        preview = preview.with_source_lines(source_line_count);
    }
    if let Some(note) = truncation_note {
        preview = preview.with_truncation(note);
    }
    preview
}

pub(super) fn truncation_note(bytes_truncated: bool, line_truncated: bool) -> Option<String> {
    let mut parts = Vec::new();
    if bytes_truncated {
        parts.push("truncated to 64 KiB".to_string());
    }
    if line_truncated {
        parts.push(format!("showing first {PREVIEW_RENDER_LINE_LIMIT} lines"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("  •  "))
    }
}

pub(super) fn combine_preview_notes(
    current: Option<String>,
    extra: Option<&str>,
) -> Option<String> {
    match (current, extra) {
        (Some(current), Some(extra)) => Some(format!("{current}  •  {extra}")),
        (Some(current), None) => Some(current),
        (None, Some(extra)) => Some(extra.to_string()),
        (None, None) => None,
    }
}

pub(super) fn read_text_preview(path: &Path) -> anyhow::Result<Option<TextPreview>> {
    let file = File::open(path)?;
    let mut buffer = Vec::with_capacity(PREVIEW_LIMIT_BYTES + 1);
    file.take(PREVIEW_LIMIT_BYTES as u64 + 1)
        .read_to_end(&mut buffer)?;
    let bytes_truncated = buffer.len() > PREVIEW_LIMIT_BYTES;
    if bytes_truncated {
        buffer.truncate(PREVIEW_LIMIT_BYTES);
    }

    if buffer.is_empty() {
        return Ok(Some(TextPreview {
            text: String::new(),
            bytes_truncated,
        }));
    }
    if let Some(text) = decode_utf16_preview(&buffer) {
        return Ok(Some(TextPreview {
            text,
            bytes_truncated,
        }));
    }
    if buffer.contains(&0) {
        return Ok(None);
    }

    match String::from_utf8(buffer) {
        Ok(text) => Ok(Some(TextPreview {
            text,
            bytes_truncated,
        })),
        Err(error) if bytes_truncated && error.utf8_error().error_len().is_none() => {
            let valid_up_to = error.utf8_error().valid_up_to();
            let bytes = error.into_bytes();
            let text = String::from_utf8(bytes[..valid_up_to].to_vec()).ok();
            Ok(text.map(|text| TextPreview {
                text,
                bytes_truncated: true,
            }))
        }
        Err(_) => Ok(None),
    }
}

pub(super) fn collect_preview_lines(text: &str) -> Vec<String> {
    text.lines()
        .take(PREVIEW_RENDER_LINE_LIMIT)
        .map(trim_trailing_line_endings)
        .collect()
}

pub(super) fn trim_trailing_line_endings(line: &str) -> String {
    line.trim_end_matches(['\n', '\r']).to_string()
}

fn decode_utf16_preview(buffer: &[u8]) -> Option<String> {
    let (endian, content) = match buffer {
        [0xFF, 0xFE, rest @ ..] => (Utf16Endian::Little, rest),
        [0xFE, 0xFF, rest @ ..] => (Utf16Endian::Big, rest),
        _ => return None,
    };

    let unit_len = content.len() / 2;
    if unit_len == 0 {
        return Some(String::new());
    }

    let units = content[..unit_len * 2]
        .chunks_exact(2)
        .map(|chunk| match endian {
            Utf16Endian::Little => u16::from_le_bytes([chunk[0], chunk[1]]),
            Utf16Endian::Big => u16::from_be_bytes([chunk[0], chunk[1]]),
        })
        .collect::<Vec<_>>();

    Some(String::from_utf16_lossy(&units))
}
