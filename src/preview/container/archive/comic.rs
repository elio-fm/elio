use super::common::{
    archive_image_extension, normalize_archive_path, parse_key_value_line, system_time_key,
};
use super::format::archive_default_label;
use super::*;
use crate::fs::natural_cmp;
use crate::preview::process::run_command_capture_stdout_cancellable;
use std::{
    collections::{HashMap, VecDeque, hash_map::DefaultHasher},
    env,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, OnceLock},
};
use zip::ZipArchive;

const COMIC_ARCHIVE_IMAGE_ENTRY_LIMIT_BYTES: usize = 32 * 1024 * 1024;
const COMIC_ARCHIVE_CACHE_LIMIT: usize = 16;
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

fn has_rar_capable_extractor() -> bool {
    has_unrar() || seven_zip_has_rar_support()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComicArchiveBackend {
    Zip,
    SevenZip,
    Unrar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComicArchiveSignature {
    Zip,
    Rar,
    SevenZip,
    Unknown,
}

#[derive(Clone, Debug)]
struct ComicArchivePage {
    entry_name: String,
    sort_key: String,
    extension: String,
}

#[derive(Clone, Debug)]
struct CachedComicArchive {
    backend: ComicArchiveBackend,
    page_entries: Vec<ComicArchivePage>,
}

#[derive(Debug, Default)]
struct ComicArchiveCache {
    archives: HashMap<ComicArchiveCacheKey, Arc<CachedComicArchive>>,
    order: VecDeque<ComicArchiveCacheKey>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ComicArchiveCacheKey {
    path: PathBuf,
    size: u64,
    modified: Option<(u64, u32)>,
}

static COMIC_ARCHIVE_CACHE: OnceLock<Mutex<ComicArchiveCache>> = OnceLock::new();

pub(super) fn build_comic_archive_preview<F>(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
    page_index: usize,
    canceled: &F,
) -> Option<PreviewContent>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let Some(comic) = load_comic_archive(path, canceled) else {
        if matches!(format, ArchiveFormat::ComicRar) {
            let detail = type_detail
                .unwrap_or(archive_default_label(format))
                .to_string();
            let note = if has_rar_capable_extractor() {
                "Unable to read RAR archive (file may be corrupted or unsupported)"
            } else {
                "RAR preview requires unrar or a 7z build with RAR support"
            };
            return Some(
                PreviewContent::new(PreviewKind::Comic, Vec::new())
                    .with_detail(detail)
                    .with_status_note(note),
            );
        }
        return None;
    };
    if comic.page_entries.is_empty() {
        return None;
    }

    let current_index = page_index.min(comic.page_entries.len().saturating_sub(1));
    let detail = type_detail
        .unwrap_or(archive_default_label(format))
        .to_string();
    let mut preview = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_detail(detail)
        .with_navigation_position("Page", current_index, comic.page_entries.len(), None);

    if canceled() {
        return None;
    }

    if let Some(visual) = extract_comic_archive_page_visual(
        path,
        &comic,
        &comic.page_entries[current_index],
        canceled,
    ) {
        preview = preview.with_preview_visual(visual);
    } else {
        preview = preview.with_status_note("Unable to extract selected page");
    }

    Some(preview)
}

fn load_comic_archive<F>(path: &Path, canceled: &F) -> Option<Arc<CachedComicArchive>>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let key = comic_archive_cache_key(path)?;
    if let Some(cached) = comic_archive_cache()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .archives
        .get(&key)
        .cloned()
    {
        return Some(cached);
    }

    let parsed = Arc::new(parse_comic_archive(path, canceled)?);
    if canceled() {
        return None;
    }
    let mut cache = comic_archive_cache()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(existing) = cache.archives.get(&key).cloned() {
        return Some(existing);
    }
    cache.order.retain(|cached_key| cached_key != &key);
    cache.order.push_back(key.clone());
    cache.archives.insert(key.clone(), Arc::clone(&parsed));
    while cache.order.len() > COMIC_ARCHIVE_CACHE_LIMIT {
        if let Some(stale_key) = cache.order.pop_front() {
            cache.archives.remove(&stale_key);
        }
    }
    Some(parsed)
}

fn sniff_comic_archive_signature(path: &Path) -> ComicArchiveSignature {
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

fn parse_comic_archive<F>(path: &Path, canceled: &F) -> Option<CachedComicArchive>
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

fn parse_zip_comic_archive<F>(path: &Path, canceled: &F) -> Option<CachedComicArchive>
where
    F: Fn() -> bool,
{
    let file = File::open(path).ok()?;
    let archive = ZipArchive::new(file).ok()?;
    let mut page_entries = Vec::new();

    // Use file_names() to iterate the central directory without seeking to each
    // entry — much faster for archives with many pages.
    let names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
    for name in names {
        if canceled() {
            return None;
        }
        // Directory entries end with '/'; skip them without an extra seek.
        if name.ends_with('/') {
            continue;
        }
        let Some(extension) = archive_image_extension(&name) else {
            continue;
        };
        let sort_key = normalize_archive_path(&name, false)
            .unwrap_or_else(|| name.clone())
            .to_lowercase();
        page_entries.push(ComicArchivePage {
            entry_name: name,
            sort_key,
            extension: extension.to_string(),
        });
    }

    if canceled() {
        return None;
    }
    page_entries.sort_by(|left, right| natural_cmp(&left.sort_key, &right.sort_key));

    Some(CachedComicArchive {
        backend: ComicArchiveBackend::Zip,
        page_entries,
    })
}

fn parse_comic_archive_with_7z<F>(path: &Path, canceled: &F) -> Option<CachedComicArchive>
where
    F: Fn() -> bool,
{
    let mut command = Command::new("7z");
    command.arg("l").arg("-slt").arg(path);
    let output = run_command_capture_stdout_cancellable(command, "comic-list", canceled)?;

    parse_comic_archive_from_7z_output(&String::from_utf8_lossy(&output), canceled)
}

fn parse_comic_archive_from_7z_output<F>(output: &str, canceled: &F) -> Option<CachedComicArchive>
where
    F: Fn() -> bool,
{
    let mut page_entries = Vec::new();
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
            push_7z_comic_page_entry(&mut current, &mut page_entries);
            continue;
        }

        if let Some((field, value)) = parse_key_value_line(line) {
            current.insert(field.to_string(), value.to_string());
        }
    }
    push_7z_comic_page_entry(&mut current, &mut page_entries);

    if canceled() || page_entries.is_empty() {
        return None;
    }

    page_entries.sort_by(|left, right| natural_cmp(&left.sort_key, &right.sort_key));
    Some(CachedComicArchive {
        backend: ComicArchiveBackend::SevenZip,
        page_entries,
    })
}

