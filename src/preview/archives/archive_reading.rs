use super::archive_formats::{ArchiveFormat, archive_format_name};
use super::archive_preview::{
    ARCHIVE_ENTRY_SCAN_LIMIT, ZIP_BUILT_IN_READER_MAX_BYTES, ZIP_MANIFEST_LIMIT_BYTES,
};
use super::archive_rendering::{ArchiveEntry, ArchiveMetadata, normalize_archive_path};
use super::external_tools::collect_archive_entries_with_bsdtar;
use super::zip_manifest::{ZipManifestMetadata, parse_zip_manifest, zip_manifest_sections};
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use sevenz_rust2::{
    ArchiveReader as SevenZipArchiveReader, Error as SevenZipError, Password as SevenZipPassword,
};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
use tar::Archive as TarArchive;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

pub(super) struct ArchiveListing {
    pub(super) metadata: ArchiveMetadata,
    pub(super) entries: Vec<ArchiveEntry>,
    pub(super) total_entries: usize,
    pub(super) extra_sections: Vec<(&'static str, Vec<(&'static str, String)>)>,
    pub(super) scan_truncated: bool,
}

pub(super) fn read_zip_archive_listing<F>(
    path: &Path,
    format: ArchiveFormat,
    canceled: &F,
) -> Option<ArchiveListing>
where
    F: Fn() -> bool,
{
    if !matches!(format, ArchiveFormat::Zip | ArchiveFormat::ComicZip) {
        return None;
    }

    let physical_size = fs::metadata(path).ok().map(|metadata| metadata.len());
    if canceled() || physical_size.is_some_and(|size| size > ZIP_BUILT_IN_READER_MAX_BYTES) {
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

    Some(ArchiveListing {
        metadata,
        entries,
        total_entries,
        extra_sections: zip_manifest_sections(&manifest),
        scan_truncated: total_entries > ARCHIVE_ENTRY_SCAN_LIMIT,
    })
}

pub(super) fn read_archive_listing(
    path: &Path,
    format: ArchiveFormat,
    canceled: &impl Fn() -> bool,
) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>, usize, bool)> {
    if canceled() {
        return None;
    }

    match format {
        ArchiveFormat::Tar => {
            let file = File::open(path).ok()?;
            collect_tar_listing_from_reader(file, path, format, canceled)
        }
        ArchiveFormat::TarGzip => {
            let file = File::open(path).ok()?;
            collect_tar_listing_from_reader(GzDecoder::new(file), path, format, canceled)
        }
        ArchiveFormat::TarXz => {
            let file = File::open(path).ok()?;
            collect_tar_listing_from_reader(XzDecoder::new(file), path, format, canceled)
        }
        ArchiveFormat::TarBzip2 => {
            let file = File::open(path).ok()?;
            collect_tar_listing_from_reader(BzDecoder::new(file), path, format, canceled)
        }
        ArchiveFormat::TarZstd => {
            let file = File::open(path).ok()?;
            let decoder = ZstdDecoder::new(file).ok()?;
            collect_tar_listing_from_reader(decoder, path, format, canceled)
        }
        ArchiveFormat::SevenZip => collect_seven_zip_listing(path, format, canceled),
        _ => None,
    }
}

pub(super) fn read_preferred_archive_entries(
    path: &Path,
    format: ArchiveFormat,
    canceled: &impl Fn() -> bool,
) -> Option<Vec<ArchiveEntry>> {
    if prefers_built_in_reader(format) {
        // If the built-in TAR reader fails, keep bsdtar as the only tar-family fallback.
        return read_archive_listing(path, format, canceled)
            .map(|(_, entries, _, _)| entries)
            .or_else(|| collect_archive_entries_with_bsdtar(path, canceled));
    }

    None
}

pub(super) fn seven_zip_listing_requires_password(
    path: &Path,
    canceled: &impl Fn() -> bool,
) -> bool {
    if canceled() {
        return false;
    }
    let Ok(file) = File::open(path) else {
        return false;
    };
    let error = SevenZipArchiveReader::new(file, SevenZipPassword::empty()).err();
    matches!(
        error,
        Some(SevenZipError::PasswordRequired | SevenZipError::MaybeBadPassword(_))
    ) && !canceled()
}

fn collect_seven_zip_listing(
    path: &Path,
    format: ArchiveFormat,
    canceled: &impl Fn() -> bool,
) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>, usize, bool)> {
    let file = File::open(path).ok()?;
    let archive = SevenZipArchiveReader::new(file, SevenZipPassword::empty()).ok()?;
    if canceled() {
        return None;
    }

    let files = &archive.archive().files;
    let total_entries = files.len();
    let mut entries = Vec::with_capacity(total_entries.min(ARCHIVE_ENTRY_SCAN_LIMIT));
    let mut metadata = ArchiveMetadata {
        format_label: Some(archive_format_name(format).to_string()),
        physical_size: fs::metadata(path).ok().map(|metadata| metadata.len()),
        ..ArchiveMetadata::default()
    };

    for entry in files.iter().take(ARCHIVE_ENTRY_SCAN_LIMIT) {
        if canceled() {
            return None;
        }
        if entry.is_anti_item {
            continue;
        }

        metadata.unpacked_size = Some(
            metadata
                .unpacked_size
                .unwrap_or(0)
                .saturating_add(entry.size),
        );
        metadata.compressed_size = Some(
            metadata
                .compressed_size
                .unwrap_or(0)
                .saturating_add(entry.compressed_size),
        );

        if let Some(path) = normalize_archive_path(&entry.name, false) {
            entries.push(ArchiveEntry {
                path,
                is_dir: entry.is_directory,
            });
        }
    }

    Some((
        metadata,
        entries,
        total_entries,
        total_entries > ARCHIVE_ENTRY_SCAN_LIMIT,
    ))
}

fn collect_tar_listing_from_reader<R: Read>(
    reader: R,
    path: &Path,
    format: ArchiveFormat,
    canceled: &impl Fn() -> bool,
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
        if canceled() {
            return None;
        }

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

fn prefers_built_in_reader(format: ArchiveFormat) -> bool {
    matches!(
        format,
        ArchiveFormat::Tar
            | ArchiveFormat::TarGzip
            | ArchiveFormat::TarXz
            | ArchiveFormat::TarBzip2
            | ArchiveFormat::TarZstd
    )
}

#[cfg(test)]
#[path = "tests/archive_reading.rs"]
mod tests;
