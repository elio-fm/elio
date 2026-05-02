use crate::core::{Entry, EntryKind, SortMode};
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
    size: u64,
    modified: Option<(u64, u32)>,
    readonly: bool,
}

#[derive(Clone, Copy, Debug)]
struct EntryDetails {
    kind: EntryKind,
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
        let name_key = name.to_lowercase();
        let hidden = super::is_hidden_entry(&item);
        if hidden && !show_hidden {
            continue;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let details = entry_details(&path, &metadata);
        entries.push(Entry {
            path,
            name,
            name_key,
            kind: details.kind,
            size: details.size,
            modified: details.modified,
            readonly: details.readonly,
        });
    }
    Ok(entries)
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
    let resolved = metadata
        .file_type()
        .is_symlink()
        .then(|| fs::metadata(path).ok())
        .flatten();
    let metadata = resolved.as_ref().unwrap_or(metadata);
    EntryDetails {
        kind: if metadata.is_dir() {
            EntryKind::Directory
        } else {
            EntryKind::File
        },
        size: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        modified: metadata.modified().ok(),
        readonly: metadata.permissions().readonly(),
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
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-{label}-{unique}"))
    }

    #[test]
    fn sort_keeps_directories_before_files() {
        let mut entries = vec![
            Entry {
                path: PathBuf::from("beta.txt"),
                name: "beta.txt".to_string(),
                name_key: "beta.txt".to_string(),
                kind: EntryKind::File,
                size: 10,
                modified: None,
                readonly: false,
            },
            Entry {
                path: PathBuf::from("alpha"),
                name: "alpha".to_string(),
                name_key: "alpha".to_string(),
                kind: EntryKind::Directory,
                size: 0,
                modified: None,
                readonly: false,
            },
        ];

        sort_entries(&mut entries, SortMode::Name);
        assert!(entries[0].is_dir());
        assert!(!entries[1].is_dir());
    }

