mod parse;
mod toc;

use self::{
    parse::{
        EpubManifestItem, parse_epub_package_document, parse_epub_rootfile_path,
        resolve_epub_cover_item,
    },
    toc::{EpubSection, build_epub_sections, epub_section_title_from_path},
};
use super::{
    common::{
        local_name, read_zip_entry, read_zip_entry_bytes_limited, read_zip_entry_limited,
        resolve_zip_entry_path, strip_fragment_identifier, xml_attribute_value,
    },
    metadata::{DocumentMetadata, render_document_preview, render_document_preview_lines},
};
use crate::{
    file_info::DocumentFormat,
    preview::{PreviewContent, PreviewKind, PreviewVisual, PreviewVisualKind, PreviewVisualLayout},
};
use quick_xml::{Reader, events::Event};
use ratatui::text::Line;
use std::{
    collections::{HashMap, VecDeque, hash_map::DefaultHasher},
    env,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};
use zip::ZipArchive;

const EPUB_NAV_ENTRY_LIMIT_BYTES: usize = 96 * 1024;
const EPUB_CONTENT_ENTRY_LIMIT_BYTES: usize = 192 * 1024;
const EPUB_SECTION_TEXT_LIMIT_CHARS: usize = 32 * 1024;
const EPUB_COVER_ENTRY_LIMIT_BYTES: usize = 4 * 1024 * 1024;
const EPUB_SECTION_IMAGE_ENTRY_LIMIT_BYTES: usize = 32 * 1024 * 1024;
const EPUB_PACKAGE_CACHE_LIMIT: usize = 16;
const EPUB_ASSET_CACHE_VERSION: usize = 2;

struct EpubPreviewData {
    metadata: DocumentMetadata,
    section_index: usize,
    section_count: usize,
    section_title: Option<String>,
    section_text: String,
    truncation_note: Option<String>,
    visual: Option<PreviewVisual>,
}

struct EpubSectionPreview {
    text: String,
    truncation_note: Option<String>,
    visual: Option<PreviewVisual>,
}

#[derive(Clone, Debug)]
struct CachedEpubPackage {
    metadata: DocumentMetadata,
    sections: Vec<EpubSection>,
    cover_asset: Option<EpubAssetDescriptor>,
}

#[derive(Clone, Debug)]
struct EpubAssetDescriptor {
    zip_path: String,
    extension: String,
}

