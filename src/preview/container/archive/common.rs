use super::*;
use std::time::SystemTime;

pub(super) fn normalize_archive_path(item: &str, strip_version_suffix: bool) -> Option<String> {
    normalize_archive_entry(item, strip_version_suffix).map(|entry| entry.path)
}

pub(super) fn archive_image_extension(path: &str) -> Option<&'static str> {
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

pub(super) fn system_time_key(time: SystemTime) -> Option<(u64, u32)> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
}

pub(super) fn parse_key_value_line(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(" = ")?;
    Some((key.trim(), value.trim()))
}

pub(super) fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}
