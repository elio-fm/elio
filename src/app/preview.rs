mod binary;
#[path = "preview/container.rs"]
mod container;
mod directory;
mod dispatch;
mod document;
mod fallback;
mod markdown;
mod structured;
mod syntax;
mod text;

use self::text::{
    collect_preview_lines, combine_preview_notes, count_source_lines, finalize_text_preview,
    read_text_preview, render_plain_text_preview, trim_trailing_line_endings, truncation_note,
};

use super::*;
use crate::appearance;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::collections::BTreeMap;

const PREVIEW_LIMIT_BYTES: usize = 64 * 1024;
const PREVIEW_RENDER_LINE_LIMIT: usize = 240;
const ARCHIVE_ENTRY_SCAN_LIMIT: usize = 50_000;
const ZIP_MANIFEST_LIMIT_BYTES: u64 = 64 * 1024;
const ISO_METADATA_SCAN_BYTES: u64 = 128 * 1024;
const ISO_DESCRIPTOR_START_SECTOR: usize = 16;
const ISO_SECTOR_SIZE: usize = 2048;
const ISO_BOOT_SYSTEM_ID: &str = "EL TORITO SPECIFICATION";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PreviewKind {
    Archive,
    Directory,
    Document,
    Image,
    Markdown,
    Code,
    Text,
    Binary,
    Unavailable,
}

impl PreviewKind {
    pub(super) fn section_label(self) -> &'static str {
        match self {
            Self::Archive => "Archive",
            Self::Directory => "Contents",
            Self::Document => "Document",
            Self::Image => "Image",
            Self::Markdown => "Markdown",
            Self::Code => "Code",
            Self::Text => "Text",
            Self::Binary | Self::Unavailable => "Preview",
        }
    }

    pub(super) fn wraps_in_preview(self) -> bool {
        matches!(
            self,
            Self::Document
                | Self::Image
                | Self::Markdown
                | Self::Text
                | Self::Binary
                | Self::Unavailable
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
    pub status_note: Option<String>,
    pub truncated: bool,
    pub truncation_note: Option<String>,
    pub source_lines: Option<usize>,
    pub item_count: Option<usize>,
    pub folder_count: Option<usize>,
    pub file_count: Option<usize>,
    pub lines: Vec<Line<'static>>,
}

#[derive(Default)]
struct TorrentMetadata {
    name: Option<String>,
    announce: Option<String>,
    announce_tiers: Vec<Vec<String>>,
    comment: Option<String>,
    created_by: Option<String>,
    piece_length: Option<u64>,
    piece_count: Option<usize>,
    private: Option<bool>,
    mode: Option<TorrentMode>,
    file_count: usize,
    total_size: Option<u64>,
    files: Vec<TorrentFileEntry>,
    file_sample_truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TorrentMode {
    SingleFile,
    MultiFile,
}

#[derive(Clone, Debug)]
struct TorrentFileEntry {
    path: String,
    length: u64,
}

#[derive(Default)]
struct IsoMetadata {
    system_id: Option<String>,
    volume_id: Option<String>,
    publisher_id: Option<String>,
    preparer_id: Option<String>,
    application_id: Option<String>,
    created_at: Option<String>,
    modified_at: Option<String>,
    effective_at: Option<String>,
    total_size: Option<u64>,
    bootable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArchiveEntry {
    path: String,
    is_dir: bool,
}

#[derive(Default)]
struct ArchiveTreeNode {
    path: String,
    is_dir: bool,
    children: BTreeMap<String, ArchiveTreeNode>,
}

#[derive(Default)]
struct ArchiveMetadata {
    format_label: Option<String>,
    physical_size: Option<u64>,
    compressed_size: Option<u64>,
    unpacked_size: Option<u64>,
    comment: Option<String>,
}

#[derive(Default)]
struct ZipManifestMetadata {
    title: Option<String>,
    version: Option<String>,
    main_class: Option<String>,
    created_by: Option<String>,
    automatic_module: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArchiveFormat {
    Zip,
    SevenZip,
    Tar,
    TarGzip,
    TarXz,
    TarBzip2,
    TarZstd,
    Gzip,
    Xz,
    Bzip2,
    Zstd,
    Deb,
    Rpm,
    AppImage,
    Unknown,
}

impl PreviewContent {
    pub(super) fn new(kind: PreviewKind, lines: Vec<Line<'static>>) -> Self {
        Self {
            kind,
            detail: None,
            status_note: None,
            truncated: false,
            truncation_note: None,
            source_lines: None,
            item_count: None,
            folder_count: None,
            file_count: None,
            lines: sanitize_preview_lines(lines),
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

    pub(super) fn with_status_note(mut self, note: impl Into<String>) -> Self {
        self.status_note = Some(note.into());
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
        let mut parts = Vec::new();
        if let Some(detail) = &self.detail
            && !detail.is_empty()
        {
            parts.push(support::sanitize_terminal_text(detail));
        }

        if let Some(note) = &self.status_note
            && !note.is_empty()
        {
            parts.push(support::sanitize_terminal_text(note));
        }

        if let Some(source_lines) = self.source_lines {
            parts.push(format!("{source_lines} lines"));
        }

        if let Some(note) = &self.truncation_note {
            parts.push(support::sanitize_terminal_text(note));
        }

        if !parts.is_empty() {
            return Some(parts.join("  •  "));
        }

        if self.kind == PreviewKind::Directory {
            return None;
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
        .map(|span| {
            support::sanitize_terminal_text(span.content.as_ref())
                .chars()
                .count()
        })
        .sum()
}

fn sanitize_preview_lines(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    lines.into_iter().map(sanitize_preview_line).collect()
}

fn sanitize_preview_line(mut line: Line<'static>) -> Line<'static> {
    for span in &mut line.spans {
        let sanitized = support::sanitize_terminal_text(span.content.as_ref());
        span.content = sanitized.into();
    }
    line
}

fn status_preview(
    kind: PreviewKind,
    detail: impl Into<String>,
    lines: impl IntoIterator<Item = Line<'static>>,
) -> PreviewContent {
    PreviewContent::new(kind, lines.into_iter().collect()).with_detail(detail)
}

pub(super) fn should_build_preview_in_background(entry: &Entry) -> bool {
    dispatch::should_build_preview_in_background(entry)
}

pub(super) fn loading_preview_for(entry: &Entry) -> PreviewContent {
    dispatch::loading_preview_for(entry)
}

pub(super) fn build_preview(entry: &Entry) -> PreviewContent {
    dispatch::build_preview(entry)
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
    support::sanitize_terminal_text(text)
}

#[cfg(test)]
mod tests;