#[derive(Clone)]
struct ExtractedEpubAsset {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Default)]
struct EpubPackageCache {
    packages: HashMap<EpubPackageCacheKey, Arc<CachedEpubPackage>>,
    order: VecDeque<EpubPackageCacheKey>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct EpubPackageCacheKey {
    path: PathBuf,
    size: u64,
    modified: Option<(u64, u32)>,
}

pub(super) fn build_epub_preview(path: &Path, section_index: usize) -> Option<PreviewContent> {
    let file = File::open(path).ok()?;
    let preview = match ZipArchive::new(file) {
        Ok(mut archive) => {
            render_epub_preview(extract_epub_preview_data(&mut archive, path, section_index))
        }
        Err(_) => render_document_preview(
            DocumentFormat::Epub,
            DocumentMetadata {
                variant: Some("EPUB package".to_string()),
                ..DocumentMetadata::default()
            },
        ),
    };
    Some(preview)
}

fn render_epub_preview(preview: EpubPreviewData) -> PreviewContent {
    let section_navigation_active = preview.section_count > 0;
    let lines = if preview.section_text.is_empty() {
        if preview.section_count == 0 {
            let mut lines = render_document_preview_lines(&preview.metadata);
            if lines.is_empty() {
                lines.push(Line::from("No readable content in this ebook"));
            }
            lines
        } else if preview
            .visual
            .as_ref()
            .is_some_and(|visual| visual.kind == PreviewVisualKind::PageImage)
        {
            Vec::new()
        } else {
            vec![Line::from("No readable content in this section")]
        }
    } else {
        super::super::render_reflowed_text_preview(&preview.section_text)
    };
    let detail = if section_navigation_active {
        DocumentFormat::Epub.detail_label().to_string()
    } else {
        preview
            .metadata
            .title
            .clone()
            .unwrap_or_else(|| DocumentFormat::Epub.detail_label().to_string())
    };
    let status_note = (!section_navigation_active).then(|| {
        let mut parts = vec![DocumentFormat::Epub.detail_label().to_string()];
        if let Some(author) = preview.metadata.author.as_deref() {
            parts.push(author.to_string());
        }
        parts.join("  •  ")
    });
    let mut content = PreviewContent::new(PreviewKind::Document, lines).with_detail(detail);
    if let Some(status_note) = status_note {
        content = content.with_status_note(status_note);
    }
    if preview.section_count > 0 {
        content = content.with_ebook_section(
            preview.section_index,
            preview.section_count,
            preview.section_title,
        );
    }
    if let Some(visual) = preview.visual {
        content = content.with_preview_visual(visual);
    }
    if let Some(note) = preview.truncation_note {
        content = content.with_truncation(note);
    }
    content
}

fn extract_epub_preview_data<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &Path,
    requested_section_index: usize,
) -> EpubPreviewData {
    let mut preview = EpubPreviewData {
        metadata: DocumentMetadata {
            variant: Some("EPUB package".to_string()),
            ..DocumentMetadata::default()
        },
        section_index: 0,
        section_count: 0,
        section_title: None,
        section_text: String::new(),
        truncation_note: None,
        visual: None,
    };
    let Some(package) = load_epub_package(archive, path) else {
        return preview;
    };
    let (section_index, section_count) = match package.sections.len() {
        0 => (0, 0),
        count => (requested_section_index.min(count.saturating_sub(1)), count),
    };

    preview.visual = package.cover_asset.as_ref().and_then(|asset| {
        extract_epub_asset_descriptor(path, archive, asset, EPUB_COVER_ENTRY_LIMIT_BYTES).map(
            |asset| {
                build_preview_visual(PreviewVisualKind::Cover, PreviewVisualLayout::Inline, asset)
            },
        )
    });
    preview.metadata = package.metadata.clone();
    preview.section_index = section_index;
    preview.section_count = section_count;

    if let Some(section) = package.sections.get(section_index) {
        let section_preview = extract_epub_section_preview(path, archive, &section.path);
        preview.section_text = section_preview.text;
        preview.section_title = section
            .title
            .clone()
            .or_else(|| epub_section_title_from_path(&section.path));
        preview.truncation_note = section_preview.truncation_note;
        if preview.section_text.is_empty()
            && let Some(visual) = section_preview.visual
        {
            preview.visual = Some(visual);
        }
    }
    preview
}

fn load_epub_package<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &Path,
) -> Option<Arc<CachedEpubPackage>> {
    let cache_key = epub_package_cache_key(path);
    if let Some(cache_key) = cache_key.as_ref()
        && let Some(cached) = cached_epub_package(cache_key)
    {
        return Some(cached);
    }

    let container_xml = read_zip_entry(archive, "META-INF/container.xml")?;
    let package_path = parse_epub_rootfile_path(&container_xml)?;
    let package_xml = read_zip_entry(archive, &package_path)?;
    #[cfg(test)]
    record_epub_package_parse(path);
    let package = parse_epub_package_document(&package_xml);
    let sections = build_epub_sections(archive, &package, &package_path);
    let cover_asset = resolve_epub_cover_item(&package)
        .and_then(|item| build_epub_asset_descriptor(&package_path, item));
    let cached = Arc::new(CachedEpubPackage {
        metadata: package.metadata,
        sections,
        cover_asset,
    });
    if let Some(cache_key) = cache_key {
        cache_epub_package(cache_key, Arc::clone(&cached));
    }
    Some(cached)
}