    #[test]
    fn sort_uses_natural_numeric_order_for_names() {
        let mut entries = vec![
            Entry {
                path: PathBuf::from("episode 10.mkv"),
                name: "episode 10.mkv".to_string(),
                name_key: "episode 10.mkv".to_string(),
                kind: EntryKind::File,
                size: 10,
                modified: None,
                readonly: false,
            },
            Entry {
                path: PathBuf::from("episode 2.mkv"),
                name: "episode 2.mkv".to_string(),
                name_key: "episode 2.mkv".to_string(),
                kind: EntryKind::File,
                size: 10,
                modified: None,
                readonly: false,
            },
            Entry {
                path: PathBuf::from("episode 1.mkv"),
                name: "episode 1.mkv".to_string(),
                name_key: "episode 1.mkv".to_string(),
                kind: EntryKind::File,
                size: 10,
                modified: None,
                readonly: false,
            },
        ];

        sort_entries(&mut entries, SortMode::Name);
        let names = entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["episode 1.mkv", "episode 2.mkv", "episode 10.mkv"]
        );
    }

    #[test]
    fn sort_uses_natural_numeric_order_with_non_latin_names() {
        let mut entries = vec![
            Entry {
                path: PathBuf::from("北斗の拳 究極版 10巻.epub"),
                name: "北斗の拳 究極版 10巻.epub".to_string(),
                name_key: "北斗の拳 究極版 10巻.epub".to_string(),
                kind: EntryKind::File,
                size: 10,
                modified: None,
                readonly: false,
            },
            Entry {
                path: PathBuf::from("北斗の拳 究極版 2巻.epub"),
                name: "北斗の拳 究極版 2巻.epub".to_string(),
                name_key: "北斗の拳 究極版 2巻.epub".to_string(),
                kind: EntryKind::File,
                size: 10,
                modified: None,
                readonly: false,
            },
            Entry {
                path: PathBuf::from("北斗の拳 究極版 1巻.epub"),
                name: "北斗の拳 究極版 1巻.epub".to_string(),
                name_key: "北斗の拳 究極版 1巻.epub".to_string(),
                kind: EntryKind::File,
                size: 10,
                modified: None,
                readonly: false,
            },
        ];

        sort_entries(&mut entries, SortMode::Name);
        let names = entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "北斗の拳 究極版 1巻.epub",
                "北斗の拳 究極版 2巻.epub",
                "北斗の拳 究極版 10巻.epub",
            ]
        );
    }

    #[test]
    #[cfg(unix)]
    fn symlinked_file_uses_target_size() {
        use std::os::unix::fs::symlink;

        let root = temp_path("symlink-file");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let target = root.join("target.txt");
        fs::write(&target, "hello world").expect("failed to write target file");
        symlink(&target, root.join("linked.txt")).expect("failed to create symlink");

        let entries = read_entries(&root, false, &|| false).expect("failed to read entries");
        let linked = entries
            .iter()
            .find(|entry| entry.name == "linked.txt")
            .expect("linked file should be present");

        assert_eq!(linked.kind, EntryKind::File);
        assert_eq!(linked.size, 11);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    #[cfg(unix)]
    fn symlinked_directory_uses_target_kind() {
        use std::os::unix::fs::symlink;

        let root = temp_path("symlink-dir");
        let target = root.join("target-dir");
        fs::create_dir_all(&target).expect("failed to create target dir");
        symlink(&target, root.join("linked-dir")).expect("failed to create directory symlink");

        let entries = read_entries(&root, false, &|| false).expect("failed to read entries");
        let linked = entries
            .iter()
            .find(|entry| entry.name == "linked-dir")
            .expect("linked dir should be present");

        assert!(linked.is_dir());
        assert_eq!(linked.size, 0);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn fingerprint_changes_when_visible_directory_entries_change() {
        let root = temp_path("fingerprint");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("one.txt"), "hello").expect("failed to write first file");

        let first = scan_directory_fingerprint(&root, false).expect("failed to fingerprint");
        fs::write(root.join("two.txt"), "world").expect("failed to write second file");
        let second = scan_directory_fingerprint(&root, false).expect("failed to fingerprint");

        assert_ne!(first, second);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn trash_deletion_date_is_parsed_to_unix_timestamp() {
        // 2024-03-15T10:30:00 UTC = 1710498600 seconds since epoch
        let time = parse_trash_deletion_date("2024-03-15T10:30:00").expect("should parse");
        let secs = time
            .duration_since(UNIX_EPOCH)
            .expect("should be after epoch")
            .as_secs();
        assert_eq!(secs, 1_710_498_600);
    }

    #[test]
    fn trash_deletion_date_rejects_invalid_input() {
        assert!(parse_trash_deletion_date("").is_none());
        assert!(parse_trash_deletion_date("not-a-date").is_none());
        assert!(parse_trash_deletion_date("2024-13-01T00:00:00").is_none()); // month 13
        assert!(parse_trash_deletion_date("2024-00-01T00:00:00").is_none()); // month 0
    }

    #[test]
    fn trash_snapshot_uses_deletion_date_from_trashinfo() {
        let root = temp_path("trash-snapshot");
        let files_dir = root.join("files");
        let info_dir = root.join("info");
        fs::create_dir_all(&files_dir).expect("failed to create files dir");
        fs::create_dir_all(&info_dir).expect("failed to create info dir");
        fs::write(files_dir.join("report.pdf"), "dummy").expect("failed to write trashed file");
        fs::write(
            info_dir.join("report.pdf.trashinfo"),
            "[Trash Info]\nPath=/home/user/report.pdf\nDeletionDate=2024-03-15T10:30:00\n",
        )
        .expect("failed to write trashinfo");

        let snapshot =
            load_directory_snapshot(&files_dir, false, SortMode::Name).expect("should load");
        let entry = snapshot
            .entries
            .iter()
            .find(|e| e.name == "report.pdf")
            .expect("entry should be present");

        let secs = entry
            .modified
            .expect("modified should be set")
            .duration_since(UNIX_EPOCH)
            .expect("should be after epoch")
            .as_secs();
        assert_eq!(secs, 1_710_498_600);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn trash_snapshot_displays_original_name_from_trashinfo() {
        let root = temp_path("trash-display-name");
        let files_dir = root.join("files");
        let info_dir = root.join("info");
        fs::create_dir_all(&files_dir).expect("failed to create files dir");
        fs::create_dir_all(&info_dir).expect("failed to create info dir");

        fs::write(files_dir.join("report.pdf.2"), "dummy")
            .expect("failed to write collision-renamed trashed file");
        fs::write(
            info_dir.join("report.pdf.2.trashinfo"),
            "[Trash Info]\nPath=/home/user/Reports/report%20final.pdf\nDeletionDate=2024-03-15T10:30:00\n",
        )
        .expect("failed to write trashinfo");

        let snapshot =
            load_directory_snapshot(&files_dir, false, SortMode::Name).expect("should load");
        let entry = snapshot.entries.first().expect("entry should be present");

        assert_eq!(entry.name, "report final.pdf");
        assert_eq!(entry.name_key, "report final.pdf");
        assert_eq!(
            entry.path.file_name().and_then(|n| n.to_str()),
            Some("report.pdf.2")
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn trash_snapshot_keeps_stored_name_without_original_path_metadata() {
        let root = temp_path("trash-display-name-fallback");
        let files_dir = root.join("files");
        let info_dir = root.join("info");
        fs::create_dir_all(&files_dir).expect("failed to create files dir");
        fs::create_dir_all(&info_dir).expect("failed to create info dir");

        fs::write(files_dir.join("stored-name.txt.2"), "dummy")
            .expect("failed to write trashed file");
        fs::write(
            info_dir.join("stored-name.txt.2.trashinfo"),
            "[Trash Info]\nDeletionDate=2024-03-15T10:30:00\n",
        )
        .expect("failed to write trashinfo");

        let snapshot =
            load_directory_snapshot(&files_dir, false, SortMode::Name).expect("should load");
        let entry = snapshot.entries.first().expect("entry should be present");

        assert_eq!(entry.name, "stored-name.txt.2");
        assert_eq!(entry.name_key, "stored-name.txt.2");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn trash_fingerprint_tracks_original_name_from_trashinfo() {
        let root = temp_path("trash-fingerprint-display-name");
        let files_dir = root.join("files");
        let info_dir = root.join("info");
        fs::create_dir_all(&files_dir).expect("failed to create files dir");
        fs::create_dir_all(&info_dir).expect("failed to create info dir");
        fs::write(files_dir.join("photo.jpeg.2"), "dummy").expect("failed to write trashed file");
        let info_path = info_dir.join("photo.jpeg.2.trashinfo");

        fs::write(
            &info_path,
            "[Trash Info]\nPath=/home/user/photo.jpeg\nDeletionDate=2024-03-15T10:30:00\n",
        )
        .expect("failed to write initial trashinfo");
        let first = scan_directory_fingerprint(&files_dir, false).expect("failed to fingerprint");

        fs::write(
            &info_path,
            "[Trash Info]\nPath=/home/user/renamed-photo.jpeg\nDeletionDate=2024-03-15T10:30:00\n",
        )
        .expect("failed to update trashinfo");
        let second = scan_directory_fingerprint(&files_dir, false).expect("failed to fingerprint");

        assert_ne!(first, second);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
