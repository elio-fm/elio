mod comic;
mod common;
mod external;
mod format;
mod internal;
mod manifest;
mod render;

const ARCHIVE_ENTRY_SCAN_LIMIT: usize = 50_000;
const ZIP_MANIFEST_LIMIT_BYTES: u64 = 64 * 1024;
const ZIP_INTERNAL_PREVIEW_MAX_BYTES: u64 = 256 * 1024 * 1024;

pub(super) use self::common::{ArchiveEntry, ArchiveTreeNode};
pub(in crate::preview) use self::format::ArchiveFormat;

use self::comic::build_comic_archive_preview;
use self::common::ArchiveMetadata;
use self::common::normalize_archive_path;
use self::external::{
    collect_archive_entries_with_bsdtar, collect_archive_entries_with_unrar,
    collect_archive_listing_with_7z, fallback_single_file_archive_entry,
};
use self::format::{archive_default_label, archive_format_name, detect_archive_format};
use self::internal::{
    collect_internal_archive_listing, collect_preferred_archive_entries,
    seven_zip_listing_requires_password,
};
use self::manifest::{ZipManifestMetadata, parse_zip_manifest, zip_manifest_sections};
use self::render::{ArchiveRenderConfig, render_archive_preview};
use super::*;
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
use zip::ZipArchive;

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
    if let Some(preview) = build_internal_archive_preview(path, format, type_detail, canceled) {
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
    if !matches!(format, ArchiveFormat::Zip | ArchiveFormat::ComicZip) {
        return None;
    }

    let physical_size = fs::metadata(path).ok().map(|metadata| metadata.len());
    if canceled() || physical_size.is_some_and(|size| size > ZIP_INTERNAL_PREVIEW_MAX_BYTES) {
        return None;
    }

    let file = File::open(path).ok()?;
    if canceled() {
        return None;
    }
    let mut archive = ZipArchive::new(file).ok()?;
    if canceled() {
        return None;
    }
    let total_entries = archive.len();
    let mut entries = Vec::with_capacity(total_entries.min(ARCHIVE_ENTRY_SCAN_LIMIT));
    let mut metadata = ArchiveMetadata {
        format_label: Some(archive_format_name(format).to_string()),
        physical_size,
        ..ArchiveMetadata::default()
    };
    let mut manifest = ZipManifestMetadata::default();

    for index in 0..total_entries.min(ARCHIVE_ENTRY_SCAN_LIMIT) {
        if canceled() {
            return None;
        }
        let entry = archive.by_index(index).ok()?;
        let is_dir = entry.is_dir();
        let name = entry.name().to_string();
        if let Some(path) = normalize_archive_path(&name, false) {
            entries.push(ArchiveEntry { path, is_dir });
        }
        metadata.unpacked_size = Some(
            metadata
                .unpacked_size
                .unwrap_or(0)
                .saturating_add(entry.size()),
        );
        metadata.compressed_size = Some(
            metadata
                .compressed_size
                .unwrap_or(0)
                .saturating_add(entry.compressed_size()),
        );

        if manifest.is_empty()
            && !is_dir
            && name.eq_ignore_ascii_case("META-INF/MANIFEST.MF")
            && entry.size() <= ZIP_MANIFEST_LIMIT_BYTES
        {
            let mut contents = String::new();
            if entry
                .take(ZIP_MANIFEST_LIMIT_BYTES)
                .read_to_string(&mut contents)
                .is_ok()
            {
                manifest = parse_zip_manifest(&contents);
            }
        }
    }

    let comment = String::from_utf8_lossy(archive.comment());
    let comment = comment.trim();
    if !comment.is_empty() {
        metadata.comment = Some(comment.to_string());
    }

    let detail = type_detail.unwrap_or(archive_default_label(format));
    let scan_truncated = total_entries > ARCHIVE_ENTRY_SCAN_LIMIT;
    let preview = render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata,
        entries: Some(entries),
        total_entries_hint: Some(total_entries),
        empty_label: ARCHIVE_EMPTY_LABEL,
        unavailable_label: "Unable to read archive contents",
        extra_sections: zip_manifest_sections(&manifest),
        scan_truncated,
    });
    Some(preview)
}

fn build_internal_archive_preview<F>(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
    canceled: &F,
) -> Option<PreviewContent>
where
    F: Fn() -> bool,
{
    let (metadata, entries, total_entries, scan_truncated) =
        collect_internal_archive_listing(path, format, canceled)?;
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
    // Common ZIP, TAR, and 7z previews are handled internally above. This path
    // is for recovery and uncommon archive types, where 7z provides the
    // broadest coverage and bsdtar remains a final generic fallback.
    let detail = type_detail.unwrap_or(archive_default_label(format));
    if canceled() {
        return None;
    }
    if let Some(entries) = collect_preferred_archive_entries(path, format, canceled)
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
mod tests {
    use super::{ArchiveFormat, ZIP_INTERNAL_PREVIEW_MAX_BYTES, build_zip_archive_preview};
    use std::{
        fs,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn oversized_zip_skips_internal_reader() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "elio-oversized-zip-internal-skip-{unique}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("huge.zip");
        let file = fs::File::create(&path).expect("failed to create sparse zip fixture");
        file.set_len(ZIP_INTERNAL_PREVIEW_MAX_BYTES + 1)
            .expect("failed to size sparse zip fixture");

        let started_at = Instant::now();
        let preview = build_zip_archive_preview(&path, ArchiveFormat::Zip, None, &|| false);

        assert!(preview.is_none());
        assert!(
            started_at.elapsed().as_millis() < 100,
            "oversized ZIP files should skip the uncancellable zip reader"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
