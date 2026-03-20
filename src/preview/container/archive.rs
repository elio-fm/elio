use super::*;
use crate::{file_info, fs::natural_cmp, ui::theme};
use flate2::read::GzDecoder;
use ratatui::text::Line;
use std::{
    collections::{BTreeMap, HashMap, VecDeque, hash_map::DefaultHasher},
    env,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};
use tar::Archive as TarArchive;
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

pub(in crate::preview) fn build_archive_preview(
    path: &Path,
    type_detail: Option<&'static str>,
    comic_page_index: Option<usize>,
) -> Option<PreviewContent> {
    let format = detect_archive_format(path);
    if matches!(format, ArchiveFormat::ComicZip | ArchiveFormat::ComicRar)
        && let Some(preview) =
            build_comic_archive_preview(path, format, type_detail, comic_page_index.unwrap_or(0))
    {
        return Some(preview);
    }
    if let Some(preview) = build_zip_archive_preview(path, format, type_detail) {
        return Some(preview);
    }
    if let Some(preview) = build_tar_archive_preview(path, format, type_detail) {
        return Some(preview);
    }
    build_external_archive_preview(path, format, type_detail)
}

fn detect_archive_format(path: &Path) -> ArchiveFormat {
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

fn archive_default_label(format: ArchiveFormat) -> &'static str {
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

fn archive_format_name(format: ArchiveFormat) -> &'static str {
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

fn build_zip_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    if !matches!(format, ArchiveFormat::Zip | ArchiveFormat::ComicZip) {
        return None;
    }

    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    let total_entries = archive.len();
    let mut entries = Vec::with_capacity(total_entries.min(ARCHIVE_ENTRY_SCAN_LIMIT));
    let mut metadata = ArchiveMetadata {
        format_label: Some(archive_format_name(format).to_string()),
        physical_size: fs::metadata(path).ok().map(|metadata| metadata.len()),
        ..ArchiveMetadata::default()
    };
    let mut manifest = ZipManifestMetadata::default();

    for index in 0..total_entries.min(ARCHIVE_ENTRY_SCAN_LIMIT) {
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

    let detail = type_detail.unwrap_or(archive_default_label(format));
    let scan_truncated = total_entries > ARCHIVE_ENTRY_SCAN_LIMIT;
    let preview = render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata,
        entries: Some(entries),
        total_entries_hint: Some(total_entries),
        empty_label: archive_is_empty_label(format),
        unavailable_label: "Unable to read archive contents",
        extra_sections: zip_manifest_sections(&manifest),
        scan_truncated,
    });
    Some(preview)
}

fn build_tar_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    let (metadata, entries, total_entries, scan_truncated) =
        collect_internal_tar_listing(path, format)?;
    let detail = type_detail.unwrap_or(archive_default_label(format));

    Some(render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata,
        entries: Some(entries),
        total_entries_hint: Some(total_entries),
        empty_label: archive_is_empty_label(format),
        unavailable_label: "Unable to read archive contents",
        extra_sections: Vec::new(),
        scan_truncated,
    }))
}

fn build_comic_archive_preview(
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

fn build_external_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    // Common ZIP and TAR previews are handled internally above. This path is for
    // recovery and uncommon archive types, where 7z provides the broadest coverage
    // and bsdtar remains a final generic fallback.
    let detail = type_detail.unwrap_or(archive_default_label(format));
    if let Some(entries) = collect_preferred_archive_entries(path, format) {
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata: ArchiveMetadata {
                format_label: Some(archive_format_name(format).to_string()),
                ..ArchiveMetadata::default()
            },
            entries: Some(entries),
            total_entries_hint: None,
            empty_label: archive_is_empty_label(format),
            unavailable_label: "Unable to read archive contents",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }

    if let Some((metadata, mut entries)) = collect_archive_listing_with_7z(path) {
        if entries.is_empty()
            && let Some(entry) = synthetic_single_file_archive_entry(path, format)
        {
            entries.push(entry);
        }
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata,
            entries: Some(entries),
            total_entries_hint: None,
            empty_label: archive_is_empty_label(format),
            unavailable_label: "Unable to read archive contents",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }

    let entries = collect_archive_entries_with_bsdtar_fallback(path)?;

    Some(render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata: ArchiveMetadata {
            format_label: Some(archive_format_name(format).to_string()),
            ..ArchiveMetadata::default()
        },
        entries: Some(entries),
        total_entries_hint: None,
        empty_label: archive_is_empty_label(format),
        unavailable_label: "Unable to read archive contents",
        extra_sections: Vec::new(),
        scan_truncated: false,
    }))
}

