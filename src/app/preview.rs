mod document;
mod fallback;
mod markdown;
mod structured;
mod syntax;

use super::*;
use crate::{appearance, file_facts};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    process::Command,
};

const PREVIEW_LIMIT_BYTES: usize = 64 * 1024;
const PREVIEW_RENDER_LINE_LIMIT: usize = 240;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PreviewKind {
    Directory,
    Document,
    Markdown,
    Code,
    Text,
    Binary,
    Unavailable,
}

impl PreviewKind {
    pub(super) fn section_label(self) -> &'static str {
        match self {
            Self::Directory => "Contents",
            Self::Document => "Document",
            Self::Markdown => "Markdown",
            Self::Code => "Code",
            Self::Text => "Text",
            Self::Binary | Self::Unavailable => "Preview",
        }
    }

    pub(super) fn wraps_in_preview(self) -> bool {
        matches!(
            self,
            Self::Document | Self::Markdown | Self::Text | Self::Binary | Self::Unavailable
        )
    }

    pub(super) fn allows_horizontal_scroll(self) -> bool {
        self == Self::Code
    }
}

#[derive(Clone, Debug)]
pub(super) struct PreviewContent {
    pub kind: PreviewKind,
    pub detail: Option<String>,
    pub truncated: bool,
    pub truncation_note: Option<String>,
    pub source_lines: Option<usize>,
    pub item_count: Option<usize>,
    pub folder_count: Option<usize>,
    pub file_count: Option<usize>,
    pub lines: Vec<Line<'static>>,
}

struct TextPreview {
    text: String,
    bytes_truncated: bool,
}

#[derive(Default)]
struct TorrentMetadata {
    name: Option<String>,
    announce: Option<String>,
    comment: Option<String>,
    created_by: Option<String>,
    file_count: usize,
    total_size: Option<u64>,
}

impl PreviewContent {
    pub(super) fn new(kind: PreviewKind, lines: Vec<Line<'static>>) -> Self {
        Self {
            kind,
            detail: None,
            truncated: false,
            truncation_note: None,
            source_lines: None,
            item_count: None,
            folder_count: None,
            file_count: None,
            lines,
        }
    }

    pub(super) fn placeholder(label: &str) -> Self {
        Self::new(
            PreviewKind::Unavailable,
            vec![Line::from(label.to_string())],
        )
    }

    pub(super) fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub(super) fn with_source_lines(mut self, source_lines: usize) -> Self {
        self.source_lines = Some(source_lines.max(1));
        self
    }

    pub(super) fn with_truncation(mut self, note: impl Into<String>) -> Self {
        self.truncated = true;
        self.truncation_note = Some(note.into());
        self
    }

    pub(super) fn with_directory_counts(
        mut self,
        item_count: usize,
        folder_count: usize,
        file_count: usize,
    ) -> Self {
        self.item_count = Some(item_count);
        self.folder_count = Some(folder_count);
        self.file_count = Some(file_count);
        self
    }

    pub(super) fn section_label(&self) -> &'static str {
        self.kind.section_label()
    }

    pub(super) fn total_lines(&self) -> usize {
        self.lines.len()
    }

    pub(super) fn lines(&self) -> Vec<Line<'static>> {
        self.lines.clone()
    }

    pub(super) fn visual_line_count(&self, width: usize) -> usize {
        if !self.kind.wraps_in_preview() {
            return self.total_lines();
        }
        let width = width.max(1);
        self.lines
            .iter()
            .map(|line| {
                let line_width = preview_line_width(line);
                line_width.max(1).div_ceil(width)
            })
            .sum::<usize>()
            .max(1)
    }

    pub(super) fn max_line_width(&self) -> usize {
        self.lines.iter().map(preview_line_width).max().unwrap_or(0)
    }

    pub(super) fn header_detail(&self, offset: usize, visible_rows: usize) -> Option<String> {
        if self.kind == PreviewKind::Directory {
            return None;
        }

        let mut parts = Vec::new();
        if let Some(detail) = &self.detail
            && !detail.is_empty()
        {
            parts.push(detail.clone());
        }

        if let Some(source_lines) = self.source_lines {
            parts.push(format!("{source_lines} lines"));
        }

        if let Some(note) = &self.truncation_note {
            parts.push(note.clone());
        }

        if !parts.is_empty() {
            return Some(parts.join("  •  "));
        }

        let rendered_total = self.total_lines();
        if rendered_total == 0 {
            return self.detail.clone();
        }

        let start = offset.saturating_add(1);
        let end = (offset + visible_rows.max(1)).min(rendered_total);
        let range = if rendered_total > visible_rows.max(1) {
            format!("{start}-{end} / {rendered_total}")
        } else {
            format!("{rendered_total} lines")
        };

        match &self.detail {
            Some(detail) if !detail.is_empty() => Some(format!("{detail}  •  {range}")),
            _ => Some(range),
        }
    }
}

fn preview_line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

fn status_preview(
    kind: PreviewKind,
    detail: impl Into<String>,
    lines: impl IntoIterator<Item = Line<'static>>,
) -> PreviewContent {
    PreviewContent::new(kind, lines.into_iter().collect()).with_detail(detail)
}

