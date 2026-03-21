use super::metadata::DocumentMetadata;
use quick_xml::{Reader, events::Event};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Cursor, Read},
    path::{Component, Path},
};
use zip::ZipArchive;

pub(super) const DOCUMENT_PREVIEW_LIMIT_BYTES: u64 = 512 * 1024;
pub(super) const DOCUMENT_XML_ENTRY_LIMIT_BYTES: usize = 64 * 1024;

pub(super) fn extract_zip_document_metadata(
    path: &Path,
    extract: impl FnOnce(&mut ZipArchive<Cursor<Vec<u8>>>) -> DocumentMetadata,
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

pub(super) fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Option<String> {
    read_zip_entry_limited(archive, name, DOCUMENT_XML_ENTRY_LIMIT_BYTES)
}

pub(super) fn read_zip_entry_limited<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit_bytes: usize,
) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    let limit = (entry.size() as usize).min(limit_bytes);
    let mut bytes = Vec::with_capacity(limit);
    entry
        .take(limit_bytes as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    String::from_utf8(bytes).ok()
}

pub(super) fn read_zip_entry_bytes_limited<R: Read + std::io::Seek>(
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

pub(super) fn push_count_stat(metadata: &mut DocumentMetadata, label: &str, value: Option<u64>) {
    if let Some(value) = value {
        metadata
            .stats
            .push((label.to_string(), format_count(value)));
    }
}

pub(super) fn push_metadata_field(
    metadata: &mut DocumentMetadata,
    label: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        metadata.metadata.push((label.to_string(), value));
    }
}

pub(super) fn xml_attribute_value(
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    name: &str,
) -> Option<String> {
    event.attributes().flatten().find_map(|attribute| {
        (local_name(attribute.key.as_ref()) == name)
            .then(|| attribute.decode_and_unescape_value(decoder).ok())
            .flatten()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn resolve_zip_entry_path(base_path: &str, href: &str) -> String {
    let href = strip_fragment_identifier(href);
    let base = Path::new(base_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let joined = base.join(href);
    let mut parts = Vec::new();
    for component in joined.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => {
                parts.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    parts.join("/")
}

pub(super) fn strip_fragment_identifier(path: &str) -> &str {
    path.split_once('#').map(|(base, _)| base).unwrap_or(path)
}

pub(super) fn parse_xml_text_fields(xml: &str) -> BTreeMap<String, String> {
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

pub(super) fn local_name(name: &[u8]) -> String {
    let name = std::str::from_utf8(name).unwrap_or_default();
    name.rsplit(':').next().unwrap_or(name).to_string()
}

pub(super) fn present_string(value: Option<&String>, label: &str) -> Option<String> {
    present_str(value?.trim(), label)
}

pub(super) fn present_str(value: &str, label: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    Some(normalize_metadata_value(label, value))
}

pub(super) fn first_present_string(
    fields: &BTreeMap<String, String>,
    keys: &[&str],
    label: &str,
) -> Option<String> {
    keys.iter()
        .find_map(|key| fields.get(*key))
        .and_then(|value| present_string(Some(value), label))
}

pub(super) fn present_count(value: Option<&String>) -> Option<u64> {
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

pub(super) fn format_unix_utc(unix_seconds: u64) -> Option<String> {
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
