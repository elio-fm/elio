use super::*;
use crate::preview::appearance as theme;
use ratatui::text::Line;
use std::{fs::File, io::Read, path::Path, process::Command};

pub(in crate::preview) const ISO_METADATA_SCAN_BYTES: u64 = 128 * 1024;
pub(in crate::preview) const ISO_DESCRIPTOR_START_SECTOR: usize = 16;
pub(in crate::preview) const ISO_SECTOR_SIZE: usize = 2048;
pub(in crate::preview) const ISO_BOOT_SYSTEM_ID: &str = "EL TORITO SPECIFICATION";

#[derive(Default)]
pub(in crate::preview) struct IsoMetadata {
    pub(in crate::preview) system_id: Option<String>,
    pub(in crate::preview) volume_id: Option<String>,
    pub(in crate::preview) publisher_id: Option<String>,
    pub(in crate::preview) preparer_id: Option<String>,
    pub(in crate::preview) application_id: Option<String>,
    pub(in crate::preview) created_at: Option<String>,
    pub(in crate::preview) modified_at: Option<String>,
    pub(in crate::preview) effective_at: Option<String>,
    pub(in crate::preview) total_size: Option<u64>,
    pub(in crate::preview) bootable: bool,
}

pub(in crate::preview) fn build_iso_preview(path: &Path) -> Option<PreviewContent> {
    let metadata = read_iso_metadata(path);
    let entries = collect_iso_entries(path);
    if metadata.is_none() && entries.is_none() {
        return None;
    }

    Some(render_iso_preview(
        metadata.unwrap_or_default(),
        entries.unwrap_or_default(),
    ))
}

fn read_iso_metadata(path: &Path) -> Option<IsoMetadata> {
    let mut bytes = Vec::with_capacity(ISO_METADATA_SCAN_BYTES as usize);
    File::open(path)
        .ok()?
        .take(ISO_METADATA_SCAN_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    parse_iso_metadata(&bytes)
}

pub(in crate::preview) fn parse_iso_metadata(bytes: &[u8]) -> Option<IsoMetadata> {
    let mut metadata = IsoMetadata::default();
    let mut found_descriptor = false;
    let start = ISO_DESCRIPTOR_START_SECTOR * ISO_SECTOR_SIZE;
    if bytes.len() < start + ISO_SECTOR_SIZE {
        return None;
    }

    for descriptor in bytes[start..].chunks_exact(ISO_SECTOR_SIZE) {
        if descriptor.get(1..6) != Some(b"CD001".as_slice()) {
            continue;
        }

        found_descriptor = true;
        match descriptor[0] {
            0 => {
                let boot_id = parse_iso_text_field(descriptor, 7, 39);
                if boot_id
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(ISO_BOOT_SYSTEM_ID))
                {
                    metadata.bootable = true;
                }
            }
            1 => {
                metadata.system_id = parse_iso_text_field(descriptor, 8, 40);
                metadata.volume_id = parse_iso_text_field(descriptor, 40, 72);
                metadata.publisher_id = parse_iso_text_field(descriptor, 318, 446);
                metadata.preparer_id = parse_iso_text_field(descriptor, 446, 574);
                metadata.application_id = parse_iso_text_field(descriptor, 574, 702);
                metadata.created_at = parse_iso_datetime_field(descriptor, 813, 830);
                metadata.modified_at = parse_iso_datetime_field(descriptor, 830, 847);
                metadata.effective_at = parse_iso_datetime_field(descriptor, 864, 881);

                let volume_blocks = parse_iso_u32_le(descriptor, 80);
                let block_size = parse_iso_u16_le(descriptor, 128);
                metadata.total_size = volume_blocks
                    .zip(block_size)
                    .map(|(blocks, block_size)| u64::from(blocks) * u64::from(block_size));
            }
            255 => break,
            _ => {}
        }
    }

    found_descriptor.then_some(metadata)
}

fn collect_iso_entries(path: &Path) -> Option<Vec<ArchiveEntry>> {
    collect_iso_entries_with_bsdtar(path).or_else(|| collect_iso_entries_with_isoinfo(path))
}

fn collect_iso_entries_with_bsdtar(path: &Path) -> Option<Vec<ArchiveEntry>> {
    let output = Command::new("bsdtar").arg("-tf").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output.stdout).lines(),
        true,
    ))
}