fn extract_epub_section_preview<R: Read + std::io::Seek>(
    source_path: &Path,
    archive: &mut ZipArchive<R>,
    section_path: &str,
) -> EpubSectionPreview {
    let Some(xml) = read_zip_entry_limited(archive, section_path, EPUB_CONTENT_ENTRY_LIMIT_BYTES)
    else {
        return EpubSectionPreview {
            text: String::new(),
            truncation_note: None,
            visual: None,
        };
    };
    let blocks = extract_xhtml_text_blocks(&xml);
    let visual = extract_xhtml_image_href(&xml).and_then(|href| {
        let asset_path = resolve_zip_entry_path(section_path, &href);
        extract_epub_asset(
            source_path,
            archive,
            &asset_path,
            EPUB_SECTION_IMAGE_ENTRY_LIMIT_BYTES,
        )
        .map(|asset| {
            build_preview_visual(
                PreviewVisualKind::PageImage,
                PreviewVisualLayout::FullHeight,
                asset,
            )
        })
    });
    if blocks.is_empty() {
        return EpubSectionPreview {
            text: String::new(),
            truncation_note: None,
            visual,
        };
    }

    let mut text = String::new();
    let mut truncated = false;
    for block in blocks {
        let remaining = EPUB_SECTION_TEXT_LIMIT_CHARS.saturating_sub(text.chars().count());
        if remaining == 0 {
            truncated = true;
            break;
        }

        let Some((clipped, was_truncated)) = clip_epub_block(&block, remaining) else {
            continue;
        };
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&clipped);
        if was_truncated {
            truncated = true;
            break;
        }
    }

    EpubSectionPreview {
        text,
        truncation_note: truncated.then(epub_section_truncation_note),
        visual,
    }
}

fn extract_epub_asset<R: Read + std::io::Seek>(
    source_path: &Path,
    archive: &mut ZipArchive<R>,
    asset_path: &str,
    limit_bytes: usize,
) -> Option<ExtractedEpubAsset> {
    let extension = epub_asset_extension(asset_path)?;
    let descriptor = EpubAssetDescriptor {
        zip_path: asset_path.to_string(),
        extension: extension.to_string(),
    };
    extract_epub_asset_descriptor(source_path, archive, &descriptor, limit_bytes)
}

fn extract_epub_asset_descriptor<R: Read + std::io::Seek>(
    source_path: &Path,
    archive: &mut ZipArchive<R>,
    asset: &EpubAssetDescriptor,
    limit_bytes: usize,
) -> Option<ExtractedEpubAsset> {
    let cache_path = epub_asset_cache_path(source_path, &asset.zip_path, &asset.extension)?;
    if cache_path.exists() {
        return extracted_epub_asset_from_path(cache_path);
    }

    let bytes = read_zip_entry_bytes_limited(archive, &asset.zip_path, limit_bytes)?;
    write_bytes_atomically(&cache_path, &bytes)?;
    extracted_epub_asset_from_path(cache_path)
}

fn extracted_epub_asset_from_path(path: PathBuf) -> Option<ExtractedEpubAsset> {
    let metadata = fs::metadata(&path).ok()?;
    Some(ExtractedEpubAsset {
        path,
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn write_bytes_atomically(path: &Path, bytes: &[u8]) -> Option<()> {
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;

    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_name = format!(
        ".{}.tmp-{}-{}",
        path.file_name()?.to_string_lossy(),
        std::process::id(),
        unique
    );
    let temp_path = parent.join(temp_name);

    let mut file = File::create(&temp_path).ok()?;
    file.write_all(bytes).ok()?;
    file.sync_all().ok()?;

    match fs::rename(&temp_path, path) {
        Ok(()) => Some(()),
        Err(_) if path.exists() => {
            let _ = fs::remove_file(&temp_path);
            Some(())
        }
        Err(_) => {
            let _ = fs::remove_file(&temp_path);
            None
        }
    }
}

fn build_epub_asset_descriptor(
    package_path: &str,
    item: &EpubManifestItem,
) -> Option<EpubAssetDescriptor> {
    Some(EpubAssetDescriptor {
        zip_path: resolve_zip_entry_path(package_path, &item.href),
        extension: epub_cover_extension(item)?.to_string(),
    })
}

fn build_preview_visual(
    kind: PreviewVisualKind,
    layout: PreviewVisualLayout,
    asset: ExtractedEpubAsset,
) -> PreviewVisual {
    PreviewVisual {
        kind,
        layout,
        path: asset.path,
        size: asset.size,
        modified: asset.modified,
    }
}

fn epub_cover_extension(item: &EpubManifestItem) -> Option<&'static str> {
    match item.media_type.as_deref() {
        Some("image/png") => Some("png"),
        Some("image/jpeg") => Some("jpg"),
        Some("image/gif") => Some("gif"),
        Some("image/webp") => Some("webp"),
        Some("image/svg+xml") => Some("svg"),
        _ => {
            let href = strip_fragment_identifier(&item.href).to_ascii_lowercase();
            if href.ends_with(".png") {
                Some("png")
            } else if href.ends_with(".jpg") || href.ends_with(".jpeg") {
                Some("jpg")
            } else if href.ends_with(".gif") {
                Some("gif")
            } else if href.ends_with(".webp") {
                Some("webp")
            } else if href.ends_with(".svg") {
                Some("svg")
            } else {
                None
            }
        }
    }
}

fn epub_asset_extension(asset_path: &str) -> Option<&str> {
    Path::new(strip_fragment_identifier(asset_path))
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            if extension.eq_ignore_ascii_case("jpeg") {
                "jpg"
            } else {
                extension
            }
        })
}