fn push_7z_comic_page_entry(
    current: &mut BTreeMap<String, String>,
    page_entries: &mut Vec<ComicArchivePage>,
) {
    if current.is_empty() {
        return;
    }

    let entry_name = current.get("Path").cloned();
    let is_dir = current.get("Folder").is_some_and(|value| value == "+")
        || current
            .get("Attributes")
            .is_some_and(|value| value.starts_with('D'));

    if !is_dir
        && let Some(entry_name) = entry_name
        && let Some(extension) = archive_image_extension(&entry_name)
    {
        let sort_key = normalize_archive_path(&entry_name, false)
            .unwrap_or_else(|| entry_name.clone())
            .to_lowercase();
        page_entries.push(ComicArchivePage {
            entry_name,
            sort_key,
            extension: extension.to_string(),
        });
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

    for line in listing.lines() {
        if canceled() {
            return None;
        }
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        let Some(extension) = archive_image_extension(name) else {
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
    Some(CachedComicArchive {
        backend: ComicArchiveBackend::Unrar,
        page_entries,
    })
}

fn comic_archive_cache() -> &'static Mutex<ComicArchiveCache> {
    COMIC_ARCHIVE_CACHE.get_or_init(|| Mutex::new(ComicArchiveCache::default()))
}

fn comic_archive_cache_key(path: &Path) -> Option<ComicArchiveCacheKey> {
    let metadata = fs::metadata(path).ok()?;
    Some(ComicArchiveCacheKey {
        path: path.to_path_buf(),
        size: metadata.len(),
        modified: metadata.modified().ok().and_then(system_time_key),
    })
}

fn extract_comic_archive_page_visual<F>(
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
                let file = File::open(archive_path).ok()?;
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

fn read_zip_entry_bytes_limited<R, F>(
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

fn read_7z_entry_bytes_limited<F>(
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

fn read_unrar_entry_bytes_limited<F>(
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

#[cfg(test)]
mod tests;