fn collect_iso_entries_with_isoinfo(path: &Path) -> Option<Vec<ArchiveEntry>> {
    let output = Command::new("isoinfo")
        .arg("-i")
        .arg(path)
        .arg("-f")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output.stdout).lines(),
        true,
    ))
}

pub(in crate::preview) fn render_iso_preview(
    metadata: IsoMetadata,
    entries: Vec<ArchiveEntry>,
) -> PreviewContent {
    let palette = theme::palette();
    let mut lines = Vec::new();
    let total_items = entries.len();
    let folder_count = entries.iter().filter(|entry| entry.is_dir).count();
    let file_count = total_items.saturating_sub(folder_count);

    let summary = vec![
        ("Volume", metadata.volume_id.clone()),
        ("System", metadata.system_id.clone()),
        (
            "Image Size",
            metadata.total_size.map(crate::fs::format_size),
        ),
        (
            "Bootable",
            metadata
                .bootable
                .then(|| "El Torito".to_string())
                .or_else(|| (total_items > 0).then(|| "No".to_string())),
        ),
        (
            "Entries",
            (total_items > 0).then(|| format!("{total_items} total")),
        ),
        (
            "Folders",
            (folder_count > 0).then(|| format!("{folder_count}")),
        ),
        ("Files", (file_count > 0).then(|| format!("{file_count}"))),
        ("Publisher", metadata.publisher_id.clone()),
        ("Prepared By", metadata.preparer_id.clone()),
        ("Application", metadata.application_id.clone()),
        ("Created", metadata.created_at.clone()),
        ("Modified", metadata.modified_at.clone()),
        ("Effective", metadata.effective_at.clone()),
    ];

    push_preview_section(&mut lines, "Image", &summary, palette);

    let mut rendered_items = 0usize;
    let mut tree_truncated = false;
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line("Contents", palette));

    if entries.is_empty() {
        lines.push(Line::from("Unable to read ISO contents"));
    } else {
        let mut root = ArchiveTreeNode::default();
        for entry in &entries {
            insert_archive_tree_entry(&mut root, entry);
        }
        let available_lines = PREVIEW_RENDER_LINE_LIMIT.saturating_sub(lines.len());
        let mut remaining = available_lines;
        if remaining == 0 {
            tree_truncated = true;
        } else {
            let children = ordered_archive_children(&root.children);
            render_archive_tree(
                &children,
                "",
                &mut remaining,
                &mut rendered_items,
                &mut lines,
                palette,
            );
            tree_truncated = rendered_items < total_items;
        }
    }

    let mut preview =
        PreviewContent::new(PreviewKind::Archive, lines).with_detail("ISO disk image");
    let truncation_note = match (tree_truncated, total_items, rendered_items) {
        (true, total, rendered) if total > 0 && rendered > 0 => {
            Some(format!("showing first {rendered} of {total} entries"))
        }
        (true, total, _) if total > 0 => {
            Some(format!("showing first {PREVIEW_RENDER_LINE_LIMIT} lines"))
        }
        _ => None,
    };
    if let Some(note) = truncation_note {
        preview = preview.with_truncation(note);
    }
    preview
}

fn parse_iso_text_field(bytes: &[u8], start: usize, end: usize) -> Option<String> {
    let field = bytes.get(start..end)?;
    let text = String::from_utf8_lossy(field);
    let trimmed = text.trim_matches(char::from(0)).trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn parse_iso_datetime_field(bytes: &[u8], start: usize, end: usize) -> Option<String> {
    let raw = parse_iso_text_field(bytes, start, end)?;
    format_iso_datetime(&raw)
}

fn format_iso_datetime(value: &str) -> Option<String> {
    let digits = value.as_bytes();
    if digits.len() != 17 || digits[..16].iter().any(|byte| !byte.is_ascii_digit()) {
        return None;
    }
    if digits[..16].iter().all(|byte| *byte == b'0') {
        return None;
    }

    let year = &value[0..4];
    let month = &value[4..6];
    let day = &value[6..8];
    let hour = &value[8..10];
    let minute = &value[10..12];
    let second = &value[12..14];
    Some(format!("{year}-{month}-{day} {hour}:{minute}:{second}"))
}

fn parse_iso_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let raw = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(raw.try_into().ok()?))
}

fn parse_iso_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let raw = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes(raw.try_into().ok()?))
}