fn collect_internal_tar_listing(
    path: &Path,
    format: ArchiveFormat,
) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>, usize, bool)> {
    match format {
        ArchiveFormat::Tar => {
            let file = File::open(path).ok()?;
            collect_tar_listing_from_reader(file, path, format)
        }
        ArchiveFormat::TarGzip => {
            let file = File::open(path).ok()?;
            collect_tar_listing_from_reader(GzDecoder::new(file), path, format)
        }
        _ => None,
    }
}

fn collect_tar_listing_from_reader<R: Read>(
    reader: R,
    path: &Path,
    format: ArchiveFormat,
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

fn collect_preferred_archive_entries(
    path: &Path,
    format: ArchiveFormat,
) -> Option<Vec<ArchiveEntry>> {
    if prefers_tar_listing(format) {
        // If internal TAR parsing fails, keep bsdtar as the only tar-family CLI fallback.
        return collect_internal_tar_listing(path, format)
            .map(|(_, entries, _, _)| entries)
            .or_else(|| collect_archive_entries_with_bsdtar(path));
    }

    None
}

fn collect_archive_entries_with_bsdtar_fallback(path: &Path) -> Option<Vec<ArchiveEntry>> {
    collect_archive_entries_with_bsdtar(path)
}

fn prefers_tar_listing(format: ArchiveFormat) -> bool {
    matches!(
        format,
        ArchiveFormat::Tar
            | ArchiveFormat::TarGzip
            | ArchiveFormat::TarXz
            | ArchiveFormat::TarBzip2
            | ArchiveFormat::TarZstd
    )
}

fn synthetic_single_file_archive_entry(path: &Path, format: ArchiveFormat) -> Option<ArchiveEntry> {
    if !matches!(
        format,
        ArchiveFormat::Gzip | ArchiveFormat::Xz | ArchiveFormat::Bzip2 | ArchiveFormat::Zstd
    ) {
        return None;
    }

    let name = path.file_stem()?.to_str()?;
    let path = normalize_archive_path(name, false)?;
    Some(ArchiveEntry {
        path,
        is_dir: false,
    })
}

fn render_archive_preview(config: ArchiveRenderConfig) -> PreviewContent {
    let palette = theme::palette();
    let mut lines = Vec::new();
    let entries = expand_archive_entries(config.entries.unwrap_or_default());
    let total_items = entries.len().max(config.total_entries_hint.unwrap_or(0));
    let folder_count = entries.iter().filter(|entry| entry.is_dir).count();
    let file_count = total_items.saturating_sub(folder_count);

    let summary = vec![
        ("Format", config.metadata.format_label),
        (
            "Entries",
            (total_items > 0).then(|| format!("{total_items} total")),
        ),
        (
            "Folders",
            (folder_count > 0).then(|| folder_count.to_string()),
        ),
        ("Files", (file_count > 0).then(|| file_count.to_string())),
        (
            "Packed",
            config.metadata.compressed_size.map(crate::app::format_size),
        ),
        (
            "Unpacked",
            config.metadata.unpacked_size.map(crate::app::format_size),
        ),
        (
            "Archive Size",
            config.metadata.physical_size.map(crate::app::format_size),
        ),
        ("Comment", config.metadata.comment),
    ];
    push_preview_section(&mut lines, "Summary", &summary, palette);

    for (title, fields) in config.extra_sections {
        push_preview_values_section(&mut lines, title, &fields, palette);
    }

    let mut rendered_items = 0usize;
    let mut tree_truncated = false;
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line("Contents", palette));

    if entries.is_empty() {
        lines.push(Line::from(if total_items == 0 {
            config.empty_label.to_string()
        } else {
            config.unavailable_label.to_string()
        }));
    } else {
        let mut root = ArchiveTreeNode::default();
        for entry in &entries {
            insert_archive_tree_entry(&mut root, entry);
        }
        let available_lines = PREVIEW_RENDER_LINE_LIMIT.saturating_sub(lines.len());
        let mut remaining = available_lines;
        if remaining == 0 {
            tree_truncated = true;
        } else {
            let children = ordered_archive_children(&root.children);
            render_archive_tree(
                &children,
                "",
                &mut remaining,
                &mut rendered_items,
                &mut lines,
                palette,
            );
            tree_truncated = rendered_items < entries.len();
        }
    }

    let mut notes = Vec::new();
    if config.scan_truncated {
        notes.push(format!(
            "scanned first {} of {} entries",
            entries.len(),
            total_items
        ));
    }
    if tree_truncated {
        notes.push(format!(
            "showing first {} of {} entries",
            rendered_items.max(entries.len().min(PREVIEW_RENDER_LINE_LIMIT)),
            total_items
        ));
    }

    let mut preview = PreviewContent::new(PreviewKind::Archive, lines)
        .with_detail(config.detail)
        .with_directory_counts(total_items, folder_count, file_count);
    if !notes.is_empty() {
        preview = preview.with_truncation(notes.join("  •  "));
    }
    preview
}

