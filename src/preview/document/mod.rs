mod common;
mod metadata;

use self::{
    common::{
        DOCUMENT_XML_ENTRY_LIMIT_BYTES, extract_zip_document_metadata, first_present_string,
        format_unix_utc, local_name, parse_xml_text_fields, present_count, present_str,
        present_string, push_count_stat, push_metadata_field, read_zip_entry,
        read_zip_entry_bytes_limited, read_zip_entry_limited, resolve_zip_entry_path,
        strip_fragment_identifier, xml_attribute_value,
    },
    metadata::{DocumentMetadata, render_document_preview, render_document_preview_lines},
};
use super::{PreviewContent, PreviewKind, PreviewVisual, PreviewVisualKind, PreviewVisualLayout};
use crate::file_info::DocumentFormat;
use quick_xml::{Reader, events::Event};
use ratatui::text::Line;
use std::{
    collections::{BTreeMap, HashMap, VecDeque, hash_map::DefaultHasher},
    env,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
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
const DOC_SUMMARY_INFORMATION_STREAM: &str = "/\u{5}SummaryInformation";
const DOC_PROPERTY_TITLE: u32 = 2;
const DOC_PROPERTY_SUBJECT: u32 = 3;
const DOC_PROPERTY_AUTHOR: u32 = 4;
const DOC_PROPERTY_LAST_SAVED_BY: u32 = 8;
const DOC_PROPERTY_CREATED: u32 = 12;
const DOC_PROPERTY_MODIFIED: u32 = 13;
const DOC_PROPERTY_PAGE_COUNT: u32 = 14;
const DOC_PROPERTY_WORD_COUNT: u32 = 15;
const DOC_PROPERTY_CHAR_COUNT: u32 = 16;
const DOC_PROPERTY_APPLICATION: u32 = 18;
const VT_I4: u16 = 0x0003;
const VT_LPSTR: u16 = 0x001E;
const VT_LPWSTR: u16 = 0x001F;
const VT_FILETIME: u16 = 0x0040;
const VT_UI4: u16 = 0x0013;
const WINDOWS_TICKS_PER_SECOND: u64 = 10_000_000;
const WINDOWS_TO_UNIX_EPOCH_SECONDS: u64 = 11_644_473_600;

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

struct EpubPackageDocument {
    metadata: DocumentMetadata,
    manifest: BTreeMap<String, EpubManifestItem>,
    spine: Vec<String>,
    nav_path: Option<String>,
    ncx_path: Option<String>,
    toc_id: Option<String>,
    cover_id: Option<String>,
}

impl EpubPackageDocument {
    fn new() -> Self {
        Self {
            metadata: DocumentMetadata {
                variant: Some("EPUB package".to_string()),
                ..DocumentMetadata::default()
            },
            manifest: BTreeMap::new(),
            spine: Vec::new(),
            nav_path: None,
            ncx_path: None,
            toc_id: None,
            cover_id: None,
        }
    }
}

#[derive(Clone)]
struct EpubManifestItem {
    href: String,
    media_type: Option<String>,
    properties: Vec<String>,
}

#[derive(Clone)]
struct EpubNavPoint {
    href: Option<String>,
    label: String,
}

#[derive(Clone, Debug)]
struct EpubSection {
    path: String,
    title: Option<String>,
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

enum DocPropertyValue {
    Count(u64),
    Text(String),
    Timestamp(String),
}

pub(super) fn build_document_preview(
    path: &Path,
    format: DocumentFormat,
    epub_section_index: Option<usize>,
) -> Option<PreviewContent> {
    let metadata = match format {
        DocumentFormat::Doc => extract_doc_metadata(path),
        DocumentFormat::Docx | DocumentFormat::Docm => {
            extract_zip_document_metadata(path, |archive| extract_ooxml_metadata(archive, format))
        }
        DocumentFormat::Odt | DocumentFormat::Ods | DocumentFormat::Odp => {
            extract_zip_document_metadata(path, |archive| {
                extract_open_document_metadata(archive, format)
            })
        }
        DocumentFormat::Pptx | DocumentFormat::Pptm => {
            extract_zip_document_metadata(path, |archive| extract_ooxml_metadata(archive, format))
        }
        DocumentFormat::Xlsx | DocumentFormat::Xlsm => {
            extract_zip_document_metadata(path, |archive| extract_ooxml_metadata(archive, format))
        }
        DocumentFormat::Pages => extract_zip_document_metadata(path, extract_pages_metadata),
        DocumentFormat::Epub => return build_epub_preview(path, epub_section_index.unwrap_or(0)),
        DocumentFormat::Pdf => extract_pdf_metadata(path),
    }?;

    Some(render_document_preview(format, metadata))
}

fn extract_doc_metadata(path: &Path) -> Option<DocumentMetadata> {
    File::open(path).ok()?;

    let mut metadata = DocumentMetadata {
        variant: Some("Legacy binary document".to_string()),
        ..DocumentMetadata::default()
    };
    let mut compound = match cfb::open(path) {
        Ok(compound) => compound,
        Err(_) => return Some(metadata),
    };
    let stream = match compound.open_stream(DOC_SUMMARY_INFORMATION_STREAM) {
        Ok(stream) => stream,
        Err(_) => return Some(metadata),
    };
    let mut bytes = Vec::with_capacity(DOCUMENT_XML_ENTRY_LIMIT_BYTES);
    stream
        .take(DOCUMENT_XML_ENTRY_LIMIT_BYTES as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    let properties = parse_doc_property_set(&bytes);

    metadata.title = doc_property_text(&properties, DOC_PROPERTY_TITLE);
    metadata.subject = doc_property_text(&properties, DOC_PROPERTY_SUBJECT);
    metadata.author = doc_property_text(&properties, DOC_PROPERTY_AUTHOR);
    metadata.modified_by = doc_property_text(&properties, DOC_PROPERTY_LAST_SAVED_BY);
    metadata.application = doc_property_text(&properties, DOC_PROPERTY_APPLICATION);
    metadata.created = doc_property_time(&properties, DOC_PROPERTY_CREATED);
    metadata.modified = doc_property_time(&properties, DOC_PROPERTY_MODIFIED);
    push_count_stat(
        &mut metadata,
        "Pages",
        doc_property_count(&properties, DOC_PROPERTY_PAGE_COUNT),
    );
    push_count_stat(
        &mut metadata,
        "Words",
        doc_property_count(&properties, DOC_PROPERTY_WORD_COUNT),
    );
    push_count_stat(
        &mut metadata,
        "Characters",
        doc_property_count(&properties, DOC_PROPERTY_CHAR_COUNT),
    );

    Some(metadata)
}

fn build_epub_preview(path: &Path, section_index: usize) -> Option<PreviewContent> {
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
        super::render_reflowed_text_preview(&preview.section_text)
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

fn extract_ooxml_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    format: DocumentFormat,
) -> DocumentMetadata {
    let mut metadata = DocumentMetadata::default();

    let core = read_zip_entry(archive, "docProps/core.xml")
        .map(|xml| parse_xml_text_fields(&xml))
        .unwrap_or_default();
    let app = read_zip_entry(archive, "docProps/app.xml")
        .map(|xml| parse_xml_text_fields(&xml))
        .unwrap_or_default();

    metadata.title = present_string(core.get("title"), "Title");
    metadata.subject = present_string(core.get("subject"), "Subject");
    metadata.author = present_string(core.get("creator"), "Author");
    metadata.modified_by = present_string(core.get("lastModifiedBy"), "Modified By");
    metadata.created = present_string(core.get("created"), "Created");
    metadata.modified = present_string(core.get("modified"), "Modified");
    metadata.application = present_string(app.get("Application"), "Application");
    if let Some(company) = present_string(app.get("Company"), "Company") {
        metadata.metadata.push(("Company".to_string(), company));
    }

    match format {
        DocumentFormat::Docx | DocumentFormat::Docm => {
            push_count_stat(&mut metadata, "Pages", present_count(app.get("Pages")));
            push_count_stat(&mut metadata, "Words", present_count(app.get("Words")));
            push_count_stat(
                &mut metadata,
                "Characters",
                present_count(app.get("Characters")),
            );
        }
        DocumentFormat::Pptx | DocumentFormat::Pptm => {
            push_count_stat(&mut metadata, "Slides", present_count(app.get("Slides")));
            push_count_stat(&mut metadata, "Notes", present_count(app.get("Notes")));
            push_count_stat(
                &mut metadata,
                "Hidden Slides",
                present_count(app.get("HiddenSlides")),
            );
            push_count_stat(
                &mut metadata,
                "Media Clips",
                present_count(app.get("MMClips")),
            );
        }
        DocumentFormat::Xlsx | DocumentFormat::Xlsm => {
            if let Some(manager) = present_string(app.get("Manager"), "Manager") {
                metadata.metadata.push(("Manager".to_string(), manager));
            }
        }
        _ => {}
    }

    metadata
}

fn extract_open_document_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    format: DocumentFormat,
) -> DocumentMetadata {
    let mut metadata = DocumentMetadata::default();
    let Some(xml) = read_zip_entry(archive, "meta.xml") else {
        return metadata;
    };

    let fields = parse_xml_text_fields(&xml);
    metadata.title = present_string(fields.get("title"), "Title");
    metadata.subject = present_string(fields.get("subject"), "Subject");
    metadata.author = present_string(
        fields.get("initial-creator").or(fields.get("creator")),
        "Author",
    );
    metadata.created = present_string(fields.get("creation-date"), "Created");
    metadata.modified = present_string(fields.get("date"), "Modified");
    metadata.application = present_string(fields.get("generator"), "Application");

    match format {
        DocumentFormat::Odt => {
            push_count_stat(
                &mut metadata,
                "Pages",
                present_count(fields.get("page-count")),
            );
            push_count_stat(
                &mut metadata,
                "Words",
                present_count(fields.get("word-count")),
            );
            push_count_stat(
                &mut metadata,
                "Characters",
                present_count(fields.get("character-count")),
            );
        }
        DocumentFormat::Ods => {
            push_count_stat(
                &mut metadata,
                "Tables",
                present_count(fields.get("table-count")),
            );
            push_count_stat(
                &mut metadata,
                "Cells",
                present_count(fields.get("cell-count")),
            );
            push_count_stat(
                &mut metadata,
                "Objects",
                present_count(fields.get("object-count")),
            );
        }
        DocumentFormat::Odp => {
            push_count_stat(
                &mut metadata,
                "Slides",
                present_count(fields.get("page-count")),
            );
            push_count_stat(
                &mut metadata,
                "Objects",
                present_count(fields.get("object-count")),
            );
        }
        _ => {}
    }

    metadata
}

fn extract_pages_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> DocumentMetadata {
    let mut metadata = DocumentMetadata {
        application: Some("Apple Pages".to_string()),
        variant: detect_pages_variant(archive),
        ..DocumentMetadata::default()
    };

    let properties = [
        "Metadata/Properties.plist",
        "metadata.plist",
        "QuickLook/Metadata.plist",
    ]
    .iter()
    .find_map(|name| read_zip_entry(archive, name).and_then(|xml| parse_plist_dict(&xml)));

    if let Some(fields) = properties {
        metadata.title = first_present_string(
            &fields,
            &["document-title", "kMDItemTitle", "title", "Title"],
            "Title",
        );
        metadata.subject = first_present_string(
            &fields,
            &["subject", "kMDItemDescription", "abstract"],
            "Subject",
        );
        metadata.author = first_present_string(
            &fields,
            &["author", "authors", "kMDItemAuthors", "kMDItemAuthor"],
            "Author",
        );
        metadata.created = first_present_string(
            &fields,
            &["creationDate", "created", "kMDItemContentCreationDate"],
            "Created",
        );
        metadata.modified = first_present_string(
            &fields,
            &[
                "modificationDate",
                "modified",
                "lastOpenedDate",
                "kMDItemContentModificationDate",
            ],
            "Modified",
        );
    }

    metadata
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
    let nav_points = extract_epub_table_of_contents(archive, &package, &package_path);
    let sections = build_epub_sections(&package, &package_path, &nav_points);
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

fn extract_pdf_metadata(path: &Path) -> Option<DocumentMetadata> {
    let mut bytes = Vec::with_capacity(256);
    File::open(path)
        .ok()?
        .take(256)
        .read_to_end(&mut bytes)
        .ok()?;

    let mut metadata = DocumentMetadata::default();
    if let Some(version) = parse_pdf_version(&bytes) {
        metadata.variant = Some(format!("PDF {version}"));
        metadata
            .metadata
            .push(("PDF Version".to_string(), version.to_string()));
    }

    let output = Command::new("pdfinfo").arg(path).output().ok();
    let Some(output) = output.filter(|output| output.status.success()) else {
        return Some(metadata);
    };
    let fields = parse_pdfinfo_fields(&String::from_utf8_lossy(&output.stdout));
    metadata.title = fields
        .get("Title")
        .and_then(|value| present_str(value, "Title"));
    metadata.subject = fields
        .get("Subject")
        .and_then(|value| present_str(value, "Subject"));
    metadata.author = fields
        .get("Author")
        .and_then(|value| present_str(value, "Author"));
    metadata.application = fields
        .get("Creator")
        .and_then(|value| present_str(value, "Application"));
    metadata.created = fields
        .get("CreationDate")
        .and_then(|value| present_str(value, "Created"));
    metadata.modified = fields
        .get("ModDate")
        .and_then(|value| present_str(value, "Modified"));

    push_count_stat(
        &mut metadata,
        "Pages",
        fields
            .get("Pages")
            .and_then(|value| value.trim().parse().ok()),
    );
    push_metadata_field(
        &mut metadata,
        "Producer",
        fields
            .get("Producer")
            .and_then(|value| present_str(value, "Producer")),
    );
    push_metadata_field(
        &mut metadata,
        "Page Size",
        fields
            .get("Page size")
            .and_then(|value| present_str(value, "Page size")),
    );
    push_metadata_field(
        &mut metadata,
        "Tagged",
        fields
            .get("Tagged")
            .and_then(|value| present_str(value, "Tagged")),
    );
    push_metadata_field(
        &mut metadata,
        "Encrypted",
        fields
            .get("Encrypted")
            .and_then(|value| present_str(value, "Encrypted")),
    );
    push_metadata_field(
        &mut metadata,
        "Optimized",
        fields
            .get("Optimized")
            .and_then(|value| present_str(value, "Optimized")),
    );

    Some(metadata)
}

fn detect_pages_variant<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> Option<String> {
    let mut saw_iwa = false;
    let mut saw_legacy_index = false;
    for index in 0..archive.len() {
        let Ok(entry) = archive.by_index(index) else {
            continue;
        };
        let name = entry.name().to_ascii_lowercase();
        saw_iwa |= name.ends_with(".iwa");
        saw_legacy_index |= name.ends_with("index.xml") || name.ends_with("index.xml.gz");
    }

    if saw_iwa {
        Some("iWork package".to_string())
    } else if saw_legacy_index {
        Some("Pages '09 package".to_string())
    } else {
        Some("Pages package".to_string())
    }
}

fn parse_epub_rootfile_path(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event() {
            Ok(Event::Empty(event)) | Ok(Event::Start(event)) => {
                if local_name(event.name().as_ref()) != "rootfile" {
                    continue;
                }
                for attribute in event.attributes().flatten() {
                    if local_name(attribute.key.as_ref()) != "full-path" {
                        continue;
                    }
                    let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
                    let value = value.trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn parse_epub_package_document(xml: &str) -> EpubPackageDocument {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut package = EpubPackageDocument::new();
    let mut stack = Vec::<String>::new();
    let mut current_metadata_tag: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                match tag.as_str() {
                    "metadata" | "manifest" => {}
                    "spine" => {
                        if package.toc_id.is_none() {
                            package.toc_id = xml_attribute_value(&event, reader.decoder(), "toc");
                        }
                    }
                    "item" if stack.last().is_some_and(|section| section == "manifest") => {
                        register_epub_manifest_item(&mut package, &event, reader.decoder());
                    }
                    "itemref" if stack.last().is_some_and(|section| section == "spine") => {
                        register_epub_spine_itemref(&mut package, &event, reader.decoder());
                    }
                    "meta" if stack.last().is_some_and(|section| section == "metadata") => {
                        register_epub_meta(&mut package, &event, reader.decoder());
                    }
                    "title" | "subject" | "creator" | "language" | "publisher" | "identifier"
                    | "date" => {
                        if stack.last().is_some_and(|section| section == "metadata") {
                            current_metadata_tag = Some(tag.clone());
                        }
                    }
                    _ => {}
                }
                stack.push(tag);
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                match tag.as_str() {
                    "item" if stack.last().is_some_and(|section| section == "manifest") => {
                        register_epub_manifest_item(&mut package, &event, reader.decoder());
                    }
                    "itemref" if stack.last().is_some_and(|section| section == "spine") => {
                        register_epub_spine_itemref(&mut package, &event, reader.decoder());
                    }
                    "meta" if stack.last().is_some_and(|section| section == "metadata") => {
                        register_epub_meta(&mut package, &event, reader.decoder());
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(text)) => {
                let Some(tag) = current_metadata_tag.as_deref() else {
                    continue;
                };
                let Ok(value) = text.decode() else {
                    continue;
                };
                assign_epub_metadata_text(&mut package.metadata, tag, value.as_ref());
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                if current_metadata_tag.as_deref() == Some(tag.as_str()) {
                    current_metadata_tag = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    if package.ncx_path.is_none()
        && let Some(toc_id) = package.toc_id.as_ref()
        && let Some(item) = package.manifest.get(toc_id)
    {
        package.ncx_path = Some(item.href.clone());
    }

    package
}

fn register_epub_manifest_item(
    package: &mut EpubPackageDocument,
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) {
    let Some(id) = xml_attribute_value(event, decoder, "id") else {
        return;
    };
    let Some(href) = xml_attribute_value(event, decoder, "href") else {
        return;
    };
    let media_type = xml_attribute_value(event, decoder, "media-type");
    let properties = xml_attribute_value(event, decoder, "properties")
        .map(|value| {
            value
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if package.nav_path.is_none() && properties.iter().any(|property| property == "nav") {
        package.nav_path = Some(href.clone());
    }
    if package.ncx_path.is_none() && media_type.as_deref() == Some("application/x-dtbncx+xml") {
        package.ncx_path = Some(href.clone());
    }
    package.manifest.insert(
        id,
        EpubManifestItem {
            href,
            media_type,
            properties,
        },
    );
}

fn register_epub_spine_itemref(
    package: &mut EpubPackageDocument,
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) {
    if matches!(
        xml_attribute_value(event, decoder, "linear").as_deref(),
        Some("no")
    ) {
        return;
    }
    if let Some(idref) = xml_attribute_value(event, decoder, "idref") {
        package.spine.push(idref);
    }
}

fn register_epub_meta(
    package: &mut EpubPackageDocument,
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) {
    let name = xml_attribute_value(event, decoder, "name");
    let property = xml_attribute_value(event, decoder, "property");
    let content = xml_attribute_value(event, decoder, "content");

    if name.as_deref() == Some("cover") && package.cover_id.is_none() {
        package.cover_id = content.clone();
    }
    if property.as_deref() == Some("dcterms:modified") && package.metadata.modified.is_none() {
        package.metadata.modified = content.and_then(|value| present_str(&value, "Modified"));
    }
}

fn assign_epub_metadata_text(metadata: &mut DocumentMetadata, tag: &str, value: &str) {
    match tag {
        "title" if metadata.title.is_none() => {
            metadata.title = present_str(value, "Title");
        }
        "subject" if metadata.subject.is_none() => {
            metadata.subject = present_str(value, "Subject");
        }
        "creator" if metadata.author.is_none() => {
            metadata.author = present_str(value, "Author");
        }
        "date" if metadata.created.is_none() => {
            metadata.created = present_str(value, "Created");
        }
        "language" => push_epub_metadata_once(metadata, "Language", value),
        "publisher" => push_epub_metadata_once(metadata, "Publisher", value),
        "identifier" => push_epub_metadata_once(metadata, "Identifier", value),
        _ => {}
    }
}

fn push_epub_metadata_once(metadata: &mut DocumentMetadata, label: &str, value: &str) {
    let Some(value) = present_str(value, label) else {
        return;
    };
    if metadata
        .metadata
        .iter()
        .all(|(existing, _)| existing != label)
    {
        metadata.metadata.push((label.to_string(), value));
    }
}

fn extract_epub_table_of_contents<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    package: &EpubPackageDocument,
    package_path: &str,
) -> Vec<EpubNavPoint> {
    let nav_href = package.nav_path.as_deref().or(package.ncx_path.as_deref());
    let Some(nav_href) = nav_href else {
        return Vec::new();
    };
    let resolved = resolve_zip_entry_path(package_path, nav_href);
    let Some(xml) = read_zip_entry_limited(archive, &resolved, EPUB_NAV_ENTRY_LIMIT_BYTES) else {
        return Vec::new();
    };

    if package.nav_path.as_deref() == Some(nav_href) {
        parse_epub_nav_toc(&xml)
    } else {
        parse_ncx_toc(&xml)
    }
}

fn build_epub_sections(
    package: &EpubPackageDocument,
    package_path: &str,
    nav_points: &[EpubNavPoint],
) -> Vec<EpubSection> {
    let titles_by_path = nav_points
        .iter()
        .filter_map(|point| {
            point.href.as_deref().map(|href| {
                (
                    resolve_zip_entry_path(package_path, href),
                    point.label.trim().to_string(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();

    let fallback_titles = nav_points
        .iter()
        .map(|point| point.label.trim())
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut fallback_index = 0usize;
    let mut sections = Vec::new();

    for idref in &package.spine {
        let Some(item) = package.manifest.get(idref) else {
            continue;
        };
        if !epub_manifest_item_is_text(item) {
            continue;
        }

        let path = resolve_zip_entry_path(package_path, &item.href);
        let title = titles_by_path.get(&path).cloned().or_else(|| {
            let title = fallback_titles.get(fallback_index).cloned();
            fallback_index += 1;
            title
        });
        sections.push(EpubSection { path, title });
    }

    sections
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

fn resolve_epub_cover_item(package: &EpubPackageDocument) -> Option<&EpubManifestItem> {
    package
        .manifest
        .values()
        .find(|item| {
            item.properties
                .iter()
                .any(|property| property == "cover-image")
        })
        .or_else(|| {
            package
                .cover_id
                .as_deref()
                .and_then(|cover_id| package.manifest.get(cover_id))
        })
        .or_else(|| {
            package.manifest.values().find(|item| {
                epub_manifest_item_is_image(item)
                    && strip_fragment_identifier(&item.href)
                        .to_ascii_lowercase()
                        .contains("cover")
            })
        })
}

fn epub_manifest_item_is_text(item: &EpubManifestItem) -> bool {
    if item.properties.iter().any(|property| property == "nav") {
        return false;
    }
    match item.media_type.as_deref() {
        Some("application/xhtml+xml")
        | Some("application/xml")
        | Some("text/html")
        | Some("application/x-dtbook+xml") => true,
        _ => {
            let href = strip_fragment_identifier(&item.href).to_ascii_lowercase();
            href.ends_with(".xhtml")
                || href.ends_with(".html")
                || href.ends_with(".htm")
                || href.ends_with(".xml")
        }
    }
}

fn epub_manifest_item_is_image(item: &EpubManifestItem) -> bool {
    matches!(
        item.media_type.as_deref(),
        Some("image/png")
            | Some("image/jpeg")
            | Some("image/gif")
            | Some("image/webp")
            | Some("image/svg+xml")
    ) || matches!(
        strip_fragment_identifier(&item.href).to_ascii_lowercase().as_str(),
        href if href.ends_with(".png")
            || href.ends_with(".jpg")
            || href.ends_with(".jpeg")
            || href.ends_with(".gif")
            || href.ends_with(".webp")
            || href.ends_with(".svg")
    )
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

fn epub_section_title_from_path(path: &str) -> Option<String> {
    Path::new(strip_fragment_identifier(path))
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace(['_', '-'], " "))
        .map(|stem| stem.trim().to_string())
        .filter(|stem| !stem.is_empty())
}

fn parse_epub_nav_toc(xml: &str) -> Vec<EpubNavPoint> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut nav_stack = Vec::<bool>::new();
    let mut item_depth = 0usize;
    let mut current_label = String::new();
    let mut current_href: Option<String> = None;
    let mut items = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "nav" {
                    nav_stack.push(epub_nav_is_toc(&event, reader.decoder()));
                    continue;
                }
                if !epub_nav_stack_active(&nav_stack) {
                    continue;
                }
                if tag == "li" {
                    item_depth += 1;
                    if item_depth == 1 {
                        current_label.clear();
                        current_href = None;
                    }
                } else if tag == "a" && item_depth > 0 && current_href.is_none() {
                    current_href = xml_attribute_value(&event, reader.decoder(), "href");
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                if !epub_nav_stack_active(&nav_stack) || item_depth == 0 {
                    continue;
                }
                if tag == "br" {
                    append_epub_text_fragment(&mut current_label, " ");
                } else if tag == "a" && current_href.is_none() {
                    current_href = xml_attribute_value(&event, reader.decoder(), "href");
                }
            }
            Ok(Event::Text(text)) => {
                if !epub_nav_stack_active(&nav_stack) || item_depth == 0 {
                    continue;
                }
                if let Ok(value) = text.decode() {
                    append_epub_text_fragment(&mut current_label, value.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "li" && epub_nav_stack_active(&nav_stack) {
                    if item_depth == 1 {
                        push_epub_nav_item(&mut items, current_href.take(), &current_label);
                    }
                    item_depth = item_depth.saturating_sub(1);
                    continue;
                }
                if tag == "nav" {
                    let completed = nav_stack.pop().unwrap_or(false);
                    if completed && !items.is_empty() {
                        break;
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    items
}

fn parse_ncx_toc(xml: &str) -> Vec<EpubNavPoint> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut nav_depth = 0usize;
    let mut in_label = false;
    let mut in_text = false;
    let mut current_label = String::new();
    let mut current_href: Option<String> = None;
    let mut items = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                match tag.as_str() {
                    "navPoint" => {
                        nav_depth += 1;
                        if nav_depth == 1 {
                            current_label.clear();
                            current_href = None;
                        }
                    }
                    "navLabel" if nav_depth > 0 => {
                        in_label = true;
                        current_label.clear();
                    }
                    "text" if in_label => in_text = true,
                    "content" if nav_depth > 0 && current_href.is_none() => {
                        current_href = xml_attribute_value(&event, reader.decoder(), "src");
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(event)) => {
                if nav_depth > 0 && local_name(event.name().as_ref()) == "content" {
                    current_href = xml_attribute_value(&event, reader.decoder(), "src");
                }
            }
            Ok(Event::Text(text)) => {
                if in_label
                    && in_text
                    && let Ok(value) = text.decode()
                {
                    append_epub_text_fragment(&mut current_label, value.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                match tag.as_str() {
                    "text" => in_text = false,
                    "navLabel" => in_label = false,
                    "navPoint" => {
                        if nav_depth == 1 {
                            push_epub_nav_item(&mut items, current_href.take(), &current_label);
                        }
                        nav_depth = nav_depth.saturating_sub(1);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    items
}

fn epub_nav_is_toc(
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) -> bool {
    event.attributes().flatten().any(|attribute| {
        let key = local_name(attribute.key.as_ref());
        let Ok(value) = attribute.decode_and_unescape_value(decoder) else {
            return false;
        };
        let value = value.trim();
        (key == "type" && value.split_whitespace().any(|token| token == "toc"))
            || (key == "role" && value.split_whitespace().any(|token| token == "doc-toc"))
    })
}

fn epub_nav_stack_active(nav_stack: &[bool]) -> bool {
    nav_stack.last().copied().unwrap_or(false)
}

fn push_epub_nav_item(items: &mut Vec<EpubNavPoint>, href: Option<String>, label: &str) {
    let label = label.trim();
    if label.is_empty() {
        return;
    }
    if items
        .last()
        .is_some_and(|existing| existing.label == label && existing.href == href)
    {
        return;
    }
    items.push(EpubNavPoint {
        href,
        label: label.to_string(),
    });
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

fn parse_pdfinfo_fields(output: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for line in output.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), value.to_string());
    }
    fields
}

fn parse_pdf_version(bytes: &[u8]) -> Option<&str> {
    let header = std::str::from_utf8(bytes).ok()?;
    let header = header.lines().next()?.trim();
    header.strip_prefix("%PDF-")
}

fn parse_plist_dict(xml: &str) -> Option<BTreeMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut fields = BTreeMap::new();
    let mut pending_key: Option<String> = None;
    let mut current_tag: Option<String> = None;
    let mut current_array_key: Option<String> = None;
    let mut array_values = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "array" {
                    current_array_key = pending_key.take();
                    array_values.clear();
                }
                current_tag = Some(tag);
            }
            Ok(Event::Empty(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "true" || tag == "false" {
                    let Some(key) = pending_key.take() else {
                        continue;
                    };
                    fields.insert(key, tag);
                }
                current_tag = None;
            }
            Ok(Event::Text(text)) => {
                let Ok(value) = text.decode() else {
                    continue;
                };
                let value = value.trim();
                if value.is_empty() {
                    continue;
                }
                match current_tag.as_deref() {
                    Some("key") => pending_key = Some(value.to_string()),
                    Some("string") | Some("date") | Some("integer") | Some("real") => {
                        if current_array_key.is_some() {
                            array_values.push(value.to_string());
                        } else if let Some(key) = pending_key.take() {
                            fields.insert(key, value.to_string());
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name().as_ref());
                if tag == "array"
                    && let Some(key) = current_array_key.take()
                    && !array_values.is_empty()
                {
                    fields.insert(key, array_values.join(", "));
                    array_values.clear();
                }
                current_tag = None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

fn parse_doc_property_set(bytes: &[u8]) -> BTreeMap<u32, DocPropertyValue> {
    let mut properties = BTreeMap::new();
    let Some(section_count) = read_u32(bytes, 28) else {
        return properties;
    };
    if section_count == 0 {
        return properties;
    }
    let Some(section_offset) = read_u32(bytes, 44).map(|offset| offset as usize) else {
        return properties;
    };
    if section_offset >= bytes.len() {
        return properties;
    }

    let section = &bytes[section_offset..];
    let Some(property_count) = read_u32(section, 4).map(|count| count as usize) else {
        return properties;
    };
    for index in 0..property_count {
        let entry_offset = 8 + index * 8;
        let Some(property_id) = read_u32(section, entry_offset) else {
            continue;
        };
        let Some(value_offset) = read_u32(section, entry_offset + 4).map(|offset| offset as usize)
        else {
            continue;
        };
        if let Some(value) = parse_doc_property_value(section, value_offset) {
            properties.insert(property_id, value);
        }
    }

    properties
}

fn parse_doc_property_value(section: &[u8], offset: usize) -> Option<DocPropertyValue> {
    let value_type = read_u16(section, offset)?;
    match value_type {
        VT_I4 | VT_UI4 => {
            read_u32(section, offset + 4).map(|value| DocPropertyValue::Count(value as u64))
        }
        VT_LPSTR => parse_lpstr(section, offset + 4).map(DocPropertyValue::Text),
        VT_LPWSTR => parse_lpwstr(section, offset + 4).map(DocPropertyValue::Text),
        VT_FILETIME => parse_filetime(section, offset + 4).map(DocPropertyValue::Timestamp),
        _ => None,
    }
}

fn parse_lpstr(bytes: &[u8], offset: usize) -> Option<String> {
    let length = read_u32(bytes, offset)? as usize;
    if length == 0 {
        return None;
    }
    let slice = bytes.get(offset + 4..offset + 4 + length)?;
    let content = slice.strip_suffix(&[0]).unwrap_or(slice);
    let value = String::from_utf8(content.to_vec()).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_lpwstr(bytes: &[u8], offset: usize) -> Option<String> {
    let length = read_u32(bytes, offset)? as usize;
    if length == 0 {
        return None;
    }
    let byte_len = length.checked_mul(2)?;
    let slice = bytes.get(offset + 4..offset + 4 + byte_len)?;
    let mut units = Vec::with_capacity(length);
    for chunk in slice.chunks_exact(2) {
        units.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    if let Some(0) = units.last().copied() {
        units.pop();
    }
    let value = String::from_utf16(&units).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_filetime(bytes: &[u8], offset: usize) -> Option<String> {
    let ticks = read_u64(bytes, offset)?;
    if ticks < WINDOWS_TO_UNIX_EPOCH_SECONDS * WINDOWS_TICKS_PER_SECOND {
        return None;
    }
    let unix_seconds =
        (ticks / WINDOWS_TICKS_PER_SECOND).checked_sub(WINDOWS_TO_UNIX_EPOCH_SECONDS)?;
    format_unix_utc(unix_seconds)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset + 8)?;
    Some(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn doc_property_text(
    properties: &BTreeMap<u32, DocPropertyValue>,
    property_id: u32,
) -> Option<String> {
    match properties.get(&property_id) {
        Some(DocPropertyValue::Text(value)) => present_str(value, ""),
        _ => None,
    }
}

fn doc_property_time(
    properties: &BTreeMap<u32, DocPropertyValue>,
    property_id: u32,
) -> Option<String> {
    match properties.get(&property_id) {
        Some(DocPropertyValue::Timestamp(value)) => present_str(value, ""),
        _ => None,
    }
}

fn doc_property_count(
    properties: &BTreeMap<u32, DocPropertyValue>,
    property_id: u32,
) -> Option<u64> {
    match properties.get(&property_id) {
        Some(DocPropertyValue::Count(value)) => Some(*value),
        _ => None,
    }
}
