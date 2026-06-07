use crate::core::{Entry, EntryKind, SortMode, SymlinkInfo};
use anyhow::{Context, Result};
use std::{
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) struct DirectoryFingerprint {
    pub digest: u64,
    pub entries: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectorySnapshot {
    pub entries: Vec<Entry>,
    pub fingerprint: DirectoryFingerprint,
}

#[derive(Clone, Debug)]
struct FingerprintPart {
    name: String,
    kind: EntryKind,
    symlink: Option<FingerprintSymlink>,
    size: u64,
    modified: Option<(u64, u32)>,
    readonly: bool,
}

#[derive(Clone, Debug, Hash)]
struct FingerprintSymlink {
    target: Option<PathBuf>,
    target_kind: Option<EntryKind>,
}

#[derive(Clone, Debug)]
struct EntryDetails {
    kind: EntryKind,
    symlink: Option<SymlinkInfo>,
    size: u64,
    modified: Option<SystemTime>,
    readonly: bool,
}

#[derive(Clone, Debug, Default)]
struct TrashInfoMetadata {
    original_name: Option<String>,
    deletion_date: Option<SystemTime>,
}

fn trash_info_dir_for_files_dir(dir: &Path) -> Option<PathBuf> {
    (dir.file_name().is_some_and(|n| n == "files"))
        .then(|| dir.parent().map(|p| p.join("info")).filter(|p| p.is_dir()))
        .flatten()
}

/// Reads the display metadata from a `.trashinfo` file inside `info_dir` for
/// the given stored trash file name.
fn read_trash_info_metadata(info_dir: &Path, name: &str) -> Option<TrashInfoMetadata> {
    let info_path = info_dir.join(format!("{name}.trashinfo"));
    let content = fs::read_to_string(info_path).ok()?;

    let mut metadata = TrashInfoMetadata::default();
    for line in content.lines() {
        if let Some(date_str) = line.trim().strip_prefix("DeletionDate=") {
            metadata.deletion_date = parse_trash_deletion_date(date_str);
        } else if let Some(path_str) = line.trim().strip_prefix("Path=") {
            metadata.original_name = trash_original_name(path_str);
        }
    }
    Some(metadata)
}

fn trash_original_name(encoded_path: &str) -> Option<String> {
    super::trashinfo::original_basename_from_path_value(encoded_path)
}

/// Parses a `DeletionDate` value from a `.trashinfo` file into a `SystemTime`.
///
/// The format is `YYYY-MM-DDTHH:MM:SS` in local time. Because Rust's standard library has no
/// timezone support, the timestamp is treated as UTC for the purpose of computing a relative age
/// ("trashed N days ago"). The error introduced by ignoring the UTC offset is at most a few hours
/// and is imperceptible when displaying coarse relative times (days, weeks, months).
fn parse_trash_deletion_date(s: &str) -> Option<SystemTime> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }

    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    let hour: u32 = s[11..13].parse().ok()?;
    let minute: u32 = s[14..16].parse().ok()?;
    let second: u32 = s[17..19].parse().ok()?;

    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    // Gregorian calendar date → Julian Day Number (standard algorithm).
    let a = (14i64 - month as i64) / 12;
    let y = year as i64 + 4800 - a;
    let m = month as i64 + 12 * a - 3;
    let jdn = day as i64 + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32_045;

    // JDN of 1970-01-01 is 2 440 588.
    let days = jdn - 2_440_588;
    if days < 0 {
        return None;
    }

    let secs = days as u64 * 86_400 + hour as u64 * 3_600 + minute as u64 * 60 + second as u64;
    Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

fn canceled_scan_error() -> anyhow::Error {
    io::Error::new(io::ErrorKind::Interrupted, "directory scan canceled").into()
}

fn check_scan_canceled(canceled: &dyn Fn() -> bool) -> Result<()> {
    if canceled() {
        Err(canceled_scan_error())
    } else {
        Ok(())
    }
}

fn read_entries(dir: &Path, show_hidden: bool, canceled: &dyn Fn() -> bool) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        check_scan_canceled(canceled)?;
        let item = match item {
            Ok(item) => item,
            Err(_) => continue,
        };
        let path = item.path();
        let file_name = item.file_name();
        let name = file_name.to_string_lossy().to_string();
        let hidden = super::is_hidden_entry(&item);
        if hidden && !show_hidden {
            continue;
        }

        if let Ok(entry) = entry_from_path(path, name) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub(crate) fn entry_from_path(path: PathBuf, name: String) -> io::Result<Entry> {
    let metadata = fs::symlink_metadata(&path)?;
    let details = entry_details(&path, &metadata);
    Ok(Entry {
        path,
        name_key: name.to_lowercase(),
        name,
        kind: details.kind,
        symlink: details.symlink,
        size: details.size,
        modified: details.modified,
        readonly: details.readonly,
    })
}

pub(crate) fn load_directory_snapshot(
    dir: &Path,
    show_hidden: bool,
    sort_mode: SortMode,
) -> Result<DirectorySnapshot> {
    load_directory_snapshot_cancellable(dir, show_hidden, sort_mode, &|| false)
}

