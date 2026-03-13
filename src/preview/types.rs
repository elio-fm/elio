use crate::fs as browser_support;
use crate::ui::theme;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::collections::BTreeMap;

pub(super) const PREVIEW_LIMIT_BYTES: usize = 64 * 1024;
pub(super) const PREVIEW_RENDER_LINE_LIMIT: usize = 240;
pub(super) const ARCHIVE_ENTRY_SCAN_LIMIT: usize = 50_000;
pub(super) const ZIP_MANIFEST_LIMIT_BYTES: u64 = 64 * 1024;
pub(super) const ISO_METADATA_SCAN_BYTES: u64 = 128 * 1024;
pub(super) const ISO_DESCRIPTOR_START_SECTOR: usize = 16;
pub(super) const ISO_SECTOR_SIZE: usize = 2048;
pub(super) const ISO_BOOT_SYSTEM_ID: &str = "EL TORITO SPECIFICATION";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewKind {
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
    pub(crate) fn section_label(self) -> &'static str {
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

    pub(crate) fn wraps_in_preview(self) -> bool {
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

    pub(crate) fn allows_horizontal_scroll(self) -> bool {
        self == Self::Code
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PreviewContent {
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
pub(super) struct TorrentMetadata {
    pub(super) name: Option<String>,
    pub(super) announce: Option<String>,
    pub(super) announce_tiers: Vec<Vec<String>>,
    pub(super) comment: Option<String>,
    pub(super) created_by: Option<String>,
    pub(super) piece_length: Option<u64>,
    pub(super) piece_count: Option<usize>,
    pub(super) private: Option<bool>,
    pub(super) mode: Option<TorrentMode>,
    pub(super) file_count: usize,
    pub(super) total_size: Option<u64>,
    pub(super) files: Vec<TorrentFileEntry>,
    pub(super) file_sample_truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TorrentMode {
    SingleFile,
    MultiFile,
}

#[derive(Clone, Debug)]
pub(super) struct TorrentFileEntry {
    pub(super) path: String,
    pub(super) length: u64,
}

#[derive(Default)]
pub(super) struct IsoMetadata {
    pub(super) system_id: Option<String>,
    pub(super) volume_id: Option<String>,
    pub(super) publisher_id: Option<String>,
    pub(super) preparer_id: Option<String>,
    pub(super) application_id: Option<String>,
    pub(super) created_at: Option<String>,
    pub(super) modified_at: Option<String>,
    pub(super) effective_at: Option<String>,
    pub(super) total_size: Option<u64>,
    pub(super) bootable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ArchiveEntry {
    pub(super) path: String,
    pub(super) is_dir: bool,
}

#[derive(Default)]
pub(super) struct ArchiveTreeNode {
    pub(super) path: String,
    pub(super) is_dir: bool,
    pub(super) children: BTreeMap<String, ArchiveTreeNode>,
}

#[derive(Default)]
pub(super) struct ArchiveMetadata {
    pub(super) format_label: Option<String>,
    pub(super) physical_size: Option<u64>,
    pub(super) compressed_size: Option<u64>,
    pub(super) unpacked_size: Option<u64>,
    pub(super) comment: Option<String>,
}

#[derive(Default)]
pub(super) struct ZipManifestMetadata {
    pub(super) title: Option<String>,
    pub(super) version: Option<String>,
    pub(super) main_class: Option<String>,
    pub(super) created_by: Option<String>,
    pub(super) automatic_module: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ArchiveFormat {
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
    pub(crate) fn new(kind: PreviewKind, lines: Vec<Line<'static>>) -> Self {
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

    pub(crate) fn placeholder(label: &str) -> Self {
        Self::new(
            PreviewKind::Unavailable,
            vec![Line::from(label.to_string())],
        )
    }

    pub(crate) fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub(crate) fn with_status_note(mut self, note: impl Into<String>) -> Self {
        self.status_note = Some(note.into());
        self
    }

    pub(crate) fn with_source_lines(mut self, source_lines: usize) -> Self {
        self.source_lines = Some(source_lines.max(1));
        self
    }

    pub(crate) fn with_truncation(mut self, note: impl Into<String>) -> Self {
        self.truncated = true;
        self.truncation_note = Some(note.into());
        self
    }

    pub(crate) fn with_directory_counts(
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

    pub(crate) fn section_label(&self) -> &'static str {
        self.kind.section_label()
    }

    pub(crate) fn total_lines(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn lines(&self) -> Vec<Line<'static>> {
        self.lines.clone()
    }

    pub(crate) fn visual_line_count(&self, width: usize) -> usize {
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

    pub(crate) fn max_line_width(&self) -> usize {
        self.lines.iter().map(preview_line_width).max().unwrap_or(0)
    }

    pub(crate) fn header_detail(&self, offset: usize, visible_rows: usize) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(detail) = &self.detail
            && !detail.is_empty()
        {
            parts.push(browser_support::sanitize_terminal_text(detail));
        }

        if let Some(note) = &self.status_note
            && !note.is_empty()
        {
            parts.push(browser_support::sanitize_terminal_text(note));
        }

        if let Some(source_lines) = self.source_lines {
            parts.push(format!("{source_lines} lines"));
        }

        if let Some(note) = &self.truncation_note {
            parts.push(browser_support::sanitize_terminal_text(note));
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
            browser_support::sanitize_terminal_text(span.content.as_ref())
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
        let sanitized = browser_support::sanitize_terminal_text(span.content.as_ref());
        span.content = sanitized.into();
    }
    line
}

pub(super) fn status_preview(
    kind: PreviewKind,
    detail: impl Into<String>,
    lines: impl IntoIterator<Item = Line<'static>>,
) -> PreviewContent {
    PreviewContent::new(kind, lines.into_iter().collect()).with_detail(detail)
}

pub(super) fn unavailable_directory_preview(error: &std::io::Error) -> PreviewContent {
    let detail = browser_support::describe_io_error(error);
    let message = match error.kind() {
        std::io::ErrorKind::PermissionDenied => "You do not have permission to open this folder",
        std::io::ErrorKind::NotFound => "This folder is no longer available",
        std::io::ErrorKind::Unsupported => "This location is not supported",
        _ => "Folder preview unavailable",
    };
    unavailable_preview(detail, message)
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

pub(super) fn line_number_span(number: usize, width: usize) -> Span<'static> {
    let preview = theme::code_preview_palette();
    Span::styled(
        format!("{number:>width$} ", width = width),
        Style::default().fg(preview.line_number),
    )
}

pub(super) fn line_number_width(lines: usize) -> usize {
    lines.max(1).to_string().len().max(2)
}

pub(super) fn expand_tabs(text: &str) -> String {
    browser_support::sanitize_terminal_text(text)
}
