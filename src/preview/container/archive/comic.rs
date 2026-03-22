use super::common::{
    archive_image_extension, normalize_archive_path, parse_key_value_line, system_time_key,
};
use super::format::archive_default_label;
use super::*;
use crate::fs::natural_cmp;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComicArchiveBackend {
    Zip,
    SevenZip,
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

pub(super) fn build_comic_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
    page_index: usize,
) -> Option<PreviewContent> {
    let comic = load_comic_archive(path)?;
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

    if let Some(visual) =
        extract_comic_archive_page_visual(path, &comic, &comic.page_entries[current_index])
    {
        preview = preview.with_preview_visual(visual);
    } else {
        preview = preview.with_status_note("Unable to extract selected page");
    }

    Some(preview)
}

fn load_comic_archive(path: &Path) -> Option<Arc<CachedComicArchive>> {
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

    let parsed = Arc::new(parse_comic_archive(path)?);
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

fn parse_comic_archive(path: &Path) -> Option<CachedComicArchive> {
    parse_zip_comic_archive(path).or_else(|| parse_comic_archive_with_7z(path))
}

fn parse_zip_comic_archive(path: &Path) -> Option<CachedComicArchive> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    let mut page_entries = Vec::new();

    for index in 0..archive.len() {
        let entry = archive.by_index(index).ok()?;
        if entry.is_dir() {
            continue;
        }

        let name = entry.name().to_string();
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

    page_entries.sort_by(|left, right| natural_cmp(&left.sort_key, &right.sort_key));

    Some(CachedComicArchive {
        backend: ComicArchiveBackend::Zip,
        page_entries,
    })
}

fn parse_comic_archive_with_7z(path: &Path) -> Option<CachedComicArchive> {
    let output = Command::new("7z")
        .arg("l")
        .arg("-slt")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    parse_comic_archive_from_7z_output(&String::from_utf8_lossy(&output.stdout))
}

fn parse_comic_archive_from_7z_output(output: &str) -> Option<CachedComicArchive> {
    let mut page_entries = Vec::new();
    let mut in_entries = false;
    let mut current = BTreeMap::<String, String>::new();

    for raw_line in output.lines() {
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

    if page_entries.is_empty() {
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

fn extract_comic_archive_page_visual(
    archive_path: &Path,
    comic: &CachedComicArchive,
    page: &ComicArchivePage,
) -> Option<PreviewVisual> {
    let cache_path = archive_asset_cache_path(archive_path, &page.entry_name, &page.extension)?;
    if !cache_path.exists() {
        let bytes = match comic.backend {
            ComicArchiveBackend::Zip => {
                let file = File::open(archive_path).ok()?;
                let mut archive = ZipArchive::new(file).ok()?;
                read_zip_entry_bytes_limited(
                    &mut archive,
                    &page.entry_name,
                    COMIC_ARCHIVE_IMAGE_ENTRY_LIMIT_BYTES,
                )?
            }
            ComicArchiveBackend::SevenZip => read_7z_entry_bytes_limited(
                archive_path,
                &page.entry_name,
                COMIC_ARCHIVE_IMAGE_ENTRY_LIMIT_BYTES,
            )?,
        };
        fs::write(&cache_path, bytes).ok()?;
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

fn read_zip_entry_bytes_limited<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit_bytes: usize,
) -> Option<Vec<u8>> {
    let entry = archive.by_name(name).ok()?;
    let limit = (entry.size() as usize).min(limit_bytes);
    let mut bytes = Vec::with_capacity(limit);
    entry
        .take(limit_bytes as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

fn read_7z_entry_bytes_limited(
    archive_path: &Path,
    entry_name: &str,
    limit_bytes: usize,
) -> Option<Vec<u8>> {
    let output = Command::new("7z")
        .arg("x")
        .arg("-so")
        .arg(archive_path)
        .arg(entry_name)
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.is_empty() || output.stdout.len() > limit_bytes {
        return None;
    }
    Some(output.stdout)
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
        ComicArchiveBackend, build_comic_archive_preview, parse_comic_archive_from_7z_output,
    };
    use crate::preview::PreviewKind;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::{
        env, fs,
        path::PathBuf,
        process::Command,
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

        let comic =
            parse_comic_archive_from_7z_output(output).expect("7z output should yield comic pages");

        assert_eq!(comic.backend, ComicArchiveBackend::SevenZip);
        assert_eq!(comic.page_entries.len(), 3);
        assert_eq!(comic.page_entries[0].entry_name, "001.jpg");
        assert_eq!(comic.page_entries[1].entry_name, "002.jpg");
        assert_eq!(comic.page_entries[2].entry_name, "010.jpg");
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

        let preview = build_archive_preview(&archive, Some("Comic RAR archive"), Some(0))
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
}
