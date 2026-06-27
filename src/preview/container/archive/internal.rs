use super::external::collect_archive_entries_with_bsdtar;
use super::format::archive_format_name;
use super::*;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use sevenz_rust2::{ArchiveReader as SevenZipArchiveReader, Password as SevenZipPassword};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
use tar::Archive as TarArchive;
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

pub(super) fn collect_internal_archive_listing(
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

pub(super) fn collect_preferred_archive_entries(
    path: &Path,
    format: ArchiveFormat,
    canceled: &impl Fn() -> bool,
) -> Option<Vec<ArchiveEntry>> {
    if prefers_internal_listing(format) {
        // If internal TAR parsing fails, keep bsdtar as the only tar-family CLI fallback.
        return collect_internal_archive_listing(path, format, canceled)
            .map(|(_, entries, _, _)| entries)
            .or_else(|| collect_archive_entries_with_bsdtar(path, canceled));
    }

    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn collects_native_compressed_tar_listings() {
        let root = temp_path("native-compressed-tar-listing");

        let tar_xz = root.join("sample.tar.xz");
        write_compressed_tar(&tar_xz, |file| Ok(xz2::write::XzEncoder::new(file, 6)));
        assert_sample_listing(&tar_xz, ArchiveFormat::TarXz, "TAR.XZ");

        let tar_bz2 = root.join("sample.tar.bz2");
        write_compressed_tar(&tar_bz2, |file| {
            Ok(bzip2::write::BzEncoder::new(
                file,
                bzip2::Compression::best(),
            ))
        });
        assert_sample_listing(&tar_bz2, ArchiveFormat::TarBzip2, "TAR.BZ2");

        let tar_zst = root.join("sample.tar.zst");
        write_zstd_tar(&tar_zst);
        assert_sample_listing(&tar_zst, ArchiveFormat::TarZstd, "TAR.ZST");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn collects_native_seven_zip_listing() {
        let root = temp_path("native-7z-listing");
        let archive_path = root.join("sample.7z");
        write_seven_zip(&archive_path);

        let (metadata, entries, total_entries, scan_truncated) =
            collect_internal_archive_listing(&archive_path, ArchiveFormat::SevenZip, &|| false)
                .expect("7z listing should be collected natively");

        assert_eq!(metadata.format_label.as_deref(), Some("7z"));
        assert_eq!(metadata.unpacked_size, Some(5));
        assert_eq!(total_entries, 2);
        assert!(!scan_truncated);
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "dir" && entry.is_dir)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "dir/file.txt" && !entry.is_dir)
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn assert_sample_listing(path: &Path, format: ArchiveFormat, label: &str) {
        let (metadata, entries, total_entries, scan_truncated) =
            collect_internal_archive_listing(path, format, &|| false)
                .expect("archive listing should be collected natively");

        assert_eq!(metadata.format_label.as_deref(), Some(label));
        assert_eq!(metadata.unpacked_size, Some(5));
        assert_eq!(total_entries, 2);
        assert!(!scan_truncated);
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "dir" && entry.is_dir)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "dir/file.txt" && !entry.is_dir)
        );
    }

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("elio-{label}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_compressed_tar<W, D>(archive_path: &Path, encoder: D)
    where
        W: Write,
        D: FnOnce(File) -> std::io::Result<W>,
    {
        let file = File::create(archive_path).unwrap();
        let writer = encoder(file).unwrap();
        write_tar(writer);
    }

    fn write_zstd_tar(archive_path: &Path) {
        let mut tar_bytes = Vec::new();
        write_tar(&mut tar_bytes);
        let compressed = zstd::stream::encode_all(tar_bytes.as_slice(), 0).unwrap();
        fs::write(archive_path, compressed).unwrap();
    }

    fn write_tar<W: Write>(writer: W) {
        let mut tar = tar::Builder::new(writer);
        let mut dir = tar::Header::new_gnu();
        dir.set_entry_type(tar::EntryType::Directory);
        dir.set_size(0);
        dir.set_mode(0o755);
        dir.set_cksum();
        tar.append_data(&mut dir, "dir", std::io::empty()).unwrap();

        let contents = b"hello";
        let mut file = tar::Header::new_gnu();
        file.set_size(contents.len() as u64);
        file.set_mode(0o644);
        file.set_cksum();
        tar.append_data(&mut file, "dir/file.txt", contents.as_slice())
            .unwrap();
        tar.finish().unwrap();
    }

    fn write_seven_zip(archive_path: &Path) {
        let mut writer = sevenz_rust2::ArchiveWriter::create(archive_path).unwrap();
        writer
            .push_archive_entry::<&[u8]>(sevenz_rust2::ArchiveEntry::new_directory("dir"), None)
            .unwrap();
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("dir/file.txt"),
                Some(&b"hello"[..]),
            )
            .unwrap();
        writer.finish().unwrap();
    }
}
