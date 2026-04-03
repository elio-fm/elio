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
mod tests {
    use super::super::ArchiveFormat;
    use super::super::build_archive_preview;
    use super::{
        ComicArchiveBackend, ComicArchiveSignature, build_comic_archive_preview,
        parse_comic_archive_from_7z_output, parse_zip_comic_archive, sniff_comic_archive_signature,
    };
    use crate::preview::PreviewKind;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::{
        env,
        fs::{self, File},
        io::Write,
        path::PathBuf,
        process::Command,
        sync::atomic::{AtomicBool, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("elio-comic-archive-{label}-{unique}"))
    }

    #[test]
    fn sniff_comic_archive_signature_detects_common_formats() {
        let root = temp_path("signature-sniff");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let zip = root.join("issue.cbz");
        fs::write(&zip, b"PK\x03\x04demo").expect("failed to write zip signature");
        assert_eq!(
            sniff_comic_archive_signature(&zip),
            ComicArchiveSignature::Zip
        );

        let rar = root.join("issue.cbr");
        fs::write(&rar, b"Rar!\x1a\x07\x01\x00demo").expect("failed to write rar signature");
        assert_eq!(
            sniff_comic_archive_signature(&rar),
            ComicArchiveSignature::Rar
        );

        let seven_zip = root.join("issue.7z");
        fs::write(&seven_zip, [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, 0, 0])
            .expect("failed to write 7z signature");
        assert_eq!(
            sniff_comic_archive_signature(&seven_zip),
            ComicArchiveSignature::SevenZip
        );

        let unknown = root.join("issue.bin");
        fs::write(&unknown, b"not-an-archive").expect("failed to write unknown file");
        assert_eq!(
            sniff_comic_archive_signature(&unknown),
            ComicArchiveSignature::Unknown
        );

        fs::remove_dir_all(&root).expect("failed to remove temp root");
    }

    #[test]
    fn parses_comic_pages_from_7z_listing_output() {
        let output = r#"
Path = /tmp/berserk.cbz
Type = Rar
Physical Size = 1024

----------
Path = 010.jpg
Folder = -
Size = 10
Packed Size = 10

Path = 002.jpg
Folder = -
Size = 20
Packed Size = 20

Path = notes/readme.txt
Folder = -
Size = 30
Packed Size = 30

Path = 001.jpg
Folder = -
Size = 40
Packed Size = 40
"#;

        let comic = parse_comic_archive_from_7z_output(output, &|| false)
            .expect("7z output should yield comic pages");

        assert_eq!(comic.backend, ComicArchiveBackend::SevenZip);
        assert_eq!(comic.page_entries.len(), 3);
        assert_eq!(comic.page_entries[0].entry_name, "001.jpg");
        assert_eq!(comic.page_entries[1].entry_name, "002.jpg");
        assert_eq!(comic.page_entries[2].entry_name, "010.jpg");
    }

