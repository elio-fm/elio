use super::super::{
    common::{DOCUMENT_XML_ENTRY_LIMIT_BYTES, format_unix_utc, present_str, push_count_stat},
    metadata::DocumentMetadata,
};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

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

enum DocPropertyValue {
    Count(u64),
    Text(String),
    Timestamp(String),
}

pub(super) fn extract_doc_metadata(path: &Path) -> Option<DocumentMetadata> {
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