struct ArchiveRenderConfig {
    detail: String,
    metadata: ArchiveMetadata,
    entries: Option<Vec<ArchiveEntry>>,
    total_entries_hint: Option<usize>,
    empty_label: &'static str,
    unavailable_label: &'static str,
    extra_sections: Vec<(&'static str, Vec<(&'static str, String)>)>,
    scan_truncated: bool,
}

fn archive_is_empty_label(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::ComicZip => "Archive is empty",
        ArchiveFormat::ComicRar => "Archive is empty",
        ArchiveFormat::Zip => "Archive is empty",
        ArchiveFormat::SevenZip => "Archive is empty",
        ArchiveFormat::Tar
        | ArchiveFormat::TarGzip
        | ArchiveFormat::TarXz
        | ArchiveFormat::TarBzip2
        | ArchiveFormat::TarZstd
        | ArchiveFormat::Gzip
        | ArchiveFormat::Xz
        | ArchiveFormat::Bzip2
        | ArchiveFormat::Zstd
        | ArchiveFormat::Deb
        | ArchiveFormat::Rpm
        | ArchiveFormat::AppImage
        | ArchiveFormat::Unknown => "Archive is empty",
    }
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

fn archive_image_extension(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        Some("png")
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("jpg")
    } else if lower.ends_with(".gif") {
        Some("gif")
    } else if lower.ends_with(".webp") {
        Some("webp")
    } else if lower.ends_with(".svg") {
        Some("svg")
    } else {
        None
    }
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

fn system_time_key(time: SystemTime) -> Option<(u64, u32)> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
}

fn collect_archive_entries_with_bsdtar(path: &Path) -> Option<Vec<ArchiveEntry>> {
    let output = Command::new("bsdtar").arg("-tf").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output.stdout).lines(),
        false,
    ))
}

fn collect_archive_listing_with_7z(path: &Path) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>)> {
    let output = Command::new("7z")
        .arg("l")
        .arg("-slt")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_7z_listing(&String::from_utf8_lossy(&output.stdout))
}

