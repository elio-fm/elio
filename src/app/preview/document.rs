use super::{PreviewContent, PreviewKind};
use crate::{appearance, file_facts::DocumentFormat};
use quick_xml::{Reader, events::Event};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Cursor, Read},
    path::Path,
};
use zip::ZipArchive;

const DOCUMENT_PREVIEW_LIMIT_BYTES: u64 = 512 * 1024;
const DOCUMENT_XML_ENTRY_LIMIT_BYTES: usize = 64 * 1024;
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

#[derive(Default)]
struct DocumentMetadata {
    variant: Option<String>,
    title: Option<String>,
    subject: Option<String>,
    author: Option<String>,
    modified_by: Option<String>,
    application: Option<String>,
    created: Option<String>,
    modified: Option<String>,
    pages: Option<u64>,
    words: Option<u64>,
    characters: Option<u64>,
}

impl DocumentMetadata {
    fn is_empty(&self) -> bool {
        self.variant.is_none()
            && self.title.is_none()
            && self.subject.is_none()
            && self.author.is_none()
            && self.modified_by.is_none()
            && self.application.is_none()
            && self.created.is_none()
            && self.modified.is_none()
            && self.pages.is_none()
            && self.words.is_none()
            && self.characters.is_none()
    }
}

enum DocPropertyValue {
    Count(u64),
    Text(String),
    Timestamp(String),
}

pub(super) fn build_document_preview(
    path: &Path,
    format: DocumentFormat,
) -> Option<PreviewContent> {
    let metadata = match format {
        DocumentFormat::Doc => extract_doc_metadata(path),
        DocumentFormat::Docx => extract_zip_document_metadata(path, extract_docx_metadata),
        DocumentFormat::Odt => extract_zip_document_metadata(path, extract_odt_metadata),
        DocumentFormat::Pages => extract_zip_document_metadata(path, extract_pages_metadata),
    }?;

    Some(render_document_preview(format, metadata))
}

fn extract_zip_document_metadata(
    path: &Path,
    extract: fn(&mut ZipArchive<Cursor<Vec<u8>>>) -> DocumentMetadata,
) -> Option<DocumentMetadata> {
    let mut bytes = Vec::with_capacity(DOCUMENT_PREVIEW_LIMIT_BYTES as usize);
    File::open(path)
        .ok()?
        .take(DOCUMENT_PREVIEW_LIMIT_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;

    let cursor = Cursor::new(bytes);
    let metadata = match ZipArchive::new(cursor) {
        Ok(mut archive) => extract(&mut archive),
        Err(_) => DocumentMetadata::default(),
    };
    Some(metadata)
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
    metadata.pages = doc_property_count(&properties, DOC_PROPERTY_PAGE_COUNT);
    metadata.words = doc_property_count(&properties, DOC_PROPERTY_WORD_COUNT);
    metadata.characters = doc_property_count(&properties, DOC_PROPERTY_CHAR_COUNT);

    Some(metadata)
}

fn render_document_preview(format: DocumentFormat, metadata: DocumentMetadata) -> PreviewContent {
    let palette = appearance::palette();
    let mut lines = Vec::new();
    let metadata_is_empty = metadata.is_empty();
    let document = vec![
        ("Variant", metadata.variant),
        ("Title", metadata.title),
        ("Subject", metadata.subject),
    ];
    let people = vec![
        ("Author", metadata.author),
        ("Modified By", metadata.modified_by),
        ("Application", metadata.application),
    ];
    let dates = vec![
        ("Created", metadata.created),
        ("Modified", metadata.modified),
    ];
    let stats = vec![
        ("Pages", metadata.pages.map(format_count)),
        ("Words", metadata.words.map(format_count)),
        ("Characters", metadata.characters.map(format_count)),
    ];
    let label_width = section_label_width([
        document.as_slice(),
        people.as_slice(),
        dates.as_slice(),
        stats.as_slice(),
    ])
    .max(6);

    push_section(&mut lines, "Document", &document, label_width, palette);
    push_section(&mut lines, "People", &people, label_width, palette);
    push_section(&mut lines, "Dates", &dates, label_width, palette);
    push_section(&mut lines, "Stats", &stats, label_width, palette);

    if metadata_is_empty {
        lines.push(Line::from("No document metadata available"));
    }

    PreviewContent::new(PreviewKind::Document, lines).with_detail(format.detail_label())
}

fn push_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(&str, Option<String>)],
    label_width: usize,
    palette: appearance::Palette,
) {
    let visible_fields: Vec<_> = fields
        .iter()
        .filter_map(|(label, value)| value.as_deref().map(|value| (*label, value)))
        .collect();
    if visible_fields.is_empty() {
        return;
    }
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line(title, palette));
    for (label, value) in visible_fields {
        lines.push(document_line(label, value, label_width, palette));
    }
}

fn section_label_width<'a>(
    sections: impl IntoIterator<Item = &'a [(&'a str, Option<String>)]>,
) -> usize {
    sections
        .into_iter()
        .flat_map(|fields| fields.iter())
        .filter(|(_, value)| value.is_some())
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(6)
}

fn section_line(title: &str, palette: appearance::Palette) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(palette.accent),
    ))
}

fn document_line(
    label: &str,
    value: &str,
    label_width: usize,
    palette: appearance::Palette,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<width$} ", width = label_width + 1),
            Style::default().fg(palette.muted),
        ),
        Span::styled(value.to_string(), Style::default().fg(palette.text)),
    ])
}

fn extract_docx_metadata<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> DocumentMetadata {
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
    metadata.pages = present_count(app.get("Pages"));
    metadata.words = present_count(app.get("Words"));
    metadata.characters = present_count(app.get("Characters"));

    metadata
}

