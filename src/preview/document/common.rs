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
    let Some((hour, minute, raw_tz)) = parse_iso_time(rest) else {
        return trimmed.to_string();
    };

    // If the source has an explicit timezone, convert to the user's local timezone.
    if let Some(tz_str) = raw_tz {
        if let Some(source_offset_minutes) = parse_offset_minutes(tz_str) {
            let utc_seconds =
                to_unix_seconds(year, month, day, hour, minute, source_offset_minutes);
            if let Some((ly, lm, ld, lh, lmin, local_offset)) = unix_to_local(utc_seconds) {
                return format_calendar_datetime(
                    ly,
                    lm,
                    ld,
                    lh,
                    lmin,
                    Some(&format_offset_label(local_offset)),
                );
            }
        }
        // Fallback: show in source timezone when local conversion is unavailable.
        return format_calendar_datetime(
            year,
            month,
            day,
            hour,
            minute,
            normalize_timezone(Some(tz_str)),
        );
    }

    // No timezone info: show as-is without a label.
    format_calendar_datetime(year, month, day, hour, minute, None)
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
    // Return the raw timezone suffix (e.g. "Z", "+02:00") without normalizing,
    // so the caller can parse the numeric offset for local-time conversion.
    let timezone = value.get(time_end..).filter(|s| !s.is_empty());
    let mut time_segments = time_part.split(':');
    let hour = time_segments.next()?.parse().ok()?;
    let minute = time_segments.next()?.parse().ok()?;
    let _seconds = time_segments.next();
    if time_segments.next().is_some() {
        return None;
    }
    Some((hour, minute, timezone))
}