pub(super) fn build_preview(entry: &Entry) -> PreviewContent {
    if entry.is_dir() {
        return build_directory_preview(entry);
    }

    let facts = file_facts::inspect_path(&entry.path, entry.kind);
    let preview_spec = facts.preview;
    let type_detail = facts.specific_type_label;
    if preview_spec.kind == file_facts::PreviewKind::Iso
        && let Some(preview) = build_iso_preview(&entry.path)
    {
        return preview;
    }
    if preview_spec.kind == file_facts::PreviewKind::Torrent
        && let Some(preview) = build_torrent_preview(&entry.path)
    {
        return preview;
    }
    if let Some(document_format) = preview_spec.document_format
        && let Some(preview) = document::build_document_preview(&entry.path, document_format)
    {
        return apply_type_detail(preview, type_detail);
    }

    let text_preview = match read_text_preview(&entry.path) {
        Ok(Some(text)) => text,
        Ok(None) => return apply_type_detail(binary_preview(), type_detail),
        Err(error) => {
            return apply_type_detail(unavailable_file_preview(&error), type_detail);
        }
    };
    let source_line_count = count_source_lines(&text_preview.text);
    let line_truncated = source_line_count > PREVIEW_RENDER_LINE_LIMIT;
    let mut preview_truncation_note = truncation_note(text_preview.bytes_truncated, line_truncated);

    if preview_spec.kind == file_facts::PreviewKind::Markdown {
        let preview = PreviewContent::new(
            PreviewKind::Markdown,
            markdown::render_markdown_preview(&text_preview.text),
        );
        return finalize_text_preview(
            preview,
            source_line_count,
            text_preview.bytes_truncated,
            preview_truncation_note,
        );
    }

    if preview_spec.kind == file_facts::PreviewKind::Source {
        if let Some(structured_format) = preview_spec.structured_format {
            let structured_attempt = structured::render_structured_preview(
                &text_preview.text,
                structured_format,
                text_preview.bytes_truncated,
            );
            preview_truncation_note =
                combine_preview_notes(preview_truncation_note, structured_attempt.note.as_deref());

            if let Some(structured_preview) = structured_attempt.preview {
                let preview = PreviewContent::new(PreviewKind::Code, structured_preview.lines)
                    .with_detail(structured_preview.detail);
                return finalize_text_preview(
                    preview,
                    source_line_count,
                    false,
                    combine_preview_notes(
                        preview_truncation_note,
                        structured_preview.truncation_note.as_deref(),
                    ),
                );
            }
        }

        if preview_spec.force_fallback
            && let Some(fallback_syntax) = preview_spec.fallback_syntax
        {
            let preview = PreviewContent::new(
                PreviewKind::Code,
                fallback::render_fallback_code_preview(&text_preview.text, fallback_syntax, true),
            )
            .with_detail(fallback_syntax.detail_label());
            return finalize_text_preview(
                preview,
                source_line_count,
                text_preview.bytes_truncated,
                preview_truncation_note.clone(),
            );
        }

        if let Some(syntax) = syntax::find_code_syntax(&entry.path, preview_spec.syntax_hint) {
            let preview = PreviewContent::new(
                PreviewKind::Code,
                syntax::render_code_preview(
                    &entry.path,
                    &text_preview.text,
                    preview_spec.syntax_hint,
                    true,
                ),
            )
            .with_detail(syntax.name.clone());
            return finalize_text_preview(
                preview,
                source_line_count,
                text_preview.bytes_truncated,
                preview_truncation_note.clone(),
            );
        }

        if let Some(fallback_syntax) = preview_spec.fallback_syntax {
            let preview = PreviewContent::new(
                PreviewKind::Code,
                fallback::render_fallback_code_preview(&text_preview.text, fallback_syntax, true),
            )
            .with_detail(fallback_syntax.detail_label());
            return finalize_text_preview(
                preview,
                source_line_count,
                text_preview.bytes_truncated,
                preview_truncation_note,
            );
        }
    }

    let preview = PreviewContent::new(
        PreviewKind::Text,
        render_plain_text_preview(&text_preview.text),
    );
    finalize_text_preview(
        apply_type_detail(preview, type_detail),
        source_line_count,
        text_preview.bytes_truncated,
        preview_truncation_note,
    )
}

fn build_directory_preview(entry: &Entry) -> PreviewContent {
    match fs::read_dir(&entry.path) {
        Ok(children) => {
            let mut items = children
                .flatten()
                .map(|child| {
                    let path = child.path();
                    let file_name = child.file_name().to_string_lossy().to_string();
                    let is_dir = path.is_dir();
                    (file_name, path, is_dir)
                })
                .collect::<Vec<_>>();
            items.sort_by(|left, right| {
                right
                    .2
                    .cmp(&left.2)
                    .then_with(|| left.0.to_lowercase().cmp(&right.0.to_lowercase()))
            });

            if items.is_empty() {
                return status_preview(
                    PreviewKind::Directory,
                    "0 items",
                    [Line::from("Folder is empty")],
                );
            }

            let palette = appearance::palette();
            let total_items = items.len();
            let folder_count = items.iter().filter(|item| item.2).count();
            let file_count = total_items.saturating_sub(folder_count);
            let mut lines = Vec::new();
            for (name, path, is_dir) in items.into_iter().take(PREVIEW_RENDER_LINE_LIMIT) {
                let appearance = appearance::resolve_path(
                    &path,
                    if is_dir {
                        EntryKind::Directory
                    } else {
                        EntryKind::File
                    },
                );
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", appearance.icon),
                        Style::default()
                            .fg(appearance.color)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                    Span::styled(name, Style::default().fg(palette.text)),
                ]));
            }

            PreviewContent::new(PreviewKind::Directory, lines)
                .with_detail(format!("{total_items} items"))
                .with_directory_counts(total_items, folder_count, file_count)
        }
        Err(error) => unavailable_directory_preview(&error),
    }
}