fn extract_odt_metadata<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> DocumentMetadata {
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
    metadata.pages = present_count(fields.get("page-count"));
    metadata.words = present_count(fields.get("word-count"));
    metadata.characters = present_count(fields.get("character-count"));

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

fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    let limit = (entry.size() as usize).min(DOCUMENT_XML_ENTRY_LIMIT_BYTES);
    let mut bytes = Vec::with_capacity(limit);
    entry
        .take(DOCUMENT_XML_ENTRY_LIMIT_BYTES as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    String::from_utf8(bytes).ok()
}

fn parse_xml_text_fields(xml: &str) -> BTreeMap<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut fields = BTreeMap::new();
    let mut current_text_tag: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                current_text_tag = Some(tag.clone());

                if tag == "document-statistic" {
                    for attribute in event.attributes().flatten() {
                        let key = local_name(attribute.key.as_ref());
                        if let Ok(value) = attribute.decode_and_unescape_value(reader.decoder()) {
                            let value = value.trim();
                            if !value.is_empty() {
                                fields.insert(key, value.to_string());
                            }
                        }
                    }
                    current_text_tag = None;
                }
            }
            Ok(Event::Empty(event)) => {
                if local_name(event.name().as_ref()) == "document-statistic" {
                    for attribute in event.attributes().flatten() {
                        let key = local_name(attribute.key.as_ref());
                        if let Ok(value) = attribute.decode_and_unescape_value(reader.decoder()) {
                            let value = value.trim();
                            if !value.is_empty() {
                                fields.insert(key, value.to_string());
                            }
                        }
                    }
                }
                current_text_tag = None;
            }
            Ok(Event::Text(text)) => {
                if let Some(tag) = &current_text_tag
                    && let Ok(value) = text.decode()
                {
                    let value = value.trim();
                    if !value.is_empty() {
                        fields
                            .entry(tag.clone())
                            .or_insert_with(|| value.to_string());
                    }
                }
            }
            Ok(Event::End(_)) => current_text_tag = None,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    fields
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

fn local_name(name: &[u8]) -> String {
    let name = std::str::from_utf8(name).unwrap_or_default();
    name.rsplit(':').next().unwrap_or(name).to_string()
}

fn present_string(value: Option<&String>, label: &str) -> Option<String> {
    present_str(value?.trim(), label)
}

fn present_str(value: &str, label: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    Some(normalize_metadata_value(label, value))
}

fn first_present_string(
    fields: &BTreeMap<String, String>,
    keys: &[&str],
    label: &str,
) -> Option<String> {
    keys.iter()
        .find_map(|key| fields.get(*key))
        .and_then(|value| present_string(Some(value), label))
}

fn present_count(value: Option<&String>) -> Option<u64> {
    value?.trim().parse().ok()
}

fn normalize_metadata_value(label: &str, value: &str) -> String {
    match label {
        "Created" | "Modified" => humanize_document_datetime(value),
        _ => value.trim().to_string(),
    }
}

fn humanize_document_datetime(value: &str) -> String {
    let trimmed = value.trim();
    let (date, rest) = match trimmed.split_once('T').or_else(|| trimmed.split_once(' ')) {
        Some(parts) => parts,
        None => return trimmed.to_string(),
    };

    let Some((year, month, day)) = parse_iso_date(date) else {
        return trimmed.to_string();
    };
    let Some((hour, minute, timezone)) = parse_iso_time(rest) else {
        return trimmed.to_string();
    };

    format_calendar_datetime(year, month, day, hour, minute, timezone)
}

fn parse_iso_date(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((year, month, day))
}

fn parse_iso_time(value: &str) -> Option<(u32, u32, Option<&str>)> {
    let time_end = value.find(['Z', '+', '-']).unwrap_or(value.len());
    let time_part = &value[..time_end];
    let timezone = value.get(time_end..).filter(|segment| !segment.is_empty());
    let mut time_segments = time_part.split(':');
    let hour = time_segments.next()?.parse().ok()?;
    let minute = time_segments.next()?.parse().ok()?;
    let _seconds = time_segments.next();
    if time_segments.next().is_some() {
        return None;
    }
    Some((hour, minute, normalize_timezone(timezone)))
}

fn normalize_timezone(timezone: Option<&str>) -> Option<&str> {
    match timezone {
        Some("Z") => Some("UTC"),
        Some(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

fn format_unix_utc(unix_seconds: u64) -> Option<String> {
    let days = unix_seconds / 86_400;
    let seconds_of_day = unix_seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64)?;
    let hour = (seconds_of_day / 3_600) as u32;
    let minute = ((seconds_of_day % 3_600) / 60) as u32;
    Some(format_calendar_datetime(
        year,
        month,
        day,
        hour,
        minute,
        Some("UTC"),
    ))
}

fn civil_from_days(days_since_unix_epoch: i64) -> Option<(i32, u32, u32)> {
    let z = days_since_unix_epoch.checked_add(719_468)?;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    Some((year as i32, month as u32, day as u32))
}

fn format_calendar_datetime(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    timezone: Option<&str>,
) -> String {
    let month_name = match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => return format!("{year}-{month:02}-{day:02} {hour:02}:{minute:02}"),
    };

    match timezone {
        Some(timezone) => format!("{month_name} {day}, {year} {hour:02}:{minute:02} {timezone}"),
        None => format!("{month_name} {day}, {year} {hour:02}:{minute:02}"),
    }
}

fn format_count(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    formatted.chars().rev().collect()
}
