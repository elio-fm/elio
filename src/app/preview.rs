mod binary;
#[path = "preview/container.rs"]
mod container;
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
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::Path,
};

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
    pub status_note: Option<String>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Utf16Endian {
    Little,
    Big,
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
    let facts = file_facts::inspect_path(&entry.path, entry.kind);
    facts.builtin_class == FileClass::Archive || facts.preview.document_format.is_some()
}

pub(super) fn loading_preview_for(entry: &Entry) -> PreviewContent {
    let facts = file_facts::inspect_path(&entry.path, entry.kind);
    let detail = facts
        .specific_type_label
        .or_else(|| {
            facts
                .preview
                .document_format
                .map(|format| format.detail_label())
        })
        .unwrap_or("Preview")
        .to_string();
    let lines = if facts.builtin_class == FileClass::Archive {
        vec![
            Line::from("Loading preview"),
            Line::from("Inspecting archive contents in background"),
        ]
    } else if facts.preview.document_format.is_some() {
        vec![
            Line::from("Loading preview"),
            Line::from("Extracting document metadata in background"),
        ]
    } else {
        vec![
            Line::from("Loading preview"),
            Line::from("Preparing file preview in background"),
        ]
    };
    PreviewContent::new(PreviewKind::Unavailable, lines).with_detail(detail)
}

