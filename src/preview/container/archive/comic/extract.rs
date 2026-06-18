use super::super::common::system_time_key;
use super::types::{CachedComicArchive, ComicArchiveBackend, ComicArchivePage};
use crate::preview::process::run_command_capture_stdout_cancellable;
use crate::preview::{PreviewVisual, PreviewVisualKind, PreviewVisualLayout};
use std::{
    collections::hash_map::DefaultHasher,
    env,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
use zip::ZipArchive;

const COMIC_ARCHIVE_IMAGE_ENTRY_LIMIT_BYTES: usize = 32 * 1024 * 1024;

pub(super) fn extract_comic_archive_page_visual<F>(
    archive_path: &Path,
    comic: &CachedComicArchive,
    page: &ComicArchivePage,
    canceled: &F,
) -> Option<PreviewVisual>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let cache_path = archive_asset_cache_path(archive_path, &page.entry_name, &page.extension)?;
    if !cache_path.exists() {
        if canceled() {
            return None;
        }
        let bytes = match comic.backend {
            ComicArchiveBackend::Zip => {
                let physical_size = fs::metadata(archive_path)
                    .ok()
                    .map(|metadata| metadata.len());
                if physical_size
                    .is_some_and(|size| size > super::super::ZIP_INTERNAL_PREVIEW_MAX_BYTES)
                {
                    return None;
                }
                let file = File::open(archive_path).ok()?;
                if canceled() {
                    return None;
                }
                let mut archive = ZipArchive::new(file).ok()?;
                read_zip_entry_bytes_limited(
                    &mut archive,
                    &page.entry_name,
                    COMIC_ARCHIVE_IMAGE_ENTRY_LIMIT_BYTES,
                    canceled,
                )?
            }
            ComicArchiveBackend::SevenZip => read_7z_entry_bytes_limited(
                archive_path,
                &page.entry_name,
                COMIC_ARCHIVE_IMAGE_ENTRY_LIMIT_BYTES,
                canceled,
            )?,
            ComicArchiveBackend::Unrar => read_unrar_entry_bytes_limited(
                archive_path,
                &page.entry_name,
                COMIC_ARCHIVE_IMAGE_ENTRY_LIMIT_BYTES,
                canceled,
            )?,
        };
        if canceled() {
            return None;
        }
        fs::write(&cache_path, bytes).ok()?;
    }
    if canceled() {
        return None;
    }
    let metadata = fs::metadata(&cache_path).ok()?;
    Some(PreviewVisual {
        kind: PreviewVisualKind::PageImage,
        layout: PreviewVisualLayout::FullHeight,
        path: cache_path,
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

pub(super) fn read_zip_entry_bytes_limited<R, F>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit_bytes: usize,
    canceled: &F,
) -> Option<Vec<u8>>
where
    R: Read + std::io::Seek,
    F: Fn() -> bool,
{
    let mut entry = archive.by_name(name).ok()?;
    let limit = (entry.size() as usize).min(limit_bytes);
    let mut bytes = Vec::with_capacity(limit);
    let mut buffer = [0_u8; 64 * 1024];
    while bytes.len() < limit {
        if canceled() {
            return None;
        }
        let remaining = (limit - bytes.len()).min(buffer.len());
        let read = entry.read(&mut buffer[..remaining]).ok()?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    (!bytes.is_empty()).then_some(bytes)
}

pub(super) fn read_7z_entry_bytes_limited<F>(
    archive_path: &Path,
    entry_name: &str,
    limit_bytes: usize,
    canceled: &F,
) -> Option<Vec<u8>>
where
    F: Fn() -> bool,
{
    let mut command = Command::new("7z");
    command
        .arg("x")
        .arg("-so")
        .arg(archive_path)
        .arg(entry_name);
    let output = run_command_capture_stdout_cancellable(command, "comic-extract", canceled)?;
    if output.is_empty() || output.len() > limit_bytes {
        return None;
    }
    Some(output)
}

pub(super) fn read_unrar_entry_bytes_limited<F>(
    archive_path: &Path,
    entry_name: &str,
    limit_bytes: usize,
    canceled: &F,
) -> Option<Vec<u8>>
where
    F: Fn() -> bool,
{
    let mut command = Command::new("unrar");
    command
        .arg("p")
        .arg("-inul")
        .arg(archive_path)
        .arg(entry_name);
    let output = run_command_capture_stdout_cancellable(command, "comic-extract", canceled)?;
    if output.is_empty() || output.len() > limit_bytes {
        return None;
    }
    Some(output)
}

fn archive_asset_cache_path(
    archive_path: &Path,
    entry_name: &str,
    extension: &str,
) -> Option<PathBuf> {
    let metadata = fs::metadata(archive_path).ok();
    let modified = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_key);
    let mut hasher = DefaultHasher::new();
    archive_path.hash(&mut hasher);
    entry_name.hash(&mut hasher);
    metadata
        .as_ref()
        .map(|metadata| metadata.len())
        .hash(&mut hasher);
    modified.hash(&mut hasher);
    let cache_dir = env::temp_dir().join("elio-archive-asset");
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!("comic-{:016x}.{extension}", hasher.finish())))
}
