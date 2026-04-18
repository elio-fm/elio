use super::types::plain;
use super::{CompoundArchiveKind, CompressionKind, DiskImageKind, FileFacts};
use crate::core::FileClass;

pub(super) fn inspect_archive_name(name: &str) -> Option<FileFacts> {
    let detail = if let Some(kind) = inspect_compound_archive_name(name) {
        Some(kind.detail_label())
    } else if name.ends_with(".cbz") {
        Some("Comic ZIP archive")
    } else if name.ends_with(".cbr") {
        Some("Comic RAR archive")
    } else if name.ends_with(".rar") {
        Some("RAR archive")
    } else if name.ends_with(".zip") {
        Some("ZIP archive")
    } else if name.ends_with(".7z") {
        Some("7z archive")
    } else if name.ends_with(".jar") {
        Some("Java archive")
    } else if name.ends_with(".apk") {
        Some("Android package")
    } else if name.ends_with(".aab") {
        Some("Android App Bundle")
    } else if name.ends_with(".apkg") {
        Some("Anki package")
    } else {
        None
    }?;

    Some(plain(FileClass::Archive, Some(detail)))
}

pub(crate) fn inspect_compound_archive_name(name: &str) -> Option<CompoundArchiveKind> {
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return Some(CompoundArchiveKind::TarGzip);
    }
    if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        return Some(CompoundArchiveKind::TarXz);
    }
    if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") || name.ends_with(".tbz") {
        return Some(CompoundArchiveKind::TarBzip2);
    }
    if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        return Some(CompoundArchiveKind::TarZstd);
    }

    inspect_compressed_disk_image_name(name)
}

fn inspect_compressed_disk_image_name(name: &str) -> Option<CompoundArchiveKind> {
    compressed_disk_image_kind(name, ".gz", CompressionKind::Gzip)
        .or_else(|| compressed_disk_image_kind(name, ".xz", CompressionKind::Xz))
        .or_else(|| compressed_disk_image_kind(name, ".bz2", CompressionKind::Bzip2))
        .or_else(|| compressed_disk_image_kind(name, ".zst", CompressionKind::Zstd))
}

fn compressed_disk_image_kind(
    name: &str,
    compression_suffix: &str,
    compression: CompressionKind,
) -> Option<CompoundArchiveKind> {
    name.strip_suffix(compression_suffix)
        .and_then(disk_image_kind_from_name)
        .map(|image| CompoundArchiveKind::CompressedDiskImage { image, compression })
}

fn disk_image_kind_from_name(name: &str) -> Option<DiskImageKind> {
    if name.ends_with(".raw") {
        Some(DiskImageKind::Raw)
    } else if name.ends_with(".img") {
        Some(DiskImageKind::Img)
    } else if name.ends_with(".iso") {
        Some(DiskImageKind::Iso)
    } else if name.ends_with(".qcow2") {
        Some(DiskImageKind::Qcow2)
    } else if name.ends_with(".vmdk") {
        Some(DiskImageKind::Vmdk)
    } else if name.ends_with(".vdi") {
        Some(DiskImageKind::Vdi)
    } else if name.ends_with(".vhd") {
        Some(DiskImageKind::Vhd)
    } else if name.ends_with(".vhdx") {
        Some(DiskImageKind::Vhdx)
    } else {
        None
    }
}
