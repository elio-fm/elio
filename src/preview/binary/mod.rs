mod elf;
mod macho;
mod pe;

use super::{PreviewContent, PreviewKind};
use crate::preview::appearance as theme;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::{fs::File, io::Read, path::Path};

const BINARY_METADATA_LIMIT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ByteOrder {
    Little,
    Big,
}

impl ByteOrder {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Little => "Little-endian",
            Self::Big => "Big-endian",
        }
    }
}

pub(super) struct BinaryMetadata {
    detail: &'static str,
    format: &'static str,
    kind: Option<String>,
    architecture: Option<String>,
    bits: Option<&'static str>,
    endianness: Option<&'static str>,
    abi: Option<String>,
    subsystem: Option<String>,
    entry_point: Option<String>,
    section_count: Option<usize>,
    command_count: Option<usize>,
}

impl BinaryMetadata {
    fn fields(&self) -> [(&'static str, Option<String>); 9] {
        [
            ("Format", Some(self.format.to_string())),
            ("Kind", self.kind.clone()),
            ("Architecture", self.architecture.clone()),
            ("Bits", self.bits.map(str::to_string)),
            ("Endianness", self.endianness.map(str::to_string)),
            ("ABI", self.abi.clone()),
            ("Subsystem", self.subsystem.clone()),
            ("Entry Point", self.entry_point.clone()),
            (
                "Sections",
                self.section_count.map(|count| count.to_string()),
            ),
        ]
    }

    fn extra_fields(&self) -> [(&'static str, Option<String>); 1] {
        [(
            "Load Commands",
            self.command_count.map(|count| count.to_string()),
        )]
    }
}

pub(super) fn build_binary_preview(
    path: &Path,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    let mut bytes = Vec::with_capacity(BINARY_METADATA_LIMIT_BYTES);
    File::open(path)
        .ok()?
        .take(BINARY_METADATA_LIMIT_BYTES as u64)
        .read_to_end(&mut bytes)
        .ok()?;

    let metadata = parse_binary_metadata(&bytes)?;
    Some(render_binary_preview(
        type_detail.unwrap_or(metadata.detail),
        metadata,
    ))
}

fn parse_binary_metadata(bytes: &[u8]) -> Option<BinaryMetadata> {
    pe::parse(bytes)
        .or_else(|| elf::parse(bytes))
        .or_else(|| macho::parse(bytes))
        .or_else(|| pe::parse_dos_mz(bytes))
}

fn render_binary_preview(detail: &str, metadata: BinaryMetadata) -> PreviewContent {
    let palette = theme::palette();
    let mut lines = Vec::new();
    push_section(&mut lines, "Details", &metadata.fields(), palette);
    push_section(&mut lines, "Metadata", &metadata.extra_fields(), palette);

    PreviewContent::new(PreviewKind::Binary, lines).with_detail(detail)
}

fn push_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(&str, Option<String>)],
    palette: theme::Palette,
) {
    let visible_fields = fields
        .iter()
        .filter_map(|(label, value)| value.as_deref().map(|value| (*label, value)))
        .collect::<Vec<_>>();
    if visible_fields.is_empty() {
        return;
    }

    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line(title, palette));
    let label_width = visible_fields
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(6);
    for (label, value) in visible_fields {
        lines.push(field_line(label, value, label_width, palette));
    }
}

fn section_line(title: &str, palette: theme::Palette) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(palette.accent),
    ))
}

fn field_line(
    label: &str,
    value: &str,
    label_width: usize,
    palette: theme::Palette,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<width$} ", width = label_width + 1),
            Style::default().fg(palette.muted),
        ),
        Span::styled(value.to_string(), Style::default().fg(palette.text)),
    ])
}

pub(super) fn format_hex(value: u64) -> String {
    format!("0x{value:X}")
}

pub(super) fn read_u16(bytes: &[u8], offset: usize, order: ByteOrder) -> Option<u16> {
    let raw = bytes.get(offset..offset + 2)?;
    Some(match order {
        ByteOrder::Little => u16::from_le_bytes(raw.try_into().ok()?),
        ByteOrder::Big => u16::from_be_bytes(raw.try_into().ok()?),
    })
}

pub(super) fn read_u32(bytes: &[u8], offset: usize, order: ByteOrder) -> Option<u32> {
    let raw = bytes.get(offset..offset + 4)?;
    Some(match order {
        ByteOrder::Little => u32::from_le_bytes(raw.try_into().ok()?),
        ByteOrder::Big => u32::from_be_bytes(raw.try_into().ok()?),
    })
}

pub(super) fn read_u64(bytes: &[u8], offset: usize, order: ByteOrder) -> Option<u64> {
    let raw = bytes.get(offset..offset + 8)?;
    Some(match order {
        ByteOrder::Little => u64::from_le_bytes(raw.try_into().ok()?),
        ByteOrder::Big => u64::from_be_bytes(raw.try_into().ok()?),
    })
}

pub(super) fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    read_u16(bytes, offset, ByteOrder::Little)
}

pub(super) fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    read_u32(bytes, offset, ByteOrder::Little)
}