    #[test]
    fn parse_zip_comic_archive_returns_none_when_canceled() {
        let root = temp_path("zip-cancel");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let archive = root.join("issue.cbz");
        let file = File::create(&archive).expect("failed to create comic zip");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("001.jpg", options)
            .expect("failed to start first page");
        zip.write_all(b"page-one")
            .expect("failed to write first page");
        zip.start_file("002.jpg", options)
            .expect("failed to start second page");
        zip.write_all(b"page-two")
            .expect("failed to write second page");
        zip.finish().expect("failed to finish comic zip");

        let canceled = AtomicBool::new(true);
        let parsed = parse_zip_comic_archive(&archive, &|| canceled.load(Ordering::Relaxed));
        assert!(parsed.is_none(), "canceled zip parsing should stop early");

        fs::remove_dir_all(&root).expect("failed to remove temp root");
    }
    #[test]
    fn build_comic_archive_preview_falls_back_to_7z_for_mislabeled_cbz() {
        let root = temp_path("mislabeled-cbz");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let first = root.join("001.png");
        let second = root.join("010.png");
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255])));
        image
            .save_with_format(&first, ImageFormat::Png)
            .expect("failed to write first image");
        image
            .save_with_format(&second, ImageFormat::Png)
            .expect("failed to write second image");

        let archive = root.join("broken.cbz");
        let status = Command::new("7z")
            .current_dir(&root)
            .arg("a")
            .arg("-t7z")
            .arg(&archive)
            .arg("001.png")
            .arg("010.png")
            .status();
        let Ok(status) = status else {
            fs::remove_dir_all(&root).expect("failed to remove temp root");
            return;
        };
        if !status.success() {
            fs::remove_dir_all(&root).expect("failed to remove temp root");
            return;
        }

        let preview = build_comic_archive_preview(
            &archive,
            ArchiveFormat::ComicZip,
            Some("Comic ZIP archive"),
            0,
            &|| false,
        )
        .expect("mislabeled cbz should still build comic preview");

        assert_eq!(preview.kind, PreviewKind::Comic);
        assert_eq!(preview.detail.as_deref(), Some("Comic ZIP archive"));
        assert_eq!(
            preview
                .navigation_position
                .as_ref()
                .map(|position| position.count),
            Some(2)
        );
        let visual = preview
            .preview_visual
            .as_ref()
            .expect("comic preview should expose a page visual");
        let dimensions = image::ImageReader::open(&visual.path)
            .expect("extracted page should open")
            .with_guessed_format()
            .expect("page format should be detected")
            .into_dimensions()
            .expect("page dimensions should be readable");
        assert_eq!(dimensions, (1, 1));

        fs::remove_dir_all(&root).expect("failed to remove temp root");
    }

    #[test]
    fn build_archive_preview_detects_cbr_as_comic_when_7z_backend_is_needed() {
        let root = temp_path("cbr-7z-backend");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let first = root.join("001.png");
        let second = root.join("010.png");
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255])));
        image
            .save_with_format(&first, ImageFormat::Png)
            .expect("failed to write first image");
        image
            .save_with_format(&second, ImageFormat::Png)
            .expect("failed to write second image");

        let archive = root.join("issue.cbr");
        let status = Command::new("7z")
            .current_dir(&root)
            .arg("a")
            .arg("-t7z")
            .arg(&archive)
            .arg("001.png")
            .arg("010.png")
            .status();
        let Ok(status) = status else {
            fs::remove_dir_all(&root).expect("failed to remove temp root");
            return;
        };
        if !status.success() {
            fs::remove_dir_all(&root).expect("failed to remove temp root");
            return;
        }

        let preview =
            build_archive_preview(&archive, Some("Comic RAR archive"), Some(0), &|| false)
                .expect("cbr should build comic preview");

        assert_eq!(preview.kind, PreviewKind::Comic);
        assert_eq!(preview.detail.as_deref(), Some("Comic RAR archive"));
        assert_eq!(
            preview
                .navigation_position
                .as_ref()
                .map(|position| position.count),
            Some(2)
        );
        assert!(preview.preview_visual.is_some());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn build_comic_archive_preview_shows_status_note_when_cbr_cannot_be_opened() {
        // Write a file with a RAR5 magic header but invalid/truncated body.
        // All backends (ZIP, 7z, unrar) reject it, so the code must choose a
        // status note based on whether a RAR-capable extractor is installed:
        //   • no extractor  → "RAR preview requires unrar or a 7z build with RAR support"
        //   • extractor present but file unreadable → "Unable to read RAR archive …"
        // Both messages contain "RAR", which is the common denominator we assert.
        let root = temp_path("cbr-unreadable");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let archive = root.join("issue.cbr");
        let rar5_header = b"Rar!\x1a\x07\x01\x00\x00\x00\x00";
        fs::write(&archive, rar5_header).expect("failed to write fake rar header");

        let preview = build_comic_archive_preview(
            &archive,
            ArchiveFormat::ComicRar,
            Some("Comic RAR archive"),
            0,
            &|| false,
        )
        .expect("unreadable cbr should return a status preview, not None");

        assert_eq!(preview.kind, PreviewKind::Comic);
        assert_eq!(preview.detail.as_deref(), Some("Comic RAR archive"));
        assert!(
            preview.navigation_position.is_none(),
            "no pages should be navigable when no backend could open the archive"
        );
        let status = preview.status_note.as_deref().unwrap_or("");
        assert!(
            status.contains("RAR"),
            "status note should mention RAR, got: {status:?}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
