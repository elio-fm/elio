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
        read_archive_listing(&archive_path, ArchiveFormat::SevenZip, &|| false)
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
        read_archive_listing(path, format, &|| false)
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