fn parse_7z_listing(output: &str) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>)> {
    let mut metadata = ArchiveMetadata::default();
    let mut entries = Vec::new();
    let mut in_entries = false;
    let mut current = BTreeMap::<String, String>::new();

    for raw_line in output.lines() {
        let line = raw_line.trim_end();
        if line == "----------" {
            in_entries = true;
            continue;
        }

        if !in_entries {
            if let Some((key, value)) = parse_key_value_line(line) {
                match key {
                    "Type" => metadata.format_label = Some(value.to_string()),
                    "Physical Size" => metadata.physical_size = parse_u64(value),
                    "Comment" if !value.is_empty() => metadata.comment = Some(value.to_string()),
                    _ => {}
                }
            }
            continue;
        }

        if line.is_empty() {
            push_7z_entry(&mut current, &mut entries, &mut metadata);
            continue;
        }

        if let Some((key, value)) = parse_key_value_line(line) {
            current.insert(key.to_string(), value.to_string());
        }
    }
    push_7z_entry(&mut current, &mut entries, &mut metadata);

    if entries.is_empty()
        && metadata.format_label.is_none()
        && metadata.physical_size.is_none()
        && metadata.comment.is_none()
    {
        None
    } else {
        Some((metadata, entries))
    }
}

fn push_7z_entry(
    current: &mut BTreeMap<String, String>,
    entries: &mut Vec<ArchiveEntry>,
    metadata: &mut ArchiveMetadata,
) {
    if current.is_empty() {
        return;
    }

    let path = current.get("Path").cloned();
    let is_dir = current.get("Folder").is_some_and(|value| value == "+")
        || current
            .get("Attributes")
            .is_some_and(|value| value.starts_with('D'));

    if let Some(path) = path.and_then(|path| normalize_archive_path(&path, false)) {
        entries.push(ArchiveEntry { path, is_dir });
    }

    if let Some(size) = current.get("Size").and_then(|value| parse_u64(value)) {
        metadata.unpacked_size = Some(metadata.unpacked_size.unwrap_or(0).saturating_add(size));
    }
    if let Some(size) = current
        .get("Packed Size")
        .and_then(|value| parse_u64(value))
    {
        metadata.compressed_size = Some(metadata.compressed_size.unwrap_or(0).saturating_add(size));
    }
    current.clear();
}

fn parse_key_value_line(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(" = ")?;
    Some((key.trim(), value.trim()))
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

fn normalize_archive_path(item: &str, strip_version_suffix: bool) -> Option<String> {
    normalize_archive_entry(item, strip_version_suffix).map(|entry| entry.path)
}

fn parse_zip_manifest(contents: &str) -> ZipManifestMetadata {
    let mut fields = BTreeMap::<String, String>::new();
    let mut current_key: Option<String> = None;

    for line in contents.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(key) = &current_key
                && let Some(value) = fields.get_mut(key)
            {
                value.push_str(rest);
            }
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            current_key = None;
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim().to_string();
        current_key = Some(key.clone());
        fields.insert(key, value);
    }

    ZipManifestMetadata {
        title: fields
            .get("Implementation-Title")
            .cloned()
            .or_else(|| fields.get("Bundle-Name").cloned()),
        version: fields
            .get("Implementation-Version")
            .cloned()
            .or_else(|| fields.get("Bundle-Version").cloned()),
        main_class: fields.get("Main-Class").cloned(),
        created_by: fields.get("Created-By").cloned(),
        automatic_module: fields.get("Automatic-Module-Name").cloned(),
    }
}

impl ZipManifestMetadata {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.version.is_none()
            && self.main_class.is_none()
            && self.created_by.is_none()
            && self.automatic_module.is_none()
    }
}

fn zip_manifest_sections(
    manifest: &ZipManifestMetadata,
) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    if manifest.is_empty() {
        return Vec::new();
    }

    let mut fields = Vec::new();
    if let Some(value) = &manifest.title {
        fields.push(("Title", value.clone()));
    }
    if let Some(value) = &manifest.version {
        fields.push(("Version", value.clone()));
    }
    if let Some(value) = &manifest.main_class {
        fields.push(("Main-Class", value.clone()));
    }
    if let Some(value) = &manifest.automatic_module {
        fields.push(("Module", value.clone()));
    }
    if let Some(value) = &manifest.created_by {
        fields.push(("Created By", value.clone()));
    }
    vec![("Manifest", fields)]
}

#[cfg(test)]
mod tests {
    use super::*;
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