fn build_iso_preview(path: &Path) -> Option<PreviewContent> {
    let output = Command::new("bsdtar").arg("-tf").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let listing = String::from_utf8(output.stdout).ok()?;
    let items = listing
        .lines()
        .map(trim_trailing_line_endings)
        .map(|line| line.trim().trim_start_matches("./").to_string())
        .filter(|line| !line.is_empty() && line != "." && line != "./")
        .collect::<Vec<_>>();

    Some(build_listing_preview(&items, "Image is empty"))
}

fn build_torrent_preview(path: &Path) -> Option<PreviewContent> {
    const TORRENT_PREVIEW_LIMIT_BYTES: u64 = 1024 * 1024;

    let mut bytes = Vec::with_capacity(TORRENT_PREVIEW_LIMIT_BYTES as usize);
    File::open(path)
        .ok()?
        .take(TORRENT_PREVIEW_LIMIT_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;

    let mut index = 0usize;
    let mut metadata = TorrentMetadata::default();
    parse_torrent_root(&bytes, &mut index, &mut metadata)?;

    let mut lines = Vec::new();
    push_preview_line(
        &mut lines,
        "Name",
        metadata.name.unwrap_or_else(|| "unknown".to_string()),
    );
    if let Some(tracker) = metadata.announce {
        push_preview_line(&mut lines, "Tracker", tracker);
    }
    let files = if metadata.file_count > 0 {
        metadata.file_count
    } else {
        1
    };
    push_preview_line(&mut lines, "Files", files.to_string());
    if let Some(total_size) = metadata.total_size {
        push_preview_line(&mut lines, "Size", crate::app::format_size(total_size));
    }
    if let Some(created_by) = metadata.created_by {
        push_preview_line(&mut lines, "Created", created_by);
    }
    if let Some(comment) = metadata.comment {
        push_preview_line(&mut lines, "Comment", comment);
    }

    let file_count = files;
    Some(
        PreviewContent::new(PreviewKind::Text, lines)
            .with_detail("BitTorrent file")
            .with_directory_counts(file_count, 0, file_count),
    )
}

fn build_listing_preview(items: &[String], empty_label: &str) -> PreviewContent {
    if items.is_empty() {
        return status_preview(
            PreviewKind::Directory,
            "0 items",
            [Line::from(empty_label.to_string())],
        );
    }

    let palette = appearance::palette();
    let total_items = items.len();
    let folder_count = items
        .iter()
        .filter(|item| item.ends_with('/') || item.ends_with('\\'))
        .count();
    let file_count = total_items.saturating_sub(folder_count);
    let mut lines = Vec::new();

    for item in items.iter().take(PREVIEW_RENDER_LINE_LIMIT) {
        let is_dir = item.ends_with('/') || item.ends_with('\\');
        let display_name = item.trim_end_matches(['/', '\\']);
        let appearance = appearance::resolve_path(
            Path::new(display_name),
            if is_dir {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
        );
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", appearance.icon),
                Style::default()
                    .fg(appearance.color)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::styled(item.clone(), Style::default().fg(palette.text)),
        ]));
    }

    PreviewContent::new(PreviewKind::Directory, lines)
        .with_detail(format!("{total_items} items"))
        .with_directory_counts(total_items, folder_count, file_count)
}

fn push_preview_line(lines: &mut Vec<Line<'static>>, label: &str, value: String) {
    let palette = appearance::palette();
    lines.push(Line::from(vec![
        Span::styled(format!("{label:<8}"), Style::default().fg(palette.muted)),
        Span::styled(value, Style::default().fg(palette.text)),
    ]));
}

fn render_plain_text_preview(text: &str) -> Vec<Line<'static>> {
    let palette = appearance::palette();
    let mut rendered = Vec::new();

    for line in collect_preview_lines(text) {
        rendered.push(Line::from(Span::styled(
            expand_tabs(&line),
            Style::default().fg(palette.text),
        )));
    }

    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
}

fn collect_preview_lines(text: &str) -> Vec<String> {
    text.lines()
        .take(PREVIEW_RENDER_LINE_LIMIT)
        .map(trim_trailing_line_endings)
        .collect()
}

fn count_source_lines(text: &str) -> usize {
    text.lines().count().max(1)
}

