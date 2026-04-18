use super::*;
use std::collections::BTreeMap;
use std::time::SystemTime;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::preview) struct ArchiveEntry {
    pub(in crate::preview) path: String,
    pub(in crate::preview) is_dir: bool,
}

#[derive(Default)]
pub(in crate::preview::container) struct ArchiveTreeNode {
    pub(in crate::preview::container) path: String,
    pub(in crate::preview::container) is_dir: bool,
    pub(in crate::preview::container) children: BTreeMap<String, ArchiveTreeNode>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ArchiveMetadata {
    pub(in crate::preview::container::archive) format_label: Option<String>,
    pub(in crate::preview::container::archive) physical_size: Option<u64>,
    pub(in crate::preview::container::archive) compressed_size: Option<u64>,
    pub(in crate::preview::container::archive) unpacked_size: Option<u64>,
    pub(in crate::preview::container::archive) comment: Option<String>,
}

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
    if let Some((key, value)) = line.split_once(" = ") {
        return Some((key.trim(), value.trim()));
    }
    line.strip_suffix(" =").map(|key| (key.trim(), ""))
}

pub(super) fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}
