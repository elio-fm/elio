use super::{appearance as theme, *};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

pub(super) struct TextPreview {
    pub text: String,
    pub bytes_truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Utf16Endian {
    Little,
    Big,
}

pub(super) fn render_reflowed_text_preview(text: &str) -> Vec<Line<'static>> {
    let palette = theme::palette();
    let mut rendered = Vec::new();
    let mut pending: Vec<&str> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !pending.is_empty() {
                let joined = pending.join(" ");
                pending.clear();
                rendered.push(Line::from(Span::styled(
                    super::expand_tabs(&joined),
                    Style::default().fg(palette.text),
                )));
            }
            rendered.push(Line::default());
        } else {
            pending.push(trimmed);
        }
    }

    if !pending.is_empty() {
        let joined = pending.join(" ");
        rendered.push(Line::from(Span::styled(
            super::expand_tabs(&joined),
            Style::default().fg(palette.text),
        )));
    }

    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
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
    line_truncated: bool,
    truncation_note: Option<String>,
) -> PreviewContent {
    let shown_lines = if line_truncated {
        PREVIEW_RENDER_LINE_LIMIT
    } else {
        source_line_count
    };
    preview = preview.with_line_coverage(
        shown_lines,
        (!bytes_truncated).then_some(source_line_count),
        bytes_truncated || line_truncated,
    );
    if !bytes_truncated {
        preview = preview.with_source_lines(source_line_count);
    }
    if let Some(note) = truncation_note {
        preview = preview.with_truncation(note);
    }
    preview
}

pub(super) fn finalize_text_preview_with_line_limit(
    mut preview: PreviewContent,
    source_line_count: usize,
    bytes_truncated: bool,
    line_truncated: bool,
    truncation_note: Option<String>,
    shown_line_limit: usize,
) -> PreviewContent {
    let shown_lines = if line_truncated {
        shown_line_limit
    } else {
        source_line_count
    };
    preview = preview.with_line_coverage(
        shown_lines,
        (!bytes_truncated).then_some(source_line_count),
        bytes_truncated || line_truncated,
    );
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

pub(super) fn truncation_note_with_line_limit(
    bytes_truncated: bool,
    line_truncated: bool,
    shown_line_limit: usize,
) -> Option<String> {
    let mut parts = Vec::new();
    if bytes_truncated {
        parts.push("truncated to 64 KiB".to_string());
    }
    if line_truncated {
        parts.push(format!("showing first {shown_line_limit} lines"));
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

pub(crate) fn count_total_text_lines(path: &Path) -> anyhow::Result<usize> {
    let mut file = File::open(path)?;
    let mut prefix = [0u8; 2];
    let prefix_len = file.read(&mut prefix)?;
    if prefix_len == 0 {
        return Ok(1);
    }

    match &prefix[..prefix_len] {
        [0xFF, 0xFE] => count_utf16_lines(BufReader::new(file), Utf16Endian::Little),
        [0xFE, 0xFF] => count_utf16_lines(BufReader::new(file), Utf16Endian::Big),
        _ => count_utf8_lines(BufReader::new(file), &prefix[..prefix_len]),
    }
}

pub(super) fn collect_preview_lines(text: &str) -> Vec<String> {
    collect_preview_lines_with_limit(text, PREVIEW_RENDER_LINE_LIMIT)
}

pub(super) fn collect_preview_lines_with_limit(text: &str, line_limit: usize) -> Vec<String> {
    text.lines()
        .take(line_limit)
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

fn count_utf8_lines(mut reader: BufReader<File>, prefix: &[u8]) -> anyhow::Result<usize> {
    let mut newline_count = prefix.iter().filter(|&&byte| byte == b'\n').count();
    let mut saw_bytes = !prefix.is_empty();
    let mut last_byte = prefix.last().copied();
    let mut buffer = [0u8; 8 * 1024];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let chunk = &buffer[..read];
        newline_count += chunk.iter().filter(|&&byte| byte == b'\n').count();
        saw_bytes = true;
        last_byte = chunk.last().copied();
    }

    Ok(finalize_counted_lines(
        saw_bytes,
        newline_count,
        last_byte == Some(b'\n'),
    ))
}

fn count_utf16_lines(mut reader: BufReader<File>, endian: Utf16Endian) -> anyhow::Result<usize> {
    let mut newline_count = 0usize;
    let mut saw_units = false;
    let mut last_unit_was_newline = false;
    let mut buffer = [0u8; 8 * 1024];
    let mut pending = Vec::new();

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        pending.extend_from_slice(&buffer[..read]);
        let complete_len = pending.len() - (pending.len() % 2);
        for chunk in pending[..complete_len].chunks_exact(2) {
            let unit = match endian {
                Utf16Endian::Little => u16::from_le_bytes([chunk[0], chunk[1]]),
                Utf16Endian::Big => u16::from_be_bytes([chunk[0], chunk[1]]),
            };
            if unit == 0x000A {
                newline_count += 1;
                last_unit_was_newline = true;
            } else {
                last_unit_was_newline = false;
            }
            saw_units = true;
        }
        pending.drain(..complete_len);
    }

    Ok(finalize_counted_lines(
        saw_units,
        newline_count,
        last_unit_was_newline,
    ))
}

fn finalize_counted_lines(
    saw_content: bool,
    newline_count: usize,
    ends_with_newline: bool,
) -> usize {
    if !saw_content {
        return 1;
    }
    if ends_with_newline {
        newline_count.max(1)
    } else {
        newline_count.saturating_add(1).max(1)
    }
}
