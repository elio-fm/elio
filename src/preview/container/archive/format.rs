use super::*;
use crate::file_info;
use std::path::Path;

pub(super) fn detect_archive_format(path: &Path) -> ArchiveFormat {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default();
    if let Some(kind) = file_info::inspect_compound_archive_name(&name) {
        return match kind {
            file_info::CompoundArchiveKind::TarGzip => ArchiveFormat::TarGzip,
            file_info::CompoundArchiveKind::TarXz => ArchiveFormat::TarXz,
            file_info::CompoundArchiveKind::TarBzip2 => ArchiveFormat::TarBzip2,
            file_info::CompoundArchiveKind::TarZstd => ArchiveFormat::TarZstd,
            file_info::CompoundArchiveKind::CompressedDiskImage {
                compression: file_info::CompressionKind::Gzip,
                ..
            } => ArchiveFormat::Gzip,
            file_info::CompoundArchiveKind::CompressedDiskImage {
                compression: file_info::CompressionKind::Xz,
                ..
            } => ArchiveFormat::Xz,
            file_info::CompoundArchiveKind::CompressedDiskImage {
                compression: file_info::CompressionKind::Bzip2,
                ..
            } => ArchiveFormat::Bzip2,
            file_info::CompoundArchiveKind::CompressedDiskImage {
                compression: file_info::CompressionKind::Zstd,
                ..
            } => ArchiveFormat::Zstd,
        };
    }

    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("cbz") => ArchiveFormat::ComicZip,
        Some("cbr") => ArchiveFormat::ComicRar,
        Some("zip" | "jar" | "apk" | "aab" | "apkg") => ArchiveFormat::Zip,
        Some("7z") => ArchiveFormat::SevenZip,
        Some("tar") => ArchiveFormat::Tar,
        Some("gz") => ArchiveFormat::Gzip,
        Some("xz") => ArchiveFormat::Xz,
        Some("bz2") => ArchiveFormat::Bzip2,
        Some("zst") => ArchiveFormat::Zstd,
        Some("deb") => ArchiveFormat::Deb,
        Some("rpm") => ArchiveFormat::Rpm,
        Some("appimage") => ArchiveFormat::AppImage,
        _ => ArchiveFormat::Unknown,
    }
}

pub(super) fn archive_default_label(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::ComicZip => "Comic ZIP archive",
        ArchiveFormat::ComicRar => "Comic RAR archive",
        ArchiveFormat::Zip => "ZIP archive",
        ArchiveFormat::SevenZip => "7z archive",
        ArchiveFormat::Tar => "TAR archive",
        ArchiveFormat::TarGzip => "TAR.GZ archive",
        ArchiveFormat::TarXz => "TAR.XZ archive",
        ArchiveFormat::TarBzip2 => "TAR.BZ2 archive",
        ArchiveFormat::TarZstd => "TAR.ZST archive",
        ArchiveFormat::Gzip => "Gzip archive",
        ArchiveFormat::Xz => "XZ archive",
        ArchiveFormat::Bzip2 => "Bzip2 archive",
        ArchiveFormat::Zstd => "Zstandard archive",
        ArchiveFormat::Deb => "Debian package",
        ArchiveFormat::Rpm => "RPM package",
        ArchiveFormat::AppImage => "AppImage bundle",
        ArchiveFormat::Unknown => "Archive",
    }
}

pub(super) fn archive_format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::ComicZip => "ZIP",
        ArchiveFormat::ComicRar => "RAR",
        ArchiveFormat::Zip => "ZIP",
        ArchiveFormat::SevenZip => "7z",
        ArchiveFormat::Tar => "TAR",
        ArchiveFormat::TarGzip => "TAR.GZ",
        ArchiveFormat::TarXz => "TAR.XZ",
        ArchiveFormat::TarBzip2 => "TAR.BZ2",
        ArchiveFormat::TarZstd => "TAR.ZST",
        ArchiveFormat::Gzip => "Gzip",
        ArchiveFormat::Xz => "XZ",
        ArchiveFormat::Bzip2 => "Bzip2",
        ArchiveFormat::Zstd => "Zstandard",
        ArchiveFormat::Deb => "DEB",
        ArchiveFormat::Rpm => "RPM",
        ArchiveFormat::AppImage => "AppImage",
        ArchiveFormat::Unknown => "Archive",
    }
}

pub(super) fn archive_is_empty_label(_format: ArchiveFormat) -> &'static str {
    "Archive is empty"
}