pub(super) fn build_preview(entry: &Entry) -> PreviewContent {
    if entry.is_dir() {
        return build_directory_preview(entry);
    }

    let facts = file_facts::inspect_path(&entry.path, entry.kind);
    let preview_spec = facts.preview;
    let type_detail = facts.specific_type_label;
    if preview_spec.kind == file_facts::PreviewKind::Iso
        && let Some(preview) = container::build_iso_preview(&entry.path)
    {
        return preview;
    }
    if preview_spec.kind == file_facts::PreviewKind::Torrent
        && let Some(preview) = container::build_torrent_preview(&entry.path)
    {
        return preview;
    }
    if facts.builtin_class == FileClass::Archive
        && let Some(preview) = container::build_archive_preview(&entry.path, type_detail)
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
        Ok(None) => {
            if let Some(preview) = binary::build_binary_preview(&entry.path, type_detail) {
                return preview;
            }
            return apply_type_detail(binary_preview(), type_detail);
        }
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
            let line_truncated = total_items > PREVIEW_RENDER_LINE_LIMIT;
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

            let mut preview = PreviewContent::new(PreviewKind::Directory, lines)
                .with_detail(format!("{total_items} items"))
                .with_directory_counts(total_items, folder_count, file_count);
            if line_truncated {
                preview = preview
                    .with_truncation(format!("showing first {PREVIEW_RENDER_LINE_LIMIT} items"));
            }
            preview
        }
        Err(error) => unavailable_directory_preview(&error),
    }
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
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

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

    fn span_color<'a>(line: &'a Line<'a>, token: &str) -> Option<Color> {
        line.spans
            .iter()
            .find(|span| span.content.contains(token))
            .and_then(|span| span.style.fg)
    }

    fn line_has_color(line: &Line<'_>, color: Color) -> bool {
        line.spans.iter().any(|span| span.style.fg == Some(color))
    }

    fn bencode_bytes(value: &[u8]) -> Vec<u8> {
        let mut encoded = format!("{}:", value.len()).into_bytes();
        encoded.extend_from_slice(value);
        encoded
    }

    fn bencode_str(value: &str) -> Vec<u8> {
        bencode_bytes(value.as_bytes())
    }

    fn bencode_int(value: i64) -> Vec<u8> {
        format!("i{value}e").into_bytes()
    }

    fn bencode_list(values: Vec<Vec<u8>>) -> Vec<u8> {
        let mut encoded = vec![b'l'];
        for value in values {
            encoded.extend(value);
        }
        encoded.push(b'e');
        encoded
    }

    fn bencode_dict(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
        let mut encoded = vec![b'd'];
        for (key, value) in entries {
            encoded.extend(bencode_str(key));
            encoded.extend(value);
        }
        encoded.push(b'e');
        encoded
    }

    fn write_iso_field(bytes: &mut [u8], start: usize, end: usize, value: &str) {
        let field = &mut bytes[start..end];
        field.fill(b' ');
        let copy_len = value.len().min(field.len());
        field[..copy_len].copy_from_slice(&value.as_bytes()[..copy_len]);
    }

    fn put_iso_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_iso_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn sample_iso_descriptors() -> Vec<u8> {
        let mut bytes = vec![0u8; (ISO_DESCRIPTOR_START_SECTOR + 3) * ISO_SECTOR_SIZE];
        let start = ISO_DESCRIPTOR_START_SECTOR * ISO_SECTOR_SIZE;

        let boot = &mut bytes[start..start + ISO_SECTOR_SIZE];
        boot[0] = 0;
        boot[1..6].copy_from_slice(b"CD001");
        boot[6] = 1;
        write_iso_field(boot, 7, 39, ISO_BOOT_SYSTEM_ID);

        let primary = &mut bytes[start + ISO_SECTOR_SIZE..start + ISO_SECTOR_SIZE * 2];
        primary[0] = 1;
        primary[1..6].copy_from_slice(b"CD001");
        primary[6] = 1;
        write_iso_field(primary, 8, 40, "ELIO_SYS");
        write_iso_field(primary, 40, 72, "ELIO_INSTALL");
        put_iso_u32_le(primary, 80, 640);
        put_iso_u16_le(primary, 128, ISO_SECTOR_SIZE as u16);
        write_iso_field(primary, 318, 446, "Elio Publisher");
        write_iso_field(primary, 446, 574, "Elio Builder");
        write_iso_field(primary, 574, 702, "Elio Image Tool");
        write_iso_field(primary, 813, 830, "20260311090000000");
        write_iso_field(primary, 830, 847, "20260311101500000");
        write_iso_field(primary, 864, 881, "20260312000000000");

        let terminator = &mut bytes[start + ISO_SECTOR_SIZE * 2..start + ISO_SECTOR_SIZE * 3];
        terminator[0] = 255;
        terminator[1..6].copy_from_slice(b"CD001");
        terminator[6] = 1;
        bytes
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

    fn write_tar_gz_entries(path: &Path, entries: &[(&str, &str)]) -> bool {
        let root = temp_path("tar-gz-root");
        fs::create_dir_all(&root).expect("failed to create tar staging root");

        for (name, contents) in entries {
            let entry_path = root.join(name);
            if let Some(parent) = entry_path.parent() {
                fs::create_dir_all(parent).expect("failed to create tar staging directory");
            }
            fs::write(&entry_path, contents).expect("failed to write tar staging file");
        }

        let created = Command::new("tar")
            .arg("-czf")
            .arg(path)
            .arg("-C")
            .arg(&root)
            .arg(".")
            .status();

        fs::remove_dir_all(root).expect("failed to remove tar staging root");
        created.as_ref().is_ok_and(|status| status.success())
    }

    fn write_xz_compressed_file(path: &Path, contents: &[u8]) -> bool {
        let source = path.with_extension("");
        fs::write(&source, contents).expect("failed to write xz staging file");

        let created = Command::new("xz").arg("-zk").arg(&source).status();
        let _ = fs::remove_file(&source);
        created.as_ref().is_ok_and(|status| status.success()) && path.exists()
    }

    fn put_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32_be(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn put_u64_le(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn sample_pe_exe_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x200];
        bytes[0..2].copy_from_slice(b"MZ");
        put_u32_le(&mut bytes, 0x3c, 0x80);

        let pe = 0x80;
        bytes[pe..pe + 4].copy_from_slice(b"PE\0\0");
        put_u16_le(&mut bytes, pe + 4, 0x8664);
        put_u16_le(&mut bytes, pe + 6, 3);
        put_u16_le(&mut bytes, pe + 20, 0x00f0);
        put_u16_le(&mut bytes, pe + 22, 0x0022);

        let optional = pe + 24;
        put_u16_le(&mut bytes, optional, 0x20b);
        put_u32_le(&mut bytes, optional + 16, 0x1230);
        put_u16_le(&mut bytes, optional + 88, 3);
        bytes
    }

    fn sample_elf_shared_object_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 64];
        bytes[0..4].copy_from_slice(b"\x7FELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[6] = 1;
        bytes[7] = 3;
        put_u16_le(&mut bytes, 16, 3);
        put_u16_le(&mut bytes, 18, 0x00b7);
        put_u64_le(&mut bytes, 24, 0x401000);
        put_u16_le(&mut bytes, 56, 8);
        put_u16_le(&mut bytes, 60, 18);
        bytes
    }

    fn sample_macho_dylib_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 32];
        bytes[0..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        put_u32_le(&mut bytes, 4, 0x0100000c);
        put_u32_le(&mut bytes, 12, 6);
        put_u32_le(&mut bytes, 16, 12);
        bytes
    }

    fn sample_dos_mz_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 64];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes
    }

    fn sample_macho_fat_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 48];
        bytes[0..4].copy_from_slice(&[0xca, 0xfe, 0xba, 0xbe]);
        put_u32_be(&mut bytes, 4, 2);
        put_u32_be(&mut bytes, 8, 7);
        put_u32_be(&mut bytes, 28, 0x0100000c);
        bytes
    }

    fn sample_pdf_bytes() -> Vec<u8> {
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Resources << >> /Contents 4 0 R >>"
                .to_string(),
            "<< /Length 0 >>\nstream\n\nendstream".to_string(),
            "<< /Title (Quarterly Report) /Author (Regueiro) /Creator (Elio) /Producer (Elio Test Suite) /CreationDate (D:20260311120000Z) /ModDate (D:20260311123000Z) >>".to_string(),
        ];

        let mut bytes = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            offsets.push(bytes.len());
            bytes.extend(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
        }

        let xref_offset = bytes.len();
        bytes.extend(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        bytes.extend(b"0000000000 65535 f \n");
        for offset in offsets {
            bytes.extend(format!("{offset:010} 00000 n \n").as_bytes());
        }
        bytes.extend(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{}\n%%EOF\n",
                objects.len() + 1,
                xref_offset
            )
            .as_bytes(),
        );
        bytes
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
        assert!(preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "inline")));

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
        assert!(preview
            .lines
            .iter()
            .any(|line| line_text(line).contains("fn main() {}")));

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
        assert!(preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "1. ")));
        assert!(preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content.contains("• "))));
        assert!(preview
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content == "2. ")));

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
    fn c_preview_uses_code_renderer() {
        let root = temp_path("c");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("main.c");
        fs::write(
            &path,
            "#include <stdio.h>\nint main(void) {\n    printf(\"hello\\n\");\n}\n",
        )
        .expect("failed to write c source");

        let preview = build_preview(&file_entry(path));
        let code_palette = appearance::code_preview_palette();

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some_and(|detail| detail.contains('C')));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("printf")));
        assert_eq!(
            span_color(&preview.lines[0], "#"),
            Some(code_palette.r#macro)
        );
        assert_eq!(
            span_color(&preview.lines[1], "int"),
            Some(code_palette.r#type)
        );
        assert_ne!(
            span_color(&preview.lines[2], "printf"),
            Some(code_palette.fg)
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn python_preview_uses_code_renderer_with_colors() {
        let root = temp_path("python");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("main.py");
        fs::write(
            &path,
            "@decorator\nclass Greeter:\n    async def greet(self, name: str) -> str:\n        \"\"\"Return greeting.\"\"\"\n        return f\"hi {name}\"\n",
        )
        .expect("failed to write python source");

        let preview = build_preview(&file_entry(path));
        let code_palette = appearance::code_preview_palette();

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some());
        assert_eq!(
            span_color(&preview.lines[0], "@"),
            Some(code_palette.r#macro)
        );
        assert_eq!(
            span_color(&preview.lines[1], "class"),
            Some(code_palette.keyword)
        );
        assert_eq!(
            span_color(&preview.lines[1], "Greeter"),
            Some(code_palette.r#type)
        );
        assert_eq!(
            span_color(&preview.lines[2], "async"),
            Some(code_palette.keyword)
        );
        assert_eq!(
            span_color(&preview.lines[2], "greet"),
            Some(code_palette.function)
        );
        assert_ne!(
            span_color(&preview.lines[4], "return"),
            Some(code_palette.fg)
        );
        assert_ne!(
            span_color(&preview.lines[4], "f\"hi {name}\""),
            Some(code_palette.fg)
        );
        assert!(line_has_color(&preview.lines[3], code_palette.string));
        assert!(line_has_color(&preview.lines[4], code_palette.string));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn javascript_preview_uses_code_renderer_with_colors() {
        let root = temp_path("javascript");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("main.js");
        fs::write(
            &path,
            "export class Greeter {\n  greet(name) { return console.log(`hi ${name}`); }\n}\n",
        )
        .expect("failed to write javascript source");

        let preview = build_preview(&file_entry(path));
        let code_palette = appearance::code_preview_palette();

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview
            .detail
            .is_some_and(|detail| detail.contains("TypeScript")));
        assert_eq!(
            span_color(&preview.lines[0], "export"),
            Some(code_palette.keyword)
        );
        assert_eq!(
            span_color(&preview.lines[0], "Greeter"),
            Some(code_palette.r#type)
        );
        assert_eq!(
            span_color(&preview.lines[1], "return"),
            Some(code_palette.keyword)
        );
        assert!(line_has_color(&preview.lines[1], code_palette.string));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn nix_preview_uses_code_renderer() {
        let root = temp_path("nix");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("flake.nix");
        fs::write(
            &path,
            "{ description = \"elio\"; outputs = { self }: { packages.x86_64-linux.default = self; }; }\n",
        )
        .expect("failed to write nix source");

        let preview = build_preview(&file_entry(path));
        let code_palette = appearance::code_preview_palette();

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some_and(|detail| detail.contains("Nix")));
        assert_eq!(
            span_color(&preview.lines[0], "description"),
            Some(code_palette.parameter)
        );
        assert!(line_has_color(&preview.lines[0], code_palette.string));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("description")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn cmake_preview_uses_code_renderer() {
        let root = temp_path("cmake");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("CMakeLists.txt");
        fs::write(
            &path,
            "cmake_minimum_required(VERSION 3.28)\nproject(elio)\nadd_executable(elio main.cpp)\n",
        )
        .expect("failed to write cmake source");

        let preview = build_preview(&file_entry(path));
        let code_palette = appearance::code_preview_palette();

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview
            .detail
            .is_some_and(|detail| detail.contains("CMake")));
        assert_ne!(
            span_color(&preview.lines[2], "add_executable"),
            Some(code_palette.fg)
        );
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("add_executable")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn generic_lockfile_uses_code_renderer() {
        let root = temp_path("lock");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("deps.lock");
        fs::write(&path, "[packages]\nelio=1.0.0\n").expect("failed to write lockfile");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some_and(|detail| detail.contains("INI")));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("elio")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn makefile_preview_uses_code_renderer() {
        let root = temp_path("makefile");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("Makefile");
        fs::write(
            &path,
            "CC := clang\n.PHONY: build\nbuild: main.o util.o\n\t$(CC) -o app main.o util.o\n",
        )
        .expect("failed to write makefile");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Make")));
        assert!(line_texts.iter().any(|text| text.contains(".PHONY")));
        assert!(line_texts.iter().any(|text| text.contains("$(CC)")));

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
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("div")));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("class")));

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
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("color")));

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
        assert!(line_texts
            .iter()
            .all(|text| !text.contains("Format") || !text.contains("DOCX document")));
        assert!(line_texts.iter().any(|text| text == "People"));
        assert!(line_texts.iter().any(|text| text == "Dates"));
        assert!(line_texts.iter().any(|text| text == "Stats"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Quarterly Report")));
        assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
        assert!(line_texts.iter().any(|text| text.contains("4,238")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("Mar 11, 2026 09:00 UTC")));
        assert!(line_texts
            .iter()
            .all(|text| !text.contains("2026-03-11T09:00:00Z")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("LibreOffice")));
        assert!(line_texts
            .iter()
            .all(|text| !text.contains("ApplicationLibreOffice")));
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
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("Mar 10, 2026 18:00 UTC")));
        assert!(line_texts
            .iter()
            .all(|text| !text.contains("2026-03-10T18:00:00Z")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn pptx_preview_shows_presentation_metadata() {
        let root = temp_path("pptx");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("deck.pptx");
        write_zip_entries(
            &path,
            &[
                (
                    "docProps/core.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/"
                        xmlns:dcterms="http://purl.org/dc/terms/">
                      <dc:title>Launch Deck</dc:title>
                      <dc:creator>Elio</dc:creator>
                      <dcterms:modified>2026-03-12T09:30:00Z</dcterms:modified>
                    </cp:coreProperties>"#,
                ),
                (
                    "docProps/app.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>PowerPoint</Application>
                      <Slides>24</Slides>
                      <Notes>6</Notes>
                      <HiddenSlides>2</HiddenSlides>
                    </Properties>"#,
                ),
            ],
        );

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some("PPTX presentation"));
        assert!(line_texts.iter().any(|text| text.contains("Launch Deck")));
        assert!(line_texts.iter().any(|text| text.contains("PowerPoint")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Slides") && text.contains("24")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Notes") && text.contains("6")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Hidden Slides") && text.contains("2")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn xlsx_preview_shows_spreadsheet_metadata() {
        let root = temp_path("xlsx");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("budget.xlsx");
        write_zip_entries(
            &path,
            &[
                (
                    "docProps/core.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/">
                      <dc:title>Q2 Budget</dc:title>
                      <dc:creator>Finance Team</dc:creator>
                    </cp:coreProperties>"#,
                ),
                (
                    "docProps/app.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>Excel</Application>
                      <Company>Elio Labs</Company>
                      <Manager>Regueiro</Manager>
                    </Properties>"#,
                ),
            ],
        );

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some("XLSX spreadsheet"));
        assert!(line_texts.iter().any(|text| text.contains("Q2 Budget")));
        assert!(line_texts.iter().any(|text| text.contains("Finance Team")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("Excel")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Company") && text.contains("Elio Labs")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Manager") && text.contains("Regueiro")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn ods_preview_shows_spreadsheet_statistics() {
        let root = temp_path("ods");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("budget.ods");
        write_zip_entries(
            &path,
            &[(
                "meta.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                    xmlns:dc="http://purl.org/dc/elements/1.1/"
                    xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
                  <office:meta>
                    <dc:title>Operations Budget</dc:title>
                    <meta:initial-creator>Elio</meta:initial-creator>
                    <meta:generator>LibreOffice Calc</meta:generator>
                    <meta:document-statistic meta:table-count="4" meta:cell-count="512" meta:object-count="2"/>
                  </office:meta>
                </office:document-meta>"#,
            )],
        );

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some("ODS spreadsheet"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Operations Budget")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("LibreOffice Calc")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Tables") && text.contains("4")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Cells") && text.contains("512")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Objects") && text.contains("2")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn epub_preview_shows_package_metadata() {
        let root = temp_path("epub");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("novel.epub");
        write_zip_entries(
            &path,
            &[
                (
                    "META-INF/container.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                      <rootfiles>
                        <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                      </rootfiles>
                    </container>"#,
                ),
                (
                    "OPS/package.opf",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                      <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                        <dc:title>Elio Handbook</dc:title>
                        <dc:creator>Regueiro</dc:creator>
                        <dc:language>en</dc:language>
                        <dc:publisher>Elio Docs</dc:publisher>
                        <dc:identifier>urn:uuid:elio-handbook</dc:identifier>
                        <dc:date>2026-03-12T08:00:00Z</dc:date>
                      </metadata>
                    </package>"#,
                ),
            ],
        );

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some("EPUB ebook"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Variant") && text.contains("EPUB package")));
        assert!(line_texts.iter().any(|text| text.contains("Elio Handbook")));
        assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Language") && text.contains("en")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Publisher") && text.contains("Elio Docs")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Identifier") && text.contains("urn:uuid:elio-handbook")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn pdf_preview_shows_pdfinfo_metadata() {
        if Command::new("pdfinfo").arg("-v").output().is_err() {
            return;
        }

        let root = temp_path("pdf");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("report.pdf");
        fs::write(&path, sample_pdf_bytes()).expect("failed to write pdf fixture");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some("PDF document"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Variant") && text.contains("PDF 1.4")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Quarterly Report")));
        assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("Elio")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Pages") && text.contains("1")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Producer") && text.contains("Elio Test Suite")));

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
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("layout")));

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
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("server")));

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
        assert!(preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail == "Desktop Entry (best-effort)"));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("エリオ")));

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
    fn shell_script_preview_uses_code_renderer() {
        let root = temp_path("shell");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("deploy.sh");
        fs::write(
            &path,
            "#!/usr/bin/env bash\nNAME=elio\nif [ -n \"$NAME\" ]; then\n  printf '%s\\n' \"$(whoami)\"\nfi\n",
        )
        .expect("failed to write shell script");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
        let code_palette = appearance::code_preview_palette();

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Shell")));
        assert!(line_texts.iter().any(|text| text.contains("printf")));
        assert!(line_texts.iter().any(|text| text.contains("$(whoami)")));
        assert_eq!(
            span_color(&preview.lines[2], "if"),
            Some(code_palette.keyword)
        );
        assert_ne!(
            span_color(&preview.lines[3], "printf"),
            Some(code_palette.fg)
        );
        assert_ne!(
            span_color(&preview.lines[3], "$(whoami)"),
            Some(code_palette.fg)
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn shell_dotfile_preview_uses_code_renderer() {
        let root = temp_path("shell-dotfile");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join(".bashrc");
        fs::write(
            &path,
            "export PATH=\"$HOME/bin:$PATH\"\nalias ll='ls -la'\n",
        )
        .expect("failed to write shell dotfile");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
        let code_palette = appearance::code_preview_palette();

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Shell")));
        assert!(line_texts.iter().any(|text| text.contains("export")));
        assert!(line_texts.iter().any(|text| text.contains("alias")));
        assert_ne!(
            span_color(&preview.lines[0], "export"),
            Some(code_palette.fg)
        );
        assert_ne!(
            span_color(&preview.lines[1], "alias"),
            Some(code_palette.fg)
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn zsh_preview_uses_shell_specific_support() {
        let root = temp_path("zsh");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("prompt.zsh");
        fs::write(
            &path,
            "autoload -U colors && colors\nprompt_elio() {\n  print -P '%F{blue}%~%f'\n}\n",
        )
        .expect("failed to write zsh script");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.detail.is_some());
        assert!(line_texts.iter().any(|text| text.contains("autoload")));
        assert!(line_texts.iter().any(|text| text.contains("prompt_elio")));

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
        assert!(preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("INI")));

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
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("ERROR")));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("request_id")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn multiline_log_preview_keeps_stack_trace_context() {
        let root = temp_path("log-multiline");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("server.log");
        fs::write(
            &path,
            "2026-03-10T12:00:00Z ERROR request_id=42 msg=\"request failed\"\n\
             \tat service.handle (/srv/app.js:10)\n\
             Caused by: timeout\n\
             2026-03-10T12:00:01Z INFO request_id=42 recovered\n",
        )
        .expect("failed to write log");

        let preview = build_preview(&file_entry(path));
        let rendered = preview
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(preview.kind, PreviewKind::Code);
        assert_eq!(preview.detail.as_deref(), Some("Log (structured)"));
        assert!(rendered.contains("request failed"));
        assert!(rendered.contains("Caused by: timeout"));
        assert!(rendered.contains("recovered"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn unstructured_log_preview_falls_back_to_best_effort_renderer() {
        let root = temp_path("log-fallback");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("notes.log");
        fs::write(
            &path,
            "starting application\nloading configuration\nready\n",
        )
        .expect("failed to write log");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert_eq!(preview.detail.as_deref(), Some("Log (best-effort)"));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("starting application")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn torrent_preview_shows_single_file_metadata_and_trackers() {
        let root = temp_path("torrent");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("sample.torrent");
        let bytes = bencode_dict(vec![
            ("announce", bencode_str("https://tracker.test")),
            (
                "announce-list",
                bencode_list(vec![bencode_list(vec![
                    bencode_str("https://tracker.test"),
                    bencode_str("https://backup.test"),
                ])]),
            ),
            ("comment", bencode_str("test torrent")),
            ("created by", bencode_str("elio")),
            (
                "info",
                bencode_dict(vec![
                    ("length", bencode_int(12_345)),
                    ("name", bencode_str("file.txt")),
                    ("piece length", bencode_int(262_144)),
                    ("pieces", bencode_bytes(b"12345678901234567890")),
                    ("private", bencode_int(1)),
                ]),
            ),
        ]);
        fs::write(&path, bytes).expect("failed to write torrent");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Text);
        assert_eq!(preview.detail.as_deref(), Some("BitTorrent file"));
        assert_eq!(line_texts.first().map(String::as_str), Some("Torrent"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Name") && text.contains("file.txt")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Mode") && text.contains("Single-file")));
        assert!(line_texts.iter().any(|text| text.contains("Private")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Trackers") && text.contains("2 across 1 tier")));
        assert!(line_texts.iter().any(|text| text == "Trackers"));
        assert!(line_texts.iter().any(|text| {
            text.contains("Tier 1") && text.contains("tracker.test") && text.contains("backup.test")
        }));
        assert!(line_texts.iter().any(|text| text == "Contents"));
        assert!(line_texts.iter().any(|text| text.contains("file.txt")));
        assert!(!preview.truncated);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn torrent_preview_shows_multifile_contents_tree() {
        let root = temp_path("torrent-multifile");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("series.torrent");
        let bytes = bencode_dict(vec![
            (
                "announce-list",
                bencode_list(vec![
                    bencode_list(vec![
                        bencode_str("https://tracker.one"),
                        bencode_str("https://tracker.two"),
                    ]),
                    bencode_list(vec![bencode_str("https://backup.tld/announce")]),
                ]),
            ),
            ("created by", bencode_str("elio")),
            (
                "info",
                bencode_dict(vec![
                    (
                        "files",
                        bencode_list(vec![
                            bencode_dict(vec![
                                ("length", bencode_int(100)),
                                (
                                    "path",
                                    bencode_list(vec![
                                        bencode_str("season-01"),
                                        bencode_str("ep1.mkv"),
                                    ]),
                                ),
                            ]),
                            bencode_dict(vec![
                                ("length", bencode_int(200)),
                                (
                                    "path.utf-8",
                                    bencode_list(vec![
                                        bencode_str("season-01"),
                                        bencode_str("ep2.mkv"),
                                    ]),
                                ),
                            ]),
                        ]),
                    ),
                    ("name", bencode_str("series")),
                    ("piece length", bencode_int(65_536)),
                    (
                        "pieces",
                        bencode_bytes(b"1234567890123456789012345678901234567890"),
                    ),
                    ("private", bencode_int(0)),
                ]),
            ),
        ]);
        fs::write(&path, bytes).expect("failed to write torrent");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Text);
        assert_eq!(preview.detail.as_deref(), Some("BitTorrent file"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Mode") && text.contains("Multi-file")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Files") && text.contains("2")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Trackers") && text.contains("3 across 2 tiers")));
        assert!(line_texts.iter().any(|text| text.contains("Tier 2")));
        assert!(line_texts.iter().any(|text| text.contains("series/")));
        assert!(line_texts.iter().any(|text| text.contains("season-01/")));
        assert!(line_texts.iter().any(|text| text.contains("ep1.mkv")));
        assert!(line_texts.iter().any(|text| text.contains("ep2.mkv")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Privacy") && text.contains("Public")));
        assert!(!preview.truncated);

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
        fs::write(&path, [0x00, 0x81, 0xFE, 0xFF]).expect("failed to write iso");

        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Binary);
        assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn iso_metadata_parser_reads_primary_volume_descriptor() {
        let metadata = container::parse_iso_metadata(&sample_iso_descriptors())
            .expect("sample descriptors should parse");

        assert_eq!(metadata.system_id.as_deref(), Some("ELIO_SYS"));
        assert_eq!(metadata.volume_id.as_deref(), Some("ELIO_INSTALL"));
        assert_eq!(metadata.publisher_id.as_deref(), Some("Elio Publisher"));
        assert_eq!(metadata.preparer_id.as_deref(), Some("Elio Builder"));
        assert_eq!(metadata.application_id.as_deref(), Some("Elio Image Tool"));
        assert_eq!(metadata.created_at.as_deref(), Some("2026-03-11 09:00:00"));
        assert_eq!(metadata.modified_at.as_deref(), Some("2026-03-11 10:15:00"));
        assert_eq!(
            metadata.effective_at.as_deref(),
            Some("2026-03-12 00:00:00")
        );
        assert_eq!(metadata.total_size, Some(640 * ISO_SECTOR_SIZE as u64));
        assert!(metadata.bootable);
    }

    #[test]
    fn iso_entry_normalization_reconstructs_parents_and_strips_versions() {
        let entries = container::normalize_archive_entries(
            ["/docs/readme.txt;1", "./EFI/BOOT/", "boot.catalog;1"],
            true,
        );

        assert!(entries
            .iter()
            .any(|entry| entry.path == "docs" && entry.is_dir));
        assert!(entries
            .iter()
            .any(|entry| entry.path == "docs/readme.txt" && !entry.is_dir));
        assert!(entries
            .iter()
            .any(|entry| entry.path == "EFI" && entry.is_dir));
        assert!(entries
            .iter()
            .any(|entry| entry.path == "EFI/BOOT" && entry.is_dir));
        assert!(entries
            .iter()
            .any(|entry| entry.path == "boot.catalog" && !entry.is_dir));
    }

    #[test]
    fn iso_preview_renders_metadata_and_tree() {
        let preview = container::render_iso_preview(
            IsoMetadata {
                volume_id: Some("ELIO_INSTALL".to_string()),
                system_id: Some("ELIO_SYS".to_string()),
                total_size: Some(640 * ISO_SECTOR_SIZE as u64),
                bootable: true,
                created_at: Some("2026-03-11 09:00:00".to_string()),
                ..IsoMetadata::default()
            },
            container::normalize_archive_entries(
                ["boot/", "boot/grub/", "boot/grub/grub.cfg", "README.txt"],
                true,
            ),
        );
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
        let header = preview
            .header_detail(0, 20)
            .expect("iso preview should expose header detail");

        assert_eq!(preview.kind, PreviewKind::Archive);
        assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));
        assert!(header.contains("ISO disk image"));
        assert_eq!(line_texts.first().map(String::as_str), Some("Image"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("ELIO_INSTALL")));
        assert!(line_texts
            .iter()
            .any(|text| text == "Contents" || text.ends_with("Contents")));
        assert!(line_texts.iter().any(|text| text.contains("boot/")));
        assert!(line_texts.iter().any(|text| text.contains("grub.cfg")));
        assert!(line_texts.iter().any(|text| text.contains("README.txt")));
    }

    #[test]
    fn iso_preview_reports_tree_truncation() {
        let items = (0..320)
            .map(|index| format!("dir/file-{index:03}.txt"))
            .collect::<Vec<_>>();
        let preview = container::render_iso_preview(
            IsoMetadata {
                volume_id: Some("BIG_IMAGE".to_string()),
                ..IsoMetadata::default()
            },
            container::normalize_archive_entries(items.iter().map(String::as_str), true),
        );
        let header = preview
            .header_detail(0, 20)
            .expect("iso preview header should include truncation");

        assert!(preview.truncated);
        assert!(header.contains("showing first"));
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

        assert_eq!(preview.kind, PreviewKind::Archive);
        assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("docs/")));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("readme.txt")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn zip_preview_renders_archive_summary_and_tree() {
        let root = temp_path("zip-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("bundle.zip");
        write_zip_entries(
            &path,
            &[
                ("docs/readme.txt", "hello"),
                ("src/main.rs", "fn main() {}\n"),
            ],
        );

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
        let header = preview
            .header_detail(0, 20)
            .expect("zip preview should expose header detail");

        assert_eq!(preview.kind, PreviewKind::Archive);
        assert_eq!(preview.detail.as_deref(), Some("ZIP archive"));
        assert!(header.contains("ZIP archive"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Entries") && text.contains("4 total")));
        assert!(line_texts.iter().any(|text| text.contains("docs/")));
        assert!(line_texts.iter().any(|text| text.contains("src/")));
        assert!(line_texts.iter().any(|text| text.contains("readme.txt")));
        assert!(line_texts.iter().any(|text| text.contains("main.rs")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn tar_gz_preview_lists_inner_archive_contents() {
        let root = temp_path("tar-gz-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("bundle.tar.gz");
        if !write_tar_gz_entries(
            &path,
            &[
                ("docs/readme.txt", "hello"),
                ("src/main.rs", "fn main() {}\n"),
            ],
        ) {
            fs::remove_dir_all(root).expect("failed to remove temp root");
            return;
        }

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Archive);
        assert_eq!(preview.detail.as_deref(), Some("TAR.GZ archive"));
        assert!(line_texts.iter().any(|text| text.contains("docs/")));
        assert!(line_texts.iter().any(|text| text.contains("src/")));
        assert!(line_texts.iter().any(|text| text.contains("readme.txt")));
        assert!(line_texts.iter().any(|text| text.contains("main.rs")));
        assert!(!line_texts.iter().any(|text| text.contains("bundle.tar")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn tgz_preview_keeps_tar_gz_label_and_contents_tree() {
        let root = temp_path("tgz-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("bundle.tgz");
        if !write_tar_gz_entries(
            &path,
            &[("assets/logo.txt", "logo"), ("bin/elio", "#!/bin/sh\n")],
        ) {
            fs::remove_dir_all(root).expect("failed to remove temp root");
            return;
        }

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Archive);
        assert_eq!(preview.detail.as_deref(), Some("TAR.GZ archive"));
        assert!(line_texts.iter().any(|text| text.contains("assets/")));
        assert!(line_texts.iter().any(|text| text.contains("bin/")));
        assert!(line_texts.iter().any(|text| text.contains("logo.txt")));
        assert!(line_texts.iter().any(|text| text.contains("elio")));
        assert!(!line_texts.iter().any(|text| text.contains("bundle.tar")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn raw_xz_preview_uses_compressed_disk_image_label() {
        let root = temp_path("raw-xz-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("fedora.aarch64.raw.xz");
        if !write_xz_compressed_file(&path, b"raw-disk-image") {
            fs::remove_dir_all(root).expect("failed to remove temp root");
            return;
        }

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Archive);
        assert_eq!(
            preview.detail.as_deref(),
            Some("XZ-compressed raw disk image")
        );
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Format") && (text.contains("XZ") || text.contains("xz"))));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("fedora.aarch64.raw")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn pe_preview_shows_windows_executable_metadata() {
        let root = temp_path("pe-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("setup.exe");
        fs::write(&path, sample_pe_exe_bytes()).expect("failed to write pe fixture");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Binary);
        assert_eq!(preview.detail.as_deref(), Some("Windows executable"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("PE/COFF")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Architecture") && text.contains("x86_64")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Bits") && text.contains("64-bit")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Subsystem") && text.contains("Console")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Entry Point") && text.contains("0x1230")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn elf_preview_detects_binaries_without_extension() {
        let root = temp_path("elf-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("app-bin");
        fs::write(&path, sample_elf_shared_object_bytes()).expect("failed to write elf fixture");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Binary);
        assert_eq!(preview.detail.as_deref(), Some("ELF shared object"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Architecture") && text.contains("AArch64")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("ABI") && text.contains("Linux")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Entry Point") && text.contains("0x401000")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Sections") && text.contains("18")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn macho_preview_shows_dynamic_library_metadata() {
        let root = temp_path("macho-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("libelio.dylib");
        fs::write(&path, sample_macho_dylib_bytes()).expect("failed to write macho fixture");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Binary);
        assert_eq!(preview.detail.as_deref(), Some("Dynamic library"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("Mach-O")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Kind") && text.contains("Dynamic library")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Architecture") && text.contains("ARM64")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Load Commands") && text.contains("12")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn dos_mz_preview_falls_back_to_legacy_executable_metadata() {
        let root = temp_path("dos-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("legacy.bin");
        fs::write(&path, sample_dos_mz_bytes()).expect("failed to write dos fixture");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Binary);
        assert_eq!(preview.detail.as_deref(), Some("DOS executable"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("MZ")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Bits") && text.contains("16-bit")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn fat_macho_preview_lists_architectures_for_universal_binaries() {
        let root = temp_path("fat-macho-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("elio-universal");
        fs::write(&path, sample_macho_fat_bytes()).expect("failed to write fat macho fixture");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Binary);
        assert_eq!(preview.detail.as_deref(), Some("Mach-O universal binary"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("Mach-O (fat)")));
        assert!(line_texts.iter().any(|text| {
            text.contains("Architecture") && text.contains("x86") && text.contains("ARM64")
        }));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Sections") && text.contains("2")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn jar_preview_surfaces_manifest_metadata() {
        let root = temp_path("jar-preview");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("app.jar");
        write_zip_entries(
            &path,
            &[
                (
                    "META-INF/MANIFEST.MF",
                    "Implementation-Title: Elio\nImplementation-Version: 1.2.3\nMain-Class: elio.Main\nCreated-By: OpenJDK\n",
                ),
                ("elio/Main.class", "compiled"),
            ],
        );

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Archive);
        assert_eq!(preview.detail.as_deref(), Some("Java archive"));
        assert!(line_texts.iter().any(|text| text == "Manifest"));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Title") && text.contains("Elio")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Version") && text.contains("1.2.3")));
        assert!(line_texts
            .iter()
            .any(|text| text.contains("Main-Class") && text.contains("elio.Main")));

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
        assert!(preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TOML")));

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
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("nested")));
        assert!(preview.lines.len() > 1);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn json_preview_adds_root_summary_and_array_indexes() {
        let root = temp_path("json-summary");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("data.json");
        fs::write(&path, "{\"items\":[{\"id\":1},{\"id\":2}],\"ok\":true}\n")
            .expect("failed to write json");

        let preview = build_preview(&file_entry(path));
        let rendered = preview
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("root: object"));
        assert!(rendered.contains("2 keys"));
        assert!(rendered.contains("[0]: {id: 1}"));
        assert!(rendered.contains("[1]: {id: 2}"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn json_preview_inlines_small_scalar_structures() {
        let root = temp_path("json-inline");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("data.json");
        fs::write(
            &path,
            "{\"meta\":{\"id\":1,\"env\":\"dev\"},\"ports\":[80,443]}\n",
        )
        .expect("failed to write json");

        let preview = build_preview(&file_entry(path));
        let rendered = preview
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("meta: {env: \"dev\", id: 1}"));
        assert!(rendered.contains("ports: [80, 443]"));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn json_preview_truncates_long_strings_with_length_hint() {
        let root = temp_path("json-long-string");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("data.json");
        fs::write(&path, format!("{{\"token\":\"{}\"}}\n", "a".repeat(120)))
            .expect("failed to write json");

        let preview = build_preview(&file_entry(path));
        let rendered = preview
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("token: "));
        assert!(rendered.contains("(120 chars)"));
        assert!(rendered.contains("…"));

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
        assert!(preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail == ".env (structured)"));
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("APP_ENV")));

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
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("name")));

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
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("trailing")));

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
        assert!(preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("services")));

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
    fn code_preview_sanitizes_control_characters() {
        let root = temp_path("control-char-code");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("main.c");
        let contents = "int main(void) {\n    puts(\"hello \u{1b} world\");\n    return 0;\n}\n";
        fs::write(&path, contents).expect("failed to write control-char source");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
        assert!(
            line_texts.iter().any(|line| line.contains("^[ world")),
            "expected control characters to be rendered safely, got: {line_texts:?}"
        );

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
    fn utf16le_bom_text_file_is_not_mislabeled_as_binary() {
        let root = temp_path("utf16le-text-kind");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("unicode.txt");
        let text = "Thu Jan 15 21:36:25 2026\r\nHello from UTF-16\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        fs::write(&path, bytes).expect("failed to write utf16 text");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_ne!(preview.kind, PreviewKind::Binary);
        assert!(line_texts
            .iter()
            .any(|line| line.contains("Hello from UTF-16")));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn utf16_log_preview_uses_decoded_text() {
        let root = temp_path("utf16-log");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("socialclub.log");
        let text = "[00000000] Thu Jan 15 21:36:25 2026 INFO launcher started\r\n\
             [00000001] Thu Jan 15 21:36:26 2026 ERROR request_id=42 failed\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        fs::write(&path, bytes).expect("failed to write utf16 log");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_ne!(preview.kind, PreviewKind::Binary);
        assert!(line_texts
            .iter()
            .any(|line| line.contains("launcher started") || line.contains("request_id=42")));
        assert!(preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Log")));

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
        assert!(preview
            .lines
            .iter()
            .any(|line| line_text(line).contains("permission")));

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
        assert!(preview
            .lines
            .iter()
            .any(|line| line_text(line).contains("permission")));

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("failed to unlock file");
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