fn epub_asset_cache_path(source_path: &Path, asset_path: &str, extension: &str) -> Option<PathBuf> {
    let metadata = fs::metadata(source_path).ok();
    let modified = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_key);
    let mut hasher = DefaultHasher::new();
    EPUB_ASSET_CACHE_VERSION.hash(&mut hasher);
    source_path.hash(&mut hasher);
    asset_path.hash(&mut hasher);
    metadata
        .as_ref()
        .map(|metadata| metadata.len())
        .hash(&mut hasher);
    modified.hash(&mut hasher);
    let cache_dir = env::temp_dir().join(format!("elio-epub-asset-v{EPUB_ASSET_CACHE_VERSION}"));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!("{:016x}.{extension}", hasher.finish())))
}

fn system_time_key(time: SystemTime) -> Option<(u64, u32)> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
}

fn epub_package_cache() -> &'static Mutex<EpubPackageCache> {
    static CACHE: OnceLock<Mutex<EpubPackageCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(EpubPackageCache::default()))
}

fn epub_package_cache_key(path: &Path) -> Option<EpubPackageCacheKey> {
    let metadata = fs::metadata(path).ok()?;
    Some(EpubPackageCacheKey {
        path: path.to_path_buf(),
        size: metadata.len(),
        modified: metadata.modified().ok().and_then(system_time_key),
    })
}

fn cached_epub_package(key: &EpubPackageCacheKey) -> Option<Arc<CachedEpubPackage>> {
    let mut cache = epub_package_cache()
        .lock()
        .expect("epub package cache lock");
    let package = cache.packages.get(key).cloned();
    if package.is_some() {
        cache.order.retain(|cached| cached != key);
        cache.order.push_back(key.clone());
    }
    package
}

fn cache_epub_package(key: EpubPackageCacheKey, package: Arc<CachedEpubPackage>) {
    let mut cache = epub_package_cache()
        .lock()
        .expect("epub package cache lock");
    cache.packages.insert(key.clone(), package);
    cache.order.retain(|cached| cached != &key);
    cache.order.push_back(key);
    while cache.order.len() > EPUB_PACKAGE_CACHE_LIMIT {
        if let Some(stale_key) = cache.order.pop_front() {
            cache.packages.remove(&stale_key);
        }
    }
}

#[cfg(test)]
fn epub_package_parse_counts() -> &'static Mutex<HashMap<PathBuf, usize>> {
    static COUNTS: OnceLock<Mutex<HashMap<PathBuf, usize>>> = OnceLock::new();
    COUNTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn record_epub_package_parse(path: &Path) {
    let mut counts = epub_package_parse_counts()
        .lock()
        .expect("epub package parse count lock");
    *counts.entry(path.to_path_buf()).or_insert(0) += 1;
}

#[cfg(test)]
pub(super) fn reset_epub_package_parse_count(path: &Path) {
    epub_package_parse_counts()
        .lock()
        .expect("epub package parse count lock")
        .remove(path);
}

#[cfg(test)]
pub(super) fn epub_package_parse_count(path: &Path) -> usize {
    epub_package_parse_counts()
        .lock()
        .expect("epub package parse count lock")
        .get(path)
        .copied()
        .unwrap_or(0)
}

#[cfg(test)]
pub(super) fn clear_epub_package_cache() {
    let mut cache = epub_package_cache()
        .lock()
        .expect("epub package cache lock");
    cache.packages.clear();
    cache.order.clear();
}