fn finalize_text_preview(
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

fn apply_type_detail(
    mut preview: PreviewContent,
    type_detail: Option<&'static str>,
) -> PreviewContent {
    if let Some(detail) = type_detail
        && matches!(
            preview.detail.as_deref(),
            None | Some("Binary file") | Some("Read error")
        )
    {
        preview.detail = Some(detail.to_string());
    }
    preview
}

fn truncation_note(bytes_truncated: bool, line_truncated: bool) -> Option<String> {
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

fn combine_preview_notes(current: Option<String>, extra: Option<&str>) -> Option<String> {
    match (current, extra) {
        (Some(current), Some(extra)) => Some(format!("{current}  •  {extra}")),
        (Some(current), None) => Some(current),
        (None, Some(extra)) => Some(extra.to_string()),
        (None, None) => None,
    }
}

fn binary_preview() -> PreviewContent {
    status_preview(
        PreviewKind::Binary,
        "Binary file",
        [
            Line::from("No text preview available"),
            Line::from("Binary or unsupported file"),
        ],
    )
}

fn unavailable_preview(detail: &str, message: &str) -> PreviewContent {
    status_preview(
        PreviewKind::Unavailable,
        detail,
        [
            Line::from("Preview unavailable"),
            Line::from(message.to_string()),
        ],
    )
}

fn unavailable_file_preview(error: &anyhow::Error) -> PreviewContent {
    let io_error = error.downcast_ref::<std::io::Error>();
    let detail = io_error.map_or("Read error", support::describe_io_error);
    let message = match io_error.map(std::io::Error::kind) {
        Some(std::io::ErrorKind::PermissionDenied) => {
            "You do not have permission to read this file"
        }
        Some(std::io::ErrorKind::NotFound) => "This file is no longer available",
        Some(std::io::ErrorKind::Unsupported) => "This location is not supported",
        _ => "The file could not be read",
    };
    unavailable_preview(detail, message)
}

fn unavailable_directory_preview(error: &std::io::Error) -> PreviewContent {
    let detail = support::describe_io_error(error);
    let message = match error.kind() {
        std::io::ErrorKind::PermissionDenied => "You do not have permission to open this folder",
        std::io::ErrorKind::NotFound => "This folder is no longer available",
        std::io::ErrorKind::Unsupported => "This location is not supported",
        _ => "Folder preview unavailable",
    };
    unavailable_preview(detail, message)
}

fn trim_trailing_line_endings(line: &str) -> String {
    line.trim_end_matches(['\n', '\r']).to_string()
}

fn read_text_preview(path: &Path) -> anyhow::Result<Option<TextPreview>> {
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

fn line_number_span(number: usize, width: usize) -> Span<'static> {
    let preview = appearance::code_preview_palette();
    Span::styled(
        format!("{number:>width$} ", width = width),
        Style::default().fg(preview.line_number),
    )
}

fn line_number_width(lines: usize) -> usize {
    lines.max(1).to_string().len().max(2)
}

fn expand_tabs(text: &str) -> String {
    text.replace('\t', "    ")
}

fn parse_torrent_root(
    bytes: &[u8],
    index: &mut usize,
    metadata: &mut TorrentMetadata,
) -> Option<()> {
    expect_byte(bytes, index, b'd')?;
    while peek_byte(bytes, *index)? != b'e' {
        let key = parse_bencode_bytes(bytes, index)?;
        match key {
            b"announce" => metadata.announce = parse_bencode_string(bytes, index),
            b"comment" => metadata.comment = parse_bencode_string(bytes, index),
            b"created by" => metadata.created_by = parse_bencode_string(bytes, index),
            b"info" => parse_torrent_info(bytes, index, metadata)?,
            _ => skip_bencode_value(bytes, index)?,
        }
    }
    expect_byte(bytes, index, b'e')
}

fn parse_torrent_info(
    bytes: &[u8],
    index: &mut usize,
    metadata: &mut TorrentMetadata,
) -> Option<()> {
    expect_byte(bytes, index, b'd')?;
    while peek_byte(bytes, *index)? != b'e' {
        let key = parse_bencode_bytes(bytes, index)?;
        match key {
            b"name" => metadata.name = parse_bencode_string(bytes, index),
            b"length" => {
                let length = parse_bencode_int(bytes, index)?;
                if length >= 0 {
                    metadata.total_size = Some(length as u64);
                    if metadata.file_count == 0 {
                        metadata.file_count = 1;
                    }
                }
            }
            b"files" => parse_torrent_files(bytes, index, metadata)?,
            _ => skip_bencode_value(bytes, index)?,
        }
    }
    expect_byte(bytes, index, b'e')
}

fn parse_torrent_files(
    bytes: &[u8],
    index: &mut usize,
    metadata: &mut TorrentMetadata,
) -> Option<()> {
    expect_byte(bytes, index, b'l')?;
    let mut file_count = 0usize;
    let mut total_size = 0u64;
    while peek_byte(bytes, *index)? != b'e' {
        let length = parse_torrent_file_entry(bytes, index)?;
        file_count += 1;
        total_size = total_size.saturating_add(length);
    }
    expect_byte(bytes, index, b'e')?;
    metadata.file_count = file_count;
    metadata.total_size = Some(total_size);
    Some(())
}

fn parse_torrent_file_entry(bytes: &[u8], index: &mut usize) -> Option<u64> {
    expect_byte(bytes, index, b'd')?;
    let mut length = 0u64;
    while peek_byte(bytes, *index)? != b'e' {
        let key = parse_bencode_bytes(bytes, index)?;
        if key == b"length" {
            let value = parse_bencode_int(bytes, index)?;
            if value >= 0 {
                length = value as u64;
            }
        } else {
            skip_bencode_value(bytes, index)?;
        }
    }
    expect_byte(bytes, index, b'e')?;
    Some(length)
}

fn skip_bencode_value(bytes: &[u8], index: &mut usize) -> Option<()> {
    match peek_byte(bytes, *index)? {
        b'd' => {
            *index += 1;
            while peek_byte(bytes, *index)? != b'e' {
                parse_bencode_bytes(bytes, index)?;
                skip_bencode_value(bytes, index)?;
            }
            *index += 1;
            Some(())
        }
        b'l' => {
            *index += 1;
            while peek_byte(bytes, *index)? != b'e' {
                skip_bencode_value(bytes, index)?;
            }
            *index += 1;
            Some(())
        }
        b'i' => {
            parse_bencode_int(bytes, index)?;
            Some(())
        }
        b'0'..=b'9' => {
            parse_bencode_bytes(bytes, index)?;
            Some(())
        }
        _ => None,
    }
}

fn parse_bencode_string(bytes: &[u8], index: &mut usize) -> Option<String> {
    let value = parse_bencode_bytes(bytes, index)?;
    Some(String::from_utf8_lossy(value).into_owned())
}

fn parse_bencode_bytes<'a>(bytes: &'a [u8], index: &mut usize) -> Option<&'a [u8]> {
    let start = *index;
    while peek_byte(bytes, *index)?.is_ascii_digit() {
        *index += 1;
    }
    let colon = *index;
    expect_byte(bytes, index, b':')?;
    let len = std::str::from_utf8(&bytes[start..colon])
        .ok()?
        .parse::<usize>()
        .ok()?;
    let end = (*index).checked_add(len)?;
    let slice = bytes.get(*index..end)?;
    *index = end;
    Some(slice)
}