pub(crate) fn load_directory_snapshot_cancellable(
    dir: &Path,
    show_hidden: bool,
    sort_mode: SortMode,
    canceled: &dyn Fn() -> bool,
) -> Result<DirectorySnapshot> {
    let mut entries = read_entries(dir, show_hidden, canceled)?;

    // If this is a freedesktop trash `files/` directory, keep the entry path
    // pointing at the stored trash file but display the original basename from
    // the matching `.trashinfo`. The stored name may be collision-renamed
    // (`foo.txt.2`, `foo.2.txt`, etc.) and is an implementation detail.
    if let Some(info_dir) = trash_info_dir_for_files_dir(dir) {
        for entry in &mut entries {
            check_scan_canceled(canceled)?;
            if let Some(metadata) = read_trash_info_metadata(&info_dir, &entry.name) {
                if let Some(date) = metadata.deletion_date {
                    entry.modified = Some(date);
                }
                if let Some(original_name) = metadata.original_name {
                    entry.name = original_name;
                    entry.name_key = entry.name.to_lowercase();
                }
            }
        }
    }

    check_scan_canceled(canceled)?;
    sort_entries(&mut entries, sort_mode);
    check_scan_canceled(canceled)?;
    let fingerprint = entries_fingerprint(&entries);
    Ok(DirectorySnapshot {
        entries,
        fingerprint,
    })
}

fn entries_fingerprint(entries: &[Entry]) -> DirectoryFingerprint {
    let mut parts = entries
        .iter()
        .map(|entry| FingerprintPart {
            name: entry.name_key.clone(),
            kind: entry.kind,
            symlink: entry.symlink.as_ref().map(fingerprint_symlink),
            size: entry.size,
            modified: fingerprint_time(entry.modified),
            readonly: entry.readonly,
        })
        .collect::<Vec<_>>();
    fingerprint_from_parts(&mut parts)
}

#[cfg(test)]
pub(crate) fn scan_directory_fingerprint(
    dir: &Path,
    show_hidden: bool,
) -> Result<DirectoryFingerprint> {
    scan_directory_fingerprint_cancellable(dir, show_hidden, &|| false)
}

pub(crate) fn scan_directory_fingerprint_cancellable(
    dir: &Path,
    show_hidden: bool,
    canceled: &dyn Fn() -> bool,
) -> Result<DirectoryFingerprint> {
    let mut parts = Vec::new();
    let trash_info_dir = trash_info_dir_for_files_dir(dir);
    for item in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        check_scan_canceled(canceled)?;
        let item = match item {
            Ok(item) => item,
            Err(_) => continue,
        };
        let file_name = item.file_name();
        if super::is_hidden_entry(&item) && !show_hidden {
            continue;
        }

        let metadata = match fs::symlink_metadata(item.path()) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => continue,
        };
        let details = entry_details(&item.path(), &metadata);
        let mut name = file_name.to_string_lossy().to_string();
        let mut modified = details.modified;
        if let Some(info_dir) = trash_info_dir.as_deref()
            && let Some(metadata) = read_trash_info_metadata(info_dir, &name)
        {
            if let Some(original_name) = metadata.original_name {
                name = original_name;
            }
            if let Some(date) = metadata.deletion_date {
                modified = Some(date);
            }
        }
        parts.push(FingerprintPart {
            name: name.to_lowercase(),
            kind: details.kind,
            symlink: details.symlink.as_ref().map(fingerprint_symlink),
            size: details.size,
            modified: fingerprint_time(modified),
            readonly: details.readonly,
        });
    }
    check_scan_canceled(canceled)?;
    Ok(fingerprint_from_parts(&mut parts))
}

fn sort_entries(entries: &mut [Entry], mode: SortMode) {
    entries.sort_by(|left, right| match (left.is_dir(), right.is_dir()) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => match mode {
            SortMode::Name => compare_entry_names(left, right),
            SortMode::Modified => right
                .modified
                .cmp(&left.modified)
                .then_with(|| compare_entry_names(left, right)),
            SortMode::Size => right
                .size
                .cmp(&left.size)
                .then_with(|| compare_entry_names(left, right)),
        },
    });
}

fn compare_entry_names(left: &Entry, right: &Entry) -> Ordering {
    super::natural_cmp(&left.name_key, &right.name_key).then_with(|| left.name.cmp(&right.name))
}

fn entry_details(path: &Path, metadata: &fs::Metadata) -> EntryDetails {
    let is_symlink = metadata.file_type().is_symlink();
    let symlink_target = is_symlink.then(|| fs::read_link(path).ok()).flatten();
    let resolved = is_symlink.then(|| fs::metadata(path).ok()).flatten();
    let symlink = is_symlink.then(|| SymlinkInfo {
        target: symlink_target,
        target_kind: resolved.as_ref().map(metadata_kind),
    });
    let metadata = resolved.as_ref().unwrap_or(metadata);
    EntryDetails {
        kind: metadata_kind(metadata),
        symlink,
        size: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        modified: metadata.modified().ok(),
        readonly: metadata.permissions().readonly(),
    }
}

fn metadata_kind(metadata: &fs::Metadata) -> EntryKind {
    if metadata.is_dir() {
        EntryKind::Directory
    } else {
        EntryKind::File
    }
}

fn fingerprint_symlink(symlink: &SymlinkInfo) -> FingerprintSymlink {
    FingerprintSymlink {
        target: symlink.target.clone(),
        target_kind: symlink.target_kind,
    }
}

fn fingerprint_from_parts(parts: &mut [FingerprintPart]) -> DirectoryFingerprint {
    parts.sort_by(|left, right| left.name.cmp(&right.name));

    let mut hasher = DefaultHasher::new();
    for part in parts.iter() {
        part.name.hash(&mut hasher);
        match part.kind {
            EntryKind::Directory => 0u8.hash(&mut hasher),
            EntryKind::File => 1u8.hash(&mut hasher),
        }
        part.symlink.hash(&mut hasher);
        part.size.hash(&mut hasher);
        part.modified.hash(&mut hasher);
        part.readonly.hash(&mut hasher);
    }

    DirectoryFingerprint {
        digest: hasher.finish(),
        entries: parts.len(),
    }
}

fn fingerprint_time(time: Option<SystemTime>) -> Option<(u64, u32)> {
    time.and_then(|time| {
        time.duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
    })
}
#[cfg(test)]
mod tests;