fn extract_xhtml_text_blocks(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut skip_depth = 0usize;
    let mut body_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "body" {
                    body_depth += 1;
                    continue;
                }
                if epub_skip_tag(&tag) {
                    skip_depth += 1;
                    continue;
                }
                if body_depth > 0 && skip_depth == 0 && epub_block_tag(&tag) {
                    flush_epub_text_block(&mut blocks, &mut current);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                if body_depth > 0 && skip_depth == 0 && (epub_block_tag(&tag) || tag == "br") {
                    flush_epub_text_block(&mut blocks, &mut current);
                }
            }
            Ok(Event::Text(text)) => {
                if body_depth == 0 || skip_depth > 0 {
                    continue;
                }
                if let Ok(value) = text.decode() {
                    append_epub_text_fragment(&mut current, value.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if body_depth == 0 || skip_depth > 0 {
                    continue;
                }
                if let Ok(value) = text.decode() {
                    append_epub_text_fragment(&mut current, value.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "body" {
                    flush_epub_text_block(&mut blocks, &mut current);
                    body_depth = body_depth.saturating_sub(1);
                    continue;
                }
                if epub_skip_tag(&tag) && skip_depth > 0 {
                    skip_depth -= 1;
                    continue;
                }
                if body_depth > 0 && skip_depth == 0 && epub_block_tag(&tag) {
                    flush_epub_text_block(&mut blocks, &mut current);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    flush_epub_text_block(&mut blocks, &mut current);
    blocks
}

fn extract_xhtml_image_href(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut body_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "body" {
                    body_depth += 1;
                    continue;
                }
                if body_depth == 0 {
                    continue;
                }
                if tag == "img" {
                    if let Some(src) = xml_attribute_value(&event, reader.decoder(), "src") {
                        return Some(src);
                    }
                } else if tag == "image" {
                    if let Some(href) = xml_attribute_value(&event, reader.decoder(), "href") {
                        return Some(href);
                    }
                } else if tag == "object"
                    && let Some(data) = xml_attribute_value(&event, reader.decoder(), "data")
                {
                    return Some(data);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                if body_depth == 0 {
                    continue;
                }
                if tag == "img" {
                    if let Some(src) = xml_attribute_value(&event, reader.decoder(), "src") {
                        return Some(src);
                    }
                } else if tag == "image" {
                    if let Some(href) = xml_attribute_value(&event, reader.decoder(), "href") {
                        return Some(href);
                    }
                } else if tag == "object"
                    && let Some(data) = xml_attribute_value(&event, reader.decoder(), "data")
                {
                    return Some(data);
                }
            }
            Ok(Event::End(event)) => {
                if local_name(event.name().as_ref()) == "body" {
                    body_depth = body_depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    None
}

fn epub_skip_tag(tag: &str) -> bool {
    matches!(tag, "head" | "script" | "style" | "svg" | "math")
}

fn epub_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "caption"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "figcaption"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "li"
            | "p"
            | "pre"
            | "section"
            | "td"
            | "th"
            | "tr"
    )
}

fn append_epub_text_fragment(target: &mut String, fragment: &str) {
    let normalized = fragment.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return;
    }
    if !target.is_empty() && !target.chars().last().is_some_and(char::is_whitespace) {
        target.push(' ');
    }
    target.push_str(&normalized);
}

fn flush_epub_text_block(blocks: &mut Vec<String>, current: &mut String) {
    let text = current.trim();
    if !text.is_empty() {
        blocks.push(text.to_string());
    }
    current.clear();
}

fn clip_epub_block(block: &str, remaining: usize) -> Option<(String, bool)> {
    if remaining == 0 {
        return None;
    }
    let char_count = block.chars().count();
    if char_count <= remaining {
        return Some((block.to_string(), false));
    }
    let clipped = block
        .chars()
        .take(remaining.saturating_sub(1))
        .collect::<String>();
    let clipped = clipped.trim_end();
    (!clipped.is_empty()).then(|| (format!("{clipped}…"), true))
}

fn epub_section_truncation_note() -> String {
    format!(
        "section excerpt limited to {} KiB",
        EPUB_SECTION_TEXT_LIMIT_CHARS / 1024
    )
}
