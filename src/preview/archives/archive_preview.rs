pub(super) const ARCHIVE_ENTRY_SCAN_LIMIT: usize = 50_000;
pub(super) const ZIP_MANIFEST_LIMIT_BYTES: u64 = 64 * 1024;
pub(in crate::preview) const ZIP_BUILT_IN_READER_MAX_BYTES: u64 = 256 * 1024 * 1024;

use super::super::PreviewContent;
use super::archive_formats::{
    ArchiveFormat, archive_default_label, archive_format_name, detect_archive_format,
};
use super::archive_reading::{
    read_archive_listing, read_preferred_archive_entries, read_zip_archive_listing,
    seven_zip_listing_requires_password,
};
use super::archive_rendering::{ArchiveMetadata, ArchiveRenderConfig, render_archive_preview};
use super::comics::build_comic_archive_preview;
use super::external_tools::{
    collect_archive_entries_with_bsdtar, collect_archive_entries_with_unrar,
    collect_archive_listing_with_7z, fallback_single_file_archive_entry,
};
use std::{fs, path::Path};

const ARCHIVE_EMPTY_LABEL: &str = "Archive is empty";

pub(in crate::preview) fn build_archive_preview<F>(
    path: &Path,
    type_detail: Option<&'static str>,
    comic_page_index: Option<usize>,
    canceled: &F,
) -> Option<PreviewContent>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }
    let format = detect_archive_format(path);
    if matches!(format, ArchiveFormat::ComicZip | ArchiveFormat::ComicRar)
        && let Some(preview) = build_comic_archive_preview(
            path,
            format,
            type_detail,
            comic_page_index.unwrap_or(0),
            canceled,
        )
    {
        return Some(preview);
    }
    if let Some(preview) = build_zip_archive_preview(path, format, type_detail, canceled) {
        return Some(preview);
    }
    if let Some(preview) = build_archive_listing_preview(path, format, type_detail, canceled) {
        return Some(preview);
    }
    if matches!(format, ArchiveFormat::SevenZip)
        && seven_zip_listing_requires_password(path, canceled)
    {
        let detail = type_detail.unwrap_or(archive_default_label(format));
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata: ArchiveMetadata {
                format_label: Some(archive_format_name(format).to_string()),
                physical_size: fs::metadata(path).ok().map(|metadata| metadata.len()),
                ..ArchiveMetadata::default()
            },
            entries: None,
            total_entries_hint: None,
            empty_label: ARCHIVE_EMPTY_LABEL,
            unavailable_label: "Password-protected",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }
    if let Some(preview) = build_external_archive_preview(path, format, type_detail, canceled) {
        return Some(preview);
    }
    if canceled() {
        None
    } else {
        Some(build_unavailable_archive_preview(path, format, type_detail))
    }
}

fn build_zip_archive_preview<F>(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
    canceled: &F,
) -> Option<PreviewContent>
where
    F: Fn() -> bool,
{
    let listing = read_zip_archive_listing(path, format, canceled)?;
    let detail = type_detail.unwrap_or(archive_default_label(format));
    Some(render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata: listing.metadata,
        entries: Some(listing.entries),
        total_entries_hint: Some(listing.total_entries),
        empty_label: ARCHIVE_EMPTY_LABEL,
        unavailable_label: "Unable to read archive contents",
        extra_sections: listing.extra_sections,
        scan_truncated: listing.scan_truncated,
    }))
}

fn build_archive_listing_preview<F>(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
    canceled: &F,
) -> Option<PreviewContent>
where
    F: Fn() -> bool,
{
    let (metadata, entries, total_entries, scan_truncated) =
        read_archive_listing(path, format, canceled)?;
    let detail = type_detail.unwrap_or(archive_default_label(format));

    Some(render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata,
        entries: Some(entries),
        total_entries_hint: Some(total_entries),
        empty_label: ARCHIVE_EMPTY_LABEL,
        unavailable_label: "Unable to read archive contents",
        extra_sections: Vec::new(),
        scan_truncated,
    }))
}

fn build_external_archive_preview<F>(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
    canceled: &F,
) -> Option<PreviewContent>
where
    F: Fn() -> bool,
{
    // Common ZIP, TAR, and 7z previews use built-in readers above. This path
    // is for recovery and uncommon archive types, where 7z provides the
    // broadest coverage and bsdtar remains a final generic fallback.
    let detail = type_detail.unwrap_or(archive_default_label(format));
    if canceled() {
        return None;
    }
    if let Some(entries) = read_preferred_archive_entries(path, format, canceled)
        && !entries.is_empty()
    {
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata: ArchiveMetadata {
                format_label: Some(archive_format_name(format).to_string()),
                ..ArchiveMetadata::default()
            },
            entries: Some(entries),
            total_entries_hint: None,
            empty_label: ARCHIVE_EMPTY_LABEL,
            unavailable_label: "Unable to read archive contents",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }

    if canceled() {
        return None;
    }
    if let Some((metadata, mut entries)) = collect_archive_listing_with_7z(path, canceled) {
        if entries.is_empty()
            && let Some(entry) = fallback_single_file_archive_entry(path, format)
        {
            entries.push(entry);
        }
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata,
            entries: Some(entries),
            total_entries_hint: None,
            empty_label: ARCHIVE_EMPTY_LABEL,
            unavailable_label: "Unable to read archive contents",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }

    if matches!(format, ArchiveFormat::Rar)
        && let Some(entries) = collect_archive_entries_with_unrar(path, canceled)
        && !entries.is_empty()
    {
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata: ArchiveMetadata {
                format_label: Some(archive_format_name(format).to_string()),
                physical_size: fs::metadata(path).ok().map(|metadata| metadata.len()),
                ..ArchiveMetadata::default()
            },
            entries: Some(entries),
            total_entries_hint: None,
            empty_label: ARCHIVE_EMPTY_LABEL,
            unavailable_label: "Unable to read archive contents",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }

    if canceled() {
        return None;
    }
    let entries = collect_archive_entries_with_bsdtar(path, canceled)?;
    if entries.is_empty() {
        return None;
    }

    Some(render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata: ArchiveMetadata {
            format_label: Some(archive_format_name(format).to_string()),
            ..ArchiveMetadata::default()
        },
        entries: Some(entries),
        total_entries_hint: None,
        empty_label: ARCHIVE_EMPTY_LABEL,
        unavailable_label: "Unable to read archive contents",
        extra_sections: Vec::new(),
        scan_truncated: false,
    }))
}

fn build_unavailable_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
) -> PreviewContent {
    let detail = type_detail.unwrap_or(archive_default_label(format));
    render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata: ArchiveMetadata {
            format_label: Some(archive_format_name(format).to_string()),
            physical_size: fs::metadata(path).ok().map(|metadata| metadata.len()),
            ..ArchiveMetadata::default()
        },
        entries: None,
        total_entries_hint: None,
        empty_label: ARCHIVE_EMPTY_LABEL,
        unavailable_label: "Unavailable",
        extra_sections: Vec::new(),
        scan_truncated: false,
    })
}

#[cfg(test)]
#[path = "tests/archive_preview.rs"]
mod tests;
