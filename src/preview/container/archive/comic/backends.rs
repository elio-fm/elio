use super::super::common::{archive_image_extension, normalize_archive_path, parse_key_value_line};
use super::extract::{
    read_7z_entry_bytes_limited, read_unrar_entry_bytes_limited, read_zip_entry_bytes_limited,
};
use super::metadata::{
    capture_comic_metadata_entry, derive_comic_archive_metadata, parse_comic_book_info_comment,
    parse_comic_metadata_xml,
};
use super::types::{
    CachedComicArchive, ComicArchiveBackend, ComicArchiveListing, ComicArchivePage,
    ComicArchiveSignature, ComicMetadataEntry,
};
use crate::fs::natural_cmp;
use crate::preview::process::run_command_capture_stdout_cancellable;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::Path,
    process::Command,
    sync::OnceLock,
};
use zip::ZipArchive;

const COMIC_INFO_ENTRY_LIMIT_BYTES: usize = 256 * 1024;

fn has_unrar() -> bool {
    static RESULT: OnceLock<bool> = OnceLock::new();
    *RESULT.get_or_init(|| Command::new("unrar").output().is_ok())
}

fn seven_zip_has_rar_support() -> bool {
    static RESULT: OnceLock<bool> = OnceLock::new();
    *RESULT.get_or_init(|| {
        Command::new("7z")
            .arg("i")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.contains("Rar"))
            .unwrap_or(false)
    })
}

pub(super) fn has_rar_capable_extractor() -> bool {
    has_unrar() || seven_zip_has_rar_support()
}

pub(super) fn sniff_comic_archive_signature(path: &Path) -> ComicArchiveSignature {
    let Ok(mut file) = File::open(path) else {
        return ComicArchiveSignature::Unknown;
    };
    let mut buf = [0u8; 8];
    let Ok(n) = file.read(&mut buf) else {
        return ComicArchiveSignature::Unknown;
    };
    if n >= 4 && matches!(&buf[..4], b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08") {
        return ComicArchiveSignature::Zip;
    }
    if n >= 6 && buf[..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
        return ComicArchiveSignature::SevenZip;
    }
    // RAR 1.5–4.x and RAR 5.0 both start with "Rar!\x1a\x07".
    if n >= 7 && buf[..4] == *b"Rar!" && buf[4] == 0x1A && buf[5] == 0x07 {
        return ComicArchiveSignature::Rar;
    }
    ComicArchiveSignature::Unknown
}

pub(super) fn parse_comic_archive<F>(path: &Path, canceled: &F) -> Option<CachedComicArchive>
where
    F: Fn() -> bool,
{
    // Comic extensions are often mislabeled in the wild (e.g. `.cbz` files that
    // actually contain RAR or 7z data). Sniff the container signature first so
    // the cold path hits the right backend immediately instead of paying for a
    // guaranteed parser miss before the real extractor runs.
    match sniff_comic_archive_signature(path) {
        ComicArchiveSignature::Zip => parse_zip_comic_archive(path, canceled)
            .or_else(|| parse_comic_archive_with_7z(path, canceled))
            .or_else(|| parse_comic_archive_with_unrar(path, canceled)),
        ComicArchiveSignature::SevenZip => parse_comic_archive_with_7z(path, canceled)
            .or_else(|| parse_zip_comic_archive(path, canceled))
            .or_else(|| parse_comic_archive_with_unrar(path, canceled)),
        ComicArchiveSignature::Rar => {
            if seven_zip_has_rar_support() {
                parse_comic_archive_with_7z(path, canceled)
                    .or_else(|| parse_comic_archive_with_unrar(path, canceled))
                    .or_else(|| parse_zip_comic_archive(path, canceled))
            } else {
                parse_comic_archive_with_unrar(path, canceled)
                    .or_else(|| parse_zip_comic_archive(path, canceled))
            }
        }
        ComicArchiveSignature::Unknown => parse_zip_comic_archive(path, canceled)
            .or_else(|| parse_comic_archive_with_7z(path, canceled))
            .or_else(|| parse_comic_archive_with_unrar(path, canceled)),
    }
}