fn parse_bencode_int(bytes: &[u8], index: &mut usize) -> Option<i64> {
    expect_byte(bytes, index, b'i')?;
    let start = *index;
    while peek_byte(bytes, *index)? != b'e' {
        *index += 1;
    }
    let value = std::str::from_utf8(bytes.get(start..*index)?)
        .ok()?
        .parse()
        .ok()?;
    expect_byte(bytes, index, b'e')?;
    Some(value)
}

fn peek_byte(bytes: &[u8], index: usize) -> Option<u8> {
    bytes.get(index).copied()
}

fn expect_byte(bytes: &[u8], index: &mut usize, expected: u8) -> Option<()> {
    if peek_byte(bytes, *index)? == expected {
        *index += 1;
        Some(())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-preview-{label}-{unique}"))
    }

    fn file_entry(path: PathBuf) -> Entry {
        Entry {
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            name_key: path.file_name().unwrap().to_string_lossy().to_lowercase(),
            path,
            kind: EntryKind::File,
            size: 0,
            modified: None,
            readonly: false,
        }
    }

    fn directory_entry(path: PathBuf) -> Entry {
        Entry {
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            name_key: path.file_name().unwrap().to_string_lossy().to_lowercase(),
            path,
            kind: EntryKind::Directory,
            size: 0,
            modified: None,
            readonly: false,
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    fn write_zip_entries(path: &Path, entries: &[(&str, &str)]) {
        let file = File::create(path).expect("failed to create zip");
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (name, contents) in entries {
            zip.start_file(name, options)
                .expect("failed to start zip entry");
            zip.write_all(contents.as_bytes())
                .expect("failed to write zip entry");
        }

        zip.finish().expect("failed to finish zip");
    }

    #[test]
    fn markdown_preview_formats_headings_and_lists() {
        let root = temp_path("markdown");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("README.md");
        fs::write(&path, "# Heading\n- item\n`inline`\n").expect("failed to write markdown");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Markdown);
        assert_eq!(preview.lines[0].spans[0].content, "Heading");
        assert!(
            preview
                .lines
                .iter()
                .any(|line| line.spans.iter().any(|span| span.content == "inline"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn markdown_preview_formats_inline_emphasis_mid_line() {
        let root = temp_path("markdown-inline");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("README.md");
        fs::write(&path, "hello **bold** world\n").expect("failed to write markdown");

        let preview = build_preview(&file_entry(path));
        let line = &preview.lines[0];

        assert_eq!(preview.kind, PreviewKind::Markdown);
        assert!(line.spans.iter().any(|span| span.content == "hello "));
        assert!(line.spans.iter().any(|span| span.content == "bold"));
        assert!(line.spans.iter().any(|span| span.content == " world"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn markdown_preview_renders_fenced_code_blocks() {
        let root = temp_path("markdown-fence");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("README.md");
        fs::write(&path, "```rust\nfn main() {}\n```\n").expect("failed to write markdown");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Markdown);
        assert_eq!(preview.lines[0].spans[1].content, "rust");
        assert!(
            preview
                .lines
                .iter()
                .any(|line| line_text(line).contains("fn main() {}"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn markdown_preview_renders_links() {
        let root = temp_path("markdown-links");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("README.md");
        fs::write(&path, "open [elio](https://example.com)\n").expect("failed to write markdown");

        let preview = build_preview(&file_entry(path));
        let line = &preview.lines[0];

        assert_eq!(preview.kind, PreviewKind::Markdown);
        let link_span = line
            .spans
            .iter()
            .find(|span| span.content == "elio")
            .expect("link label should be rendered");
        assert!(link_span.style.add_modifier.contains(Modifier::UNDERLINED));
        assert!(line_text(line).contains("(https://example.com)"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn markdown_preview_adds_spacing_between_blocks() {
        let root = temp_path("markdown-spacing");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("README.md");
        fs::write(
            &path,
            "# Heading\nParagraph text\n\n```rust\nlet x = 1;\n```\n",
        )
        .expect("failed to write markdown");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Markdown);
        assert!(preview.lines.iter().any(|line| line.spans.is_empty()));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn markdown_preview_renders_nested_emphasis() {
        let root = temp_path("markdown-nested");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("README.md");
        fs::write(&path, "**bold and *italic***\n").expect("failed to write markdown");

        let preview = build_preview(&file_entry(path));
        let line = &preview.lines[0];

        assert_eq!(preview.kind, PreviewKind::Markdown);
        let italic_span = line
            .spans
            .iter()
            .find(|span| span.content == "italic")
            .expect("nested italic content should be rendered");
        assert!(italic_span.style.add_modifier.contains(Modifier::BOLD));
        assert!(italic_span.style.add_modifier.contains(Modifier::ITALIC));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn markdown_preview_renders_mixed_lists() {
        let root = temp_path("markdown-mixed-lists");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("README.md");
        fs::write(&path, "1. first\n   - nested\n2. second\n").expect("failed to write markdown");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Markdown);
        assert!(
            preview
                .lines
                .iter()
                .any(|line| line.spans.iter().any(|span| span.content == "1. "))
        );
        assert!(
            preview
                .lines
                .iter()
                .any(|line| line.spans.iter().any(|span| span.content.contains("• ")))
        );
        assert!(
            preview
                .lines
                .iter()
                .any(|line| line.spans.iter().any(|span| span.content == "2. "))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn code_preview_includes_line_numbers() {
        let root = temp_path("code");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("main.rs");
        fs::write(&path, "fn main() {}\n").expect("failed to write code");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.lines[0].spans[0].content.contains("1"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn html_preview_uses_code_renderer() {
        let root = temp_path("html");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("index.html");
        fs::write(
            &path,
            "<!DOCTYPE html>\n<div class=\"app\" data-id=\"42\">elio</div>\n",
        )
        .expect("failed to write html");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some());
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("div"))
        );
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("class"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn css_preview_uses_code_renderer() {
        let root = temp_path("css");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("styles.css");
        fs::write(&path, ".app {\n  color: #fff;\n  margin: 12px;\n}\n")
            .expect("failed to write css");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some());
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("color"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn docx_preview_shows_document_metadata() {
        let root = temp_path("docx");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("report.docx");
        write_zip_entries(
            &path,
            &[
                (
                    "docProps/core.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/"
                        xmlns:dcterms="http://purl.org/dc/terms/">
                      <dc:title>Quarterly Report</dc:title>
                      <dc:creator>Regueiro</dc:creator>
                      <dcterms:created>2026-03-11T09:00:00Z</dcterms:created>
                    </cp:coreProperties>"#,
                ),
                (
                    "docProps/app.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>LibreOffice</Application>
                      <Pages>12</Pages>
                      <Words>4238</Words>
                    </Properties>"#,
                ),
            ],
        );

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some("DOCX document"));
        assert_eq!(line_texts[0], "Document");
        assert!(
            line_texts
                .iter()
                .all(|text| !text.contains("Format") || !text.contains("DOCX document"))
        );
        assert!(line_texts.iter().any(|text| text == "People"));
        assert!(line_texts.iter().any(|text| text == "Dates"));
        assert!(line_texts.iter().any(|text| text == "Stats"));
        assert!(
            line_texts
                .iter()
                .any(|text| text.contains("Quarterly Report"))
        );
        assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
        assert!(line_texts.iter().any(|text| text.contains("4,238")));
        assert!(
            line_texts
                .iter()
                .any(|text| text.contains("Created") && text.contains("Mar 11, 2026 09:00 UTC"))
        );
        assert!(
            line_texts
                .iter()
                .all(|text| !text.contains("2026-03-11T09:00:00Z"))
        );
        assert!(
            line_texts
                .iter()
                .any(|text| text.contains("Application") && text.contains("LibreOffice"))
        );
        assert!(
            line_texts
                .iter()
                .all(|text| !text.contains("ApplicationLibreOffice"))
        );
        assert!(
            line_texts
                .iter()
                .position(|text| text == "Document")
                .unwrap()
                < line_texts.iter().position(|text| text == "People").unwrap()
        );
        assert!(
            line_texts.iter().position(|text| text == "People").unwrap()
                < line_texts.iter().position(|text| text == "Dates").unwrap()
        );
        assert!(
            line_texts.iter().position(|text| text == "Dates").unwrap()
                < line_texts.iter().position(|text| text == "Stats").unwrap()
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn odt_preview_shows_document_metadata() {
        let root = temp_path("odt");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("report.odt");
        write_zip_entries(
            &path,
            &[(
                "meta.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                    xmlns:dc="http://purl.org/dc/elements/1.1/"
                    xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
                  <office:meta>
                    <dc:title>Project Notes</dc:title>
                    <meta:initial-creator>Elio</meta:initial-creator>
                    <meta:creation-date>2026-03-10T18:00:00Z</meta:creation-date>
                    <meta:generator>LibreOffice</meta:generator>
                    <meta:document-statistic meta:page-count="3" meta:word-count="980" meta:character-count="6400"/>
                  </office:meta>
                </office:document-meta>"#,
            )],
        );

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some("ODT document"));
        assert_eq!(line_texts[0], "Document");
        assert!(line_texts.iter().any(|text| text == "People"));
        assert!(line_texts.iter().any(|text| text == "Dates"));
        assert!(line_texts.iter().any(|text| text == "Stats"));
        assert!(line_texts.iter().any(|text| text.contains("Project Notes")));
        assert!(line_texts.iter().any(|text| text.contains("LibreOffice")));
        assert!(line_texts.iter().any(|text| text.contains("980")));
        assert!(line_texts.iter().any(|text| text.contains("6,400")));
        assert!(
            line_texts
                .iter()
                .any(|text| text.contains("Created") && text.contains("Mar 10, 2026 18:00 UTC"))
        );
        assert!(
            line_texts
                .iter()
                .all(|text| !text.contains("2026-03-10T18:00:00Z"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn xml_preview_uses_code_renderer() {
        let root = temp_path("xml");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("layout.xml");
        fs::write(&path, "<?xml version=\"1.0\"?>\n<layout id=\"main\" />\n")
            .expect("failed to write xml");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some());
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("layout"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn toml_preview_uses_structured_renderer() {
        let root = temp_path("toml");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("config.toml");
        fs::write(&path, "name = \"elio\"\n[server]\nport = 3000\n").expect("failed to write toml");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert_eq!(preview.detail.as_deref(), Some("TOML (structured)"));
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("server"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn desktop_preview_uses_code_renderer() {
        let root = temp_path("desktop-entry");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("app.desktop");
        fs::write(
            &path,
            "[Desktop Entry]\nName=エリオ\nName[ja]=エリオ\nExec=elio\nTerminal=false\n",
        )
        .expect("failed to write desktop entry");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(
            preview
                .detail
                .as_deref()
                .is_some_and(|detail| detail == "Desktop Entry (best-effort)")
        );
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("エリオ"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn pkgbuild_preview_uses_shell_renderer() {
        let root = temp_path("pkgbuild");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("PKGBUILD");
        fs::write(
            &path,
            "pkgname=elio\nbuild() {\n  cargo build --release\n}\n",
        )
        .expect("failed to write pkgbuild");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn keys_preview_uses_fallback_renderer() {
        let root = temp_path("keys");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("bindings.keys");
        fs::write(&path, "ctrl+h=left\nctrl+l=right\n").expect("failed to write keys");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(
            preview
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("INI"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn log_preview_uses_structured_renderer() {
        let root = temp_path("log");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("server.log");
        fs::write(
            &path,
            "2026-03-10T12:00:00Z ERROR request_id=42 path=/login failed\n",
        )
        .expect("failed to write log");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert_eq!(preview.detail.as_deref(), Some("Log (structured)"));
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("ERROR"))
        );
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("request_id"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn torrent_preview_shows_basic_metadata() {
        let root = temp_path("torrent");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("sample.torrent");
        let bytes = b"d8:announce20:https://tracker.test7:comment12:test torrent10:created by4:elio4:infod6:lengthi12345e4:name8:file.txt12:piece lengthi262144e6:pieces20:12345678901234567890ee";
        fs::write(&path, bytes).expect("failed to write torrent");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Text);
        assert_eq!(preview.detail.as_deref(), Some("BitTorrent file"));
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("file.txt"))
        );
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("tracker.test"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn srt_preview_keeps_specific_type_detail() {
        let root = temp_path("srt");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("movie.srt");
        fs::write(&path, "1\n00:00:01,000 --> 00:00:02,000\nHello\n").expect("failed to write srt");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Text);
        assert_eq!(preview.detail.as_deref(), Some("SubRip subtitles"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn iso_binary_preview_keeps_specific_type_detail() {
        let root = temp_path("iso");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("disk.iso");
        fs::write(&path, [0xff, 0xfe, 0x00, 0x81]).expect("failed to write iso");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Binary);
        assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn iso_preview_lists_contents_when_bsdtar_can_read_image() {
        let root = temp_path("iso-listing");
        let image_root = root.join("image-root");
        fs::create_dir_all(image_root.join("docs")).expect("failed to create image tree");
        fs::write(image_root.join("docs/readme.txt"), "hello").expect("failed to write image file");
        let path = root.join("sample.iso");

        let created = Command::new("bsdtar")
            .arg("-cf")
            .arg(&path)
            .arg("-C")
            .arg(&image_root)
            .arg(".")
            .status();
        if !created.as_ref().is_ok_and(|status| status.success()) {
            fs::remove_dir_all(root).expect("failed to remove temp root");
            return;
        }

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Directory);
        assert_eq!(preview.item_count, Some(2));
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("docs/"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn typescript_preview_uses_code_renderer() {
        let root = temp_path("typescript");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("main.ts");
        fs::write(&path, "export const value = 1;\n").expect("failed to write ts");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn tsx_preview_uses_code_renderer() {
        let root = temp_path("tsx");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("App.tsx");
        fs::write(
            &path,
            "export function App() { return <div>Hello</div>; }\n",
        )
        .expect("failed to write tsx");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn cargo_lock_preview_uses_code_renderer() {
        let root = temp_path("cargo-lock");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("Cargo.lock");
        fs::write(&path, "version = 3\n").expect("failed to write cargo lock");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(
            preview
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("TOML"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn json_preview_formats_minified_content() {
        let root = temp_path("json");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("package.json");
        fs::write(&path, "{\"name\":\"elio\",\"nested\":{\"enabled\":true}}\n")
            .expect("failed to write json");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert_eq!(preview.detail.as_deref(), Some("JSON (structured)"));
        assert_eq!(preview.source_lines, Some(1));
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("nested"))
        );
        assert!(preview.lines.len() > 1);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn truncated_json_preview_reports_why_formatting_was_skipped() {
        let root = temp_path("json-truncated");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("package.json");
        let oversized = format!("{{\"value\":\"{}\"}}", "a".repeat(PREVIEW_LIMIT_BYTES));
        fs::write(&path, oversized).expect("failed to write oversized json");

        let preview = build_preview(&file_entry(path));
        let header = preview
            .header_detail(0, 12)
            .expect("formatted header detail should be present");

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(
            header.contains("structured preview skipped: input truncated"),
            "unexpected header: {header}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn dotenv_preview_uses_structured_renderer() {
        let root = temp_path("dotenv");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join(".env.local");
        fs::write(&path, "APP_ENV=dev\nPORT=3000\n").expect("failed to write dotenv file");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(
            preview
                .detail
                .as_deref()
                .is_some_and(|detail| detail == ".env (structured)")
        );
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("APP_ENV"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn jsonc_preview_uses_structured_renderer() {
        let root = temp_path("jsonc");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("deno.jsonc");
        fs::write(&path, "{\n  // comment\n  \"name\": \"elio\",\n}\n")
            .expect("failed to write jsonc");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert_eq!(preview.detail.as_deref(), Some("JSONC (structured)"));
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("name"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn json5_preview_uses_structured_renderer() {
        let root = temp_path("json5");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("config.json5");
        fs::write(&path, "{\n  trailing: true,\n  list: [1, 2,],\n}\n")
            .expect("failed to write json5");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert_eq!(preview.detail.as_deref(), Some("JSON5 (structured)"));
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("trailing"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn yaml_preview_uses_structured_renderer() {
        let root = temp_path("yaml");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("docker-compose.yaml");
        fs::write(
            &path,
            "services:\n  app:\n    image: elio:latest\n    ports:\n      - \"3000:3000\"\n",
        )
        .expect("failed to write yaml");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert_eq!(preview.detail.as_deref(), Some("YAML (structured)"));
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.content.contains("services"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn text_preview_stays_plain() {
        let root = temp_path("text");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("notes.txt");
        fs::write(&path, "hello\nworld\n").expect("failed to write text");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Text);
        assert_eq!(preview.lines[0].spans.len(), 1);
        assert_eq!(preview.lines[0].spans[0].content, "hello");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn text_preview_keeps_enough_lines_for_scrolling() {
        let root = temp_path("scroll-depth");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("long.txt");
        let text = (1..=80)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, text).expect("failed to write long text");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Text);
        assert!(preview.lines.len() >= 80);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn utf8_preview_trims_to_last_valid_boundary() {
        let root = temp_path("utf8-boundary");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("unicode.txt");
        let bytes = [
            "a".repeat(PREVIEW_LIMIT_BYTES - 1).into_bytes(),
            "é".as_bytes().to_vec(),
        ]
        .concat();
        fs::write(&path, bytes).expect("failed to write unicode text");

        let preview = read_text_preview(&path)
            .expect("preview read should succeed")
            .expect("utf8 text should stay text");

        assert!(preview.bytes_truncated);
        assert_eq!(preview.text.len(), PREVIEW_LIMIT_BYTES - 1);
        assert!(preview.text.chars().all(|ch| ch == 'a'));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn utf8_text_file_is_not_mislabeled_as_binary() {
        let root = temp_path("utf8-text-kind");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("unicode.txt");
        let bytes = [
            "a".repeat(PREVIEW_LIMIT_BYTES - 1).into_bytes(),
            "é".as_bytes().to_vec(),
        ]
        .concat();
        fs::write(&path, bytes).expect("failed to write unicode text");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Text);
        assert!(preview.truncated);
        assert!(preview.lines.iter().all(|line| {
            line.spans
                .iter()
                .all(|span| span.content != "No text preview available")
        }));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn byte_truncated_preview_reports_truncation_without_fake_line_totals() {
        let root = temp_path("byte-truncated");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("notes.txt");
        fs::write(&path, "a".repeat(PREVIEW_LIMIT_BYTES + 32)).expect("failed to write text");

        let preview = build_preview(&file_entry(path));
        let header = preview
            .header_detail(0, 20)
            .expect("header detail should be present");

        assert_eq!(preview.kind, PreviewKind::Text);
        assert!(preview.truncated);
        assert!(preview.source_lines.is_none());
        assert!(header.contains("truncated to 64 KiB"));
        assert!(!header.contains("lines"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn line_truncated_preview_reports_visible_limit() {
        let root = temp_path("line-truncated");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("long.txt");
        let text = (1..=300)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, text).expect("failed to write long text");

        let preview = build_preview(&file_entry(path));
        let header = preview
            .header_detail(0, 20)
            .expect("header detail should be present");

        assert!(preview.truncated);
        assert_eq!(preview.source_lines, Some(300));
        assert!(header.contains("300 lines"));
        assert!(header.contains("showing first 240 lines"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    #[cfg(unix)]
    fn protected_directory_preview_reports_permission_denied() {
        let root = temp_path("protected-dir-preview");
        let locked = root.join("locked");
        fs::create_dir_all(&locked).expect("failed to create locked dir");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))
            .expect("failed to lock dir");

        let preview = build_preview(&directory_entry(locked.clone()));

        assert_eq!(preview.kind, PreviewKind::Unavailable);
        assert_eq!(preview.detail.as_deref(), Some("Permission denied"));
        assert!(
            preview
                .lines
                .iter()
                .any(|line| line_text(line).contains("permission"))
        );

        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))
            .expect("failed to unlock dir");
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    #[cfg(unix)]
    fn protected_file_preview_reports_permission_denied() {
        let root = temp_path("protected-file-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("secret.txt");
        fs::write(&path, "secret").expect("failed to write file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("failed to lock file");

        let preview = build_preview(&file_entry(path.clone()));

        assert_eq!(preview.kind, PreviewKind::Unavailable);
        assert_eq!(preview.detail.as_deref(), Some("Permission denied"));
        assert!(
            preview
                .lines
                .iter()
                .any(|line| line_text(line).contains("permission"))
        );

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("failed to unlock file");
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
