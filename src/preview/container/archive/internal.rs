use super::external::collect_archive_entries_with_bsdtar;
use super::format::archive_format_name;
use super::*;
use crate::preview::ArchiveFormat;
use flate2::read::GzDecoder;
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
use tar::Archive as TarArchive;

pub(super) fn collect_internal_archive_listing(
    path: &Path,
    format: ArchiveFormat,
) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>, usize, bool)> {
    match format {
        ArchiveFormat::Tar => {
            let file = File::open(path).ok()?;
            collect_tar_listing_from_reader(file, path, format)
        }
        ArchiveFormat::TarGzip => {
            let file = File::open(path).ok()?;
            collect_tar_listing_from_reader(GzDecoder::new(file), path, format)
        }
        _ => None,
    }
}

pub(super) fn collect_preferred_archive_entries(
    path: &Path,
    format: ArchiveFormat,
) -> Option<Vec<ArchiveEntry>> {
    if prefers_internal_listing(format) {
        // If internal TAR parsing fails, keep bsdtar as the only tar-family CLI fallback.
        return collect_internal_archive_listing(path, format)
            .map(|(_, entries, _, _)| entries)
            .or_else(|| collect_archive_entries_with_bsdtar(path));
    }

    None
}

fn collect_tar_listing_from_reader<R: Read>(
    reader: R,
    path: &Path,
    format: ArchiveFormat,
) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>, usize, bool)> {
    let mut archive = TarArchive::new(reader);
    let entries = archive.entries().ok()?;
    let mut normalized_entries = Vec::new();
    let mut metadata = ArchiveMetadata {
        format_label: Some(archive_format_name(format).to_string()),
        physical_size: fs::metadata(path).ok().map(|metadata| metadata.len()),
        ..ArchiveMetadata::default()
    };
    let mut total_entries = 0usize;
    let mut scan_truncated = false;

    for entry in entries {
        let entry = entry.ok()?;
        total_entries = total_entries.saturating_add(1);
        if total_entries > ARCHIVE_ENTRY_SCAN_LIMIT {
            scan_truncated = true;
            break;
        }

        let is_dir = entry.header().entry_type().is_dir();
        metadata.unpacked_size = Some(
            metadata
                .unpacked_size
                .unwrap_or(0)
                .saturating_add(entry.header().size().ok().unwrap_or(0)),
        );

        let path = entry.path().ok()?;
        let path = path.to_string_lossy();
        if let Some(path) = normalize_archive_path(&path, false) {
            normalized_entries.push(ArchiveEntry { path, is_dir });
        }
    }

    Some((metadata, normalized_entries, total_entries, scan_truncated))
}

fn prefers_internal_listing(format: ArchiveFormat) -> bool {
    matches!(
        format,
        ArchiveFormat::Tar
            | ArchiveFormat::TarGzip
            | ArchiveFormat::TarXz
            | ArchiveFormat::TarBzip2
            | ArchiveFormat::TarZstd
    )
}