pub(super) fn parse_zip_comic_archive<F>(path: &Path, canceled: &F) -> Option<CachedComicArchive>
where
    F: Fn() -> bool,
{
    let physical_size = fs::metadata(path).ok().map(|metadata| metadata.len());
    if canceled()
        || physical_size.is_some_and(|size| size > super::super::ZIP_INTERNAL_PREVIEW_MAX_BYTES)
    {
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
    let mut page_entries = Vec::new();
    let mut metadata_entry = None;

    // Use file_names() to iterate the central directory without seeking to each
    // entry — much faster for archives with many pages.
    let names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
    for name in &names {
        if canceled() {
            return None;
        }
        // Directory entries end with '/'; skip them without an extra seek.
        if name.ends_with('/') {
            continue;
        }
        let Some(extension) = archive_image_extension(name) else {
            capture_comic_metadata_entry(&mut metadata_entry, name);
            continue;
        };
        let sort_key = normalize_archive_path(name, false)
            .unwrap_or_else(|| name.clone())
            .to_lowercase();
        page_entries.push(ComicArchivePage {
            entry_name: name.clone(),
            sort_key,
            extension: extension.to_string(),
        });
    }

    if canceled() {
        return None;
    }
    page_entries.sort_by(|left, right| natural_cmp(&left.sort_key, &right.sort_key));
    let embedded_info = metadata_entry.as_ref().and_then(|entry| {
        read_zip_entry_bytes_limited(
            &mut archive,
            &entry.name,
            COMIC_INFO_ENTRY_LIMIT_BYTES,
            canceled,
        )
        .and_then(|bytes| parse_comic_metadata_xml(&String::from_utf8_lossy(&bytes)))
    });
    let comic_info = embedded_info.or_else(|| parse_comic_book_info_comment(archive.comment()));
    let derived_info = derive_comic_archive_metadata(path, &page_entries);

    Some(CachedComicArchive {
        backend: ComicArchiveBackend::Zip,
        page_entries,
        comic_info,
        derived_info,
    })
}

fn parse_comic_archive_with_7z<F>(path: &Path, canceled: &F) -> Option<CachedComicArchive>
where
    F: Fn() -> bool,
{
    let mut command = Command::new("7z");
    command.arg("l").arg("-slt").arg(path);
    let output = run_command_capture_stdout_cancellable(command, "comic-list", canceled)?;

    let listing = parse_comic_archive_from_7z_output(&String::from_utf8_lossy(&output), canceled)?;
    let embedded_info = listing.metadata_entry.as_ref().and_then(|entry| {
        read_7z_entry_bytes_limited(path, &entry.name, COMIC_INFO_ENTRY_LIMIT_BYTES, canceled)
            .and_then(|bytes| parse_comic_metadata_xml(&String::from_utf8_lossy(&bytes)))
    });
    let comment_info = listing
        .archive_comment
        .as_deref()
        .and_then(parse_comic_book_info_comment);
    let comic_info = embedded_info.or(comment_info);
    let derived_info = derive_comic_archive_metadata(path, &listing.page_entries);
    Some(CachedComicArchive {
        backend: listing.backend,
        page_entries: listing.page_entries,
        comic_info,
        derived_info,
    })
}

pub(super) fn parse_comic_archive_from_7z_output<F>(
    output: &str,
    canceled: &F,
) -> Option<ComicArchiveListing>
where
    F: Fn() -> bool,
{
    let mut page_entries = Vec::new();
    let mut metadata_entry = None;
    let archive_comment = parse_7z_archive_comment(output);
    let mut in_entries = false;
    let mut current = BTreeMap::<String, String>::new();

    for raw_line in output.lines() {
        if canceled() {
            return None;
        }
        let line = raw_line.trim_end();
        if line == "----------" {
            in_entries = true;
            continue;
        }

        if !in_entries {
            continue;
        }

        if line.is_empty() {
            push_7z_comic_entry(&mut current, &mut page_entries, &mut metadata_entry);
            continue;
        }

        if let Some((field, value)) = parse_key_value_line(line) {
            current.insert(field.to_string(), value.to_string());
        }
    }
    push_7z_comic_entry(&mut current, &mut page_entries, &mut metadata_entry);

    if canceled() || page_entries.is_empty() {
        return None;
    }

    page_entries.sort_by(|left, right| natural_cmp(&left.sort_key, &right.sort_key));
    Some(ComicArchiveListing {
        backend: ComicArchiveBackend::SevenZip,
        page_entries,
        metadata_entry,
        archive_comment,
    })
}

fn parse_7z_archive_comment(output: &str) -> Option<Vec<u8>> {
    let mut lines = output.lines().map(str::trim_end);
    while let Some(line) = lines.next() {
        if line == "----------" {
            return None;
        }
        let Some((field, value)) = parse_key_value_line(line) else {
            continue;
        };
        if field != "Comment" {
            continue;
        }
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.as_bytes().to_vec());
        }

        let mut comment_lines = Vec::new();
        for line in lines.by_ref() {
            if line == "----------" || parse_key_value_line(line).is_some() {
                break;
            }
            comment_lines.push(line.to_string());
        }
        return normalize_7z_multiline_comment(comment_lines);
    }
    None
}