fn normalize_timezone(timezone: Option<&str>) -> Option<&str> {
    match timezone {
        Some("Z") => Some("UTC"),
        Some(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

/// Parses a raw ISO 8601 timezone suffix ("Z", "+02:00", "-05:30", etc.) into
/// total UTC offset minutes.  Returns `None` for unrecognised formats.
fn parse_offset_minutes(tz_str: &str) -> Option<i32> {
    if tz_str == "Z" {
        return Some(0);
    }
    let (sign, rest) = match tz_str.as_bytes().first()? {
        b'+' => (1i32, &tz_str[1..]),
        b'-' => (-1i32, &tz_str[1..]),
        _ => return None,
    };
    let mut parts = rest.split(':');
    let hours: i32 = parts.next()?.parse().ok()?;
    let minutes: i32 = parts.next()?.parse().ok()?;
    // Reject extra segments (e.g. "+05:30:99") and out-of-range values.
    if parts.next().is_some() || !(0..60).contains(&minutes) || !(0..=14).contains(&hours) {
        return None;
    }
    Some(sign * (hours * 60 + minutes))
}

/// Formats a UTC offset in minutes as a display label ("UTC", "+01:00", "-05:30").
fn format_offset_label(offset_minutes: i32) -> String {
    if offset_minutes == 0 {
        "UTC".to_string()
    } else {
        let sign = if offset_minutes >= 0 { '+' } else { '-' };
        let abs = offset_minutes.unsigned_abs();
        format!("{sign}{:02}:{:02}", abs / 60, abs % 60)
    }
}

/// Converts a UTC unix timestamp to the user's local timezone.
///
/// Returns `(year, month, day, hour, minute, local_utc_offset_minutes)`.
#[cfg(unix)]
fn unix_to_local(unix_seconds: i64) -> Option<(i32, u32, u32, u32, u32, i32)> {
    use std::mem::MaybeUninit;
    // `libc::time_t` is i32 on 32-bit targets and i64 on 64-bit targets.
    // Return None for timestamps that don't fit (only an issue on 32-bit after 2038).
    let t = libc::time_t::try_from(unix_seconds).ok()?;
    let mut tm = MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `libc::localtime_r` is thread-safe and fully initialises `*tm`
    // before returning a non-null pointer.  `libc::tm` has the correct layout
    // for the current platform as defined by the libc crate.
    let result = unsafe { libc::localtime_r(&t, tm.as_mut_ptr()) };
    if result.is_null() {
        return None;
    }
    let tm = unsafe { tm.assume_init() };
    let year = tm.tm_year.checked_add(1900)?;
    let month = (tm.tm_mon + 1) as u32;
    let day = tm.tm_mday as u32;
    let hour = tm.tm_hour as u32;
    let minute = tm.tm_min as u32;
    // tm_gmtoff is seconds east of UTC; fits in i32 for all valid TZ offsets.
    let offset_minutes = (tm.tm_gmtoff / 60) as i32;
    Some((year, month, day, hour, minute, offset_minutes))
}

/// Windows path: convert unix timestamp → FILETIME → UTC SYSTEMTIME → local SYSTEMTIME.
/// `SystemTimeToTzSpecificLocalTime(NULL, …)` uses the current system timezone and
/// correctly accounts for the DST rule in effect at `unix_seconds`.
#[cfg(windows)]
fn unix_to_local(unix_seconds: i64) -> Option<(i32, u32, u32, u32, u32, i32)> {
    // FILETIME is 100-nanosecond intervals since 1601-01-01 00:00:00 UTC.
    const UNIX_TO_WINDOWS_EPOCH: i64 = 11_644_473_600; // seconds between epochs
    const TICKS_PER_SECOND: i64 = 10_000_000;

    let ticks = unix_seconds
        .checked_add(UNIX_TO_WINDOWS_EPOCH)?
        .checked_mul(TICKS_PER_SECOND)?;
    if ticks < 0 {
        return None;
    }

    // FILETIME is two u32s (low, high) in little-endian order.
    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    // SYSTEMTIME fields are all u16.
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    unsafe extern "system" {
        fn FileTimeToSystemTime(ft: *const FileTime, st: *mut SystemTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(
            tz: *const u8, // NULL → use the current system timezone
            utc: *const SystemTime,
            local: *mut SystemTime,
        ) -> i32;
    }

    let ft = FileTime {
        low: (ticks as u64 & 0xFFFF_FFFF) as u32,
        high: ((ticks as u64) >> 32) as u32,
    };
    let mut utc_st = SystemTime::default();
    let mut local_st = SystemTime::default();

    // SAFETY: structs mirror the Win32 FILETIME / SYSTEMTIME layouts exactly.
    // NULL timezone pointer causes the function to use the current system
    // timezone, DST-aware for the supplied timestamp.
    unsafe {
        if FileTimeToSystemTime(&ft, &mut utc_st) == 0 {
            return None;
        }
        if SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc_st, &mut local_st) == 0 {
            return None;
        }
    }

    // Derive the UTC offset by comparing day-aligned minute totals.
    // days_from_civil handles month/day wrap-arounds (e.g. UTC 23:00 → next day local).
    let utc_mins =
        days_from_civil(utc_st.year as i32, utc_st.month as u32, utc_st.day as u32) * 24 * 60
            + utc_st.hour as i64 * 60
            + utc_st.minute as i64;
    let local_mins = days_from_civil(
        local_st.year as i32,
        local_st.month as u32,
        local_st.day as u32,
    ) * 24
        * 60
        + local_st.hour as i64 * 60
        + local_st.minute as i64;

    Some((
        local_st.year as i32,
        local_st.month as u32,
        local_st.day as u32,
        local_st.hour as u32,
        local_st.minute as u32,
        (local_mins - utc_mins) as i32,
    ))
}

/// Fallback for targets without a supported local-time API: display as UTC.
#[cfg(not(any(unix, windows)))]
fn unix_to_local(unix_seconds: i64) -> Option<(i32, u32, u32, u32, u32, i32)> {
    let days = unix_seconds.checked_div(86_400)?;
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days)?;
    let hour = (seconds_of_day / 3_600) as u32;
    let minute = ((seconds_of_day % 3_600) / 60) as u32;
    Some((year, month, day, hour, minute, 0))
}

/// Converts a civil date plus a known UTC offset to a UTC unix timestamp (seconds).
fn to_unix_seconds(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    offset_minutes: i32,
) -> i64 {
    let days = days_from_civil(year, month, day);
    let time_of_day = hour as i64 * 3_600 + minute as i64 * 60;
    days * 86_400 + time_of_day - offset_minutes as i64 * 60
}

/// Inverse of `civil_from_days`: converts a civil date to days since the Unix epoch.
/// Uses the same Howard Hinnant civil-calendar algorithm.
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 {
        year as i64 - 1
    } else {
        year as i64
    };
    let m = month as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Formats a UTC unix timestamp in the user's local timezone.
pub(super) fn format_unix_local(unix_seconds: u64) -> Option<String> {
    let (year, month, day, hour, minute, offset_minutes) = unix_to_local(unix_seconds as i64)?;
    Some(format_calendar_datetime(
        year,
        month,
        day,
        hour,
        minute,
        Some(&format_offset_label(offset_minutes)),
    ))
}

#[cfg(not(any(unix, windows)))]
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

/// Formats a pdfinfo `CreationDate`/`ModDate` value in the user's local timezone.
///
/// `pdfinfo` date output varies by poppler version:
/// - Newer builds (≥ 22.02) emit ISO 8601, e.g. `"2026-03-11 09:00:00"` (local time, no TZ)
///   or `"2026-03-11T09:00:00+00:00"` (with offset).
/// - Older builds emit a ctime-style string, e.g. `"Wed Mar 11 09:00:00 2026 UTC"`.
///
/// ISO 8601 with an explicit offset is converted to local time.  ISO 8601 without
/// an offset and ctime without a timezone label are passed through as-is (pdfinfo
/// already outputs those in local time).  Ctime with the `UTC` label is converted
/// to local time.  Ctime with any other named timezone label is reformatted but
/// the original label is preserved, since abbreviations like `CST` are ambiguous.
pub(super) fn humanize_pdfinfo_datetime(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return trimmed.to_string();
    }
    // Try the ISO 8601 path first.
    let iso_result = humanize_document_datetime(trimmed);
    if iso_result != trimmed {
        return iso_result;
    }
    // Fall back to ctime-style parsing.
    try_humanize_ctime_datetime(trimmed).unwrap_or_else(|| trimmed.to_string())
}

/// Attempts to parse and reformat a ctime-style datetime string
/// (`"Ddd Mon DD HH:MM:SS YYYY[ TZ]"`).  Returns `None` on parse failure.
fn try_humanize_ctime_datetime(value: &str) -> Option<String> {
    let mut parts = value.split_whitespace();
    let _weekday = parts.next()?;
    let month_str = parts.next()?;
    let day_str = parts.next()?;
    let time_str = parts.next()?;
    let year_str = parts.next()?;
    let tz_str = parts.next(); // optional

    // Reject if there are unexpected extra tokens.
    if parts.next().is_some() {
        return None;
    }

    let month: u32 = parse_ctime_month(month_str)?;
    let day: u32 = day_str.parse().ok()?;
    let year: i32 = year_str.parse().ok()?;

    if !(1900..=9999).contains(&year) || !(1..=31).contains(&day) {
        return None;
    }
    // Time must look like HH:MM or HH:MM:SS.
    if !time_str.contains(':') {
        return None;
    }
    let mut time_parts = time_str.splitn(3, ':');
    let hour: u32 = time_parts.next()?.parse().ok()?;
    let minute: u32 = time_parts.next()?.parse().ok()?;

    match tz_str {
        Some("UTC") | Some("Z") => {
            // Explicit UTC: convert to the user's local timezone.
            let utc_secs = to_unix_seconds(year, month, day, hour, minute, 0);
            if let Some((ly, lm, ld, lh, lmin, loff)) = unix_to_local(utc_secs) {
                return Some(format_calendar_datetime(
                    ly,
                    lm,
                    ld,
                    lh,
                    lmin,
                    Some(&format_offset_label(loff)),
                ));
            }
            // Conversion unavailable: show as UTC.
            Some(format_calendar_datetime(
                year,
                month,
                day,
                hour,
                minute,
                Some("UTC"),
            ))
        }
        Some(tz) => {
            // Named but ambiguous timezone abbreviation (e.g. "CET", "PST"):
            // reformat and keep the original label.
            Some(format_calendar_datetime(
                year,
                month,
                day,
                hour,
                minute,
                Some(tz),
            ))
        }
        None => {
            // No timezone: pdfinfo already outputs in local time; reformat without a label.
            Some(format_calendar_datetime(
                year, month, day, hour, minute, None,
            ))
        }
    }
}

fn parse_ctime_month(name: &str) -> Option<u32> {
    match name {
        "Jan" => Some(1),
        "Feb" => Some(2),
        "Mar" => Some(3),
        "Apr" => Some(4),
        "May" => Some(5),
        "Jun" => Some(6),
        "Jul" => Some(7),
        "Aug" => Some(8),
        "Sep" => Some(9),
        "Oct" => Some(10),
        "Nov" => Some(11),
        "Dec" => Some(12),
        _ => None,
    }
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

#[cfg(test)]
mod tests;