fn normalize_7z_multiline_comment(lines: Vec<String>) -> Option<Vec<u8>> {
    let start = lines.iter().position(|line| !line.trim().is_empty())?;
    let end = lines.iter().rposition(|line| !line.trim().is_empty())?;
    let comment = lines[start..=end].join("\n");
    let comment = comment.trim();
    (!comment.is_empty()).then(|| comment.as_bytes().to_vec())
}

fn push_7z_comic_entry(
    current: &mut BTreeMap<String, String>,
    page_entries: &mut Vec<ComicArchivePage>,
    metadata_entry: &mut Option<ComicMetadataEntry>,
) {
    if current.is_empty() {
        return;
    }

    let entry_name = current.get("Path").cloned();
    let is_dir = current.get("Folder").is_some_and(|value| value == "+")
        || current
            .get("Attributes")
            .is_some_and(|value| value.starts_with('D'));

    if !is_dir && let Some(entry_name) = entry_name {
        if let Some(extension) = archive_image_extension(&entry_name) {
            let sort_key = normalize_archive_path(&entry_name, false)
                .unwrap_or_else(|| entry_name.clone())
                .to_lowercase();
            page_entries.push(ComicArchivePage {
                entry_name,
                sort_key,
                extension: extension.to_string(),
            });
        } else {
            capture_comic_metadata_entry(metadata_entry, &entry_name);
        }
    }

    current.clear();
}

fn parse_comic_archive_with_unrar<F>(path: &Path, canceled: &F) -> Option<CachedComicArchive>
where
    F: Fn() -> bool,
{
    let mut command = Command::new("unrar");
    command.arg("lb").arg(path);
    let output = run_command_capture_stdout_cancellable(command, "comic-list", canceled)?;
    let listing = String::from_utf8_lossy(&output);
    let mut page_entries = Vec::new();
    let mut metadata_entry = None;

    for line in listing.lines() {
        if canceled() {
            return None;
        }
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        let Some(extension) = archive_image_extension(name) else {
            capture_comic_metadata_entry(&mut metadata_entry, name);
            continue;
        };
        let sort_key = normalize_archive_path(name, false)
            .unwrap_or_else(|| name.to_string())
            .to_lowercase();
        page_entries.push(ComicArchivePage {
            entry_name: name.to_string(),
            sort_key,
            extension: extension.to_string(),
        });
    }

    if canceled() || page_entries.is_empty() {
        return None;
    }

    page_entries.sort_by(|a, b| natural_cmp(&a.sort_key, &b.sort_key));
    let embedded_info = metadata_entry.as_ref().and_then(|entry| {
        read_unrar_entry_bytes_limited(path, &entry.name, COMIC_INFO_ENTRY_LIMIT_BYTES, canceled)
            .and_then(|bytes| parse_comic_metadata_xml(&String::from_utf8_lossy(&bytes)))
    });
    let comment_info = embedded_info
        .is_none()
        .then(|| {
            read_unrar_archive_comment(path, canceled)
                .and_then(|comment| parse_comic_book_info_comment(&comment))
        })
        .flatten();
    let comic_info = embedded_info.or(comment_info);
    let derived_info = derive_comic_archive_metadata(path, &page_entries);
    Some(CachedComicArchive {
        backend: ComicArchiveBackend::Unrar,
        page_entries,
        comic_info,
        derived_info,
    })
}

fn read_unrar_archive_comment<F>(path: &Path, canceled: &F) -> Option<Vec<u8>>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }
    let mut command = Command::new("unrar");
    command.arg("l").arg(path);
    let output = run_command_capture_stdout_cancellable(command, "comic-comment", canceled)?;
    parse_unrar_archive_comment(&String::from_utf8_lossy(&output))
}

pub(super) fn parse_unrar_archive_comment(output: &str) -> Option<Vec<u8>> {
    let mut in_archive = false;
    let mut comment_lines = Vec::new();

    for line in output.lines().map(str::trim_end) {
        let trimmed = line.trim();
        if trimmed.starts_with("Archive:") {
            in_archive = true;
            continue;
        }
        if !in_archive {
            continue;
        }
        if trimmed.starts_with("Details:")
            || trimmed.starts_with("Attributes")
            || trimmed.starts_with("-----------")
        {
            break;
        }
        if trimmed.is_empty() && comment_lines.is_empty() {
            continue;
        }
        comment_lines.push(line.to_string());
    }

    let comment = comment_lines.join("\n");
    let comment = comment.trim();
    (!comment.is_empty()).then(|| comment.as_bytes().to_vec())
}
