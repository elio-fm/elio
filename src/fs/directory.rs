use crate::core::{Entry, EntryKind, SortMode};
use anyhow::{Context, Result};
#[cfg(test)]
use std::cell::RefCell;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(test)]
use std::path::PathBuf;
use std::{
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io,
    path::Path,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
thread_local! {
    static TEST_OPEN_IN_SYSTEM_CAPTURE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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

/// Reads the `DeletionDate` from a `.trashinfo` file inside `info_dir` for the given file name.
fn read_trash_deletion_date(info_dir: &Path, name: &str) -> Option<SystemTime> {
    let info_path = info_dir.join(format!("{name}.trashinfo"));
    let content = fs::read_to_string(info_path).ok()?;
    for line in content.lines() {
        if let Some(date_str) = line.trim().strip_prefix("DeletionDate=") {
            return parse_trash_deletion_date(date_str);
        }
    }
    None
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

fn read_entries(dir: &Path, show_hidden: bool) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
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
    let mut entries = read_entries(dir, show_hidden)?;

    // If this is a freedesktop trash `files/` directory (recognised by the presence of a
    // sibling `info/` directory), replace each entry's modification time with the deletion
    // date stored in the corresponding `.trashinfo` file so the listing shows "trashed X ago"
    // rather than the file's own last-modified timestamp.
    if dir.file_name().is_some_and(|n| n == "files")
        && let Some(info_dir) = dir.parent().map(|p| p.join("info")).filter(|p| p.is_dir())
    {
        for entry in &mut entries {
            if let Some(date) = read_trash_deletion_date(&info_dir, &entry.name) {
                entry.modified = Some(date);
            }
        }
    }

    sort_entries(&mut entries, sort_mode);
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

pub(crate) fn scan_directory_fingerprint(
    dir: &Path,
    show_hidden: bool,
) -> Result<DirectoryFingerprint> {
    let mut parts = Vec::new();
    for item in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
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
        parts.push(FingerprintPart {
            name: file_name.to_string_lossy().to_lowercase(),
            kind: details.kind,
            size: details.size,
            modified: fingerprint_time(details.modified),
            readonly: details.readonly,
        });
    }
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

pub(crate) fn open_in_system(target: &Path) -> Result<(), String> {
    #[cfg(test)]
    if let Some(capture) = TEST_OPEN_IN_SYSTEM_CAPTURE.with(|slot| slot.borrow().clone()) {
        return fs::write(&capture, target.display().to_string()).map_err(|e| e.to_string());
    }

    #[cfg(target_os = "macos")]
    {
        detached_open("open", &[], target).map_err(|e| format!("open: {e}"))
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

        Command::new("cmd")
            .args(["/c", "start", ""])
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("cmd: {e}"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        open_with_unix_backends(target, &[("xdg-open", &[][..]), ("gio", &["open"][..])])
    }
}

#[cfg(test)]
pub(crate) fn set_open_in_system_capture_for_test(path: Option<PathBuf>) {
    TEST_OPEN_IN_SYSTEM_CAPTURE.with(|slot| *slot.borrow_mut() = path);
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_with_unix_backends(target: &Path, backends: &[(&str, &[&str])]) -> Result<(), String> {
    for &(program, args) in backends {
        match detached_open(program, args, target) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("{program}: {e}")),
        }
    }
    Err(String::from("No desktop opener available in this session"))
}

pub(crate) fn detached_open(program: &str, args: &[&str], target: &Path) -> io::Result<()> {
    let mut command = Command::new(program);
    command.args(args);
    command.arg(target);

    #[cfg(target_os = "macos")]
    if program == "open" {
        return status_spawn(&mut command);
    }

    detached_spawn(&mut command)
}

/// Spawns `program` with the given `args` detached from the terminal.
/// Unlike [`detached_open`], the target path is not appended — it must
/// already be present in `args` (as produced by the Exec= expansion).
pub(crate) fn detached_open_command(program: &str, args: &[String]) -> io::Result<()> {
    let mut command = Command::new(program);
    command.args(args);

    #[cfg(target_os = "macos")]
    if program == "open" {
        return status_spawn(&mut command);
    }

    detached_spawn(&mut command)
}

fn detached_spawn(command: &mut Command) -> io::Result<()> {
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    command.spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn status_spawn(command: &mut Command) -> io::Result<()> {
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("process exited with {status}")))
    }
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

    /// Wraps `s` in single quotes, escaping any embedded single quotes so the
    /// result is safe to embed in a POSIX shell command string even when `s`
    /// contains apostrophes (e.g. a TMPDIR like `/tmp/user's tmp`).
    ///
    /// Strategy: end the current single-quoted span, emit `'\''`, then re-open.
    /// `foo'bar` → `'foo'\''bar'`
    #[cfg(unix)]
    fn shell_quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', r"'\''"))
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

        let entries = read_entries(&root, false).expect("failed to read entries");
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

        let entries = read_entries(&root, false).expect("failed to read entries");
        let linked = entries
            .iter()
            .find(|entry| entry.name == "linked-dir")
            .expect("linked dir should be present");

        assert!(linked.is_dir());
        assert_eq!(linked.size, 0);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    #[cfg(unix)]
    fn detached_open_moves_child_into_its_own_process_group() {
        let root = temp_path("detached-open");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let capture = root.join("capture.txt");
        // Use /bin/sh -c with the capture path interpolated directly into the
        // command string.  Passing it via $1 relies on how the target shell
        // (e.g. FreeBSD sh) handles the positional-parameter slot after "-c cmd",
        // which varies across implementations.  The path comes from temp_path()
        // and contains only alphanumerics, hyphens, and slashes — safe to
        // single-quote.  The target arg that detached_open appends becomes $0
        // (the script name) and is harmlessly ignored.
        let capture_str = capture
            .to_str()
            .expect("capture path should be valid utf-8");
        let cmd = format!(
            "pgid=$(ps -o pgid= -p $$ | tr -d ' '); printf '%s %s\\n' \"$$\" \"$pgid\" > {}",
            shell_quote(capture_str)
        );
        detached_open("/bin/sh", &["-c", &cmd], &root).expect("failed to spawn fake opener");

        // Wait for non-empty content — the shell's `>` redirect creates the
        // file before printf writes to it, so existence alone is not enough.
        let mut capture_text = String::new();
        for _ in 0..300 {
            match fs::read_to_string(&capture) {
                Ok(s) if !s.is_empty() => {
                    capture_text = s;
                    break;
                }
                _ => {}
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let mut parts = capture_text.split_whitespace();
        let pid = parts
            .next()
            .expect("capture should contain pid")
            .parse::<i32>()
            .expect("pid should be numeric");
        let pgid = parts
            .next()
            .expect("capture should contain pgid")
            .parse::<i32>()
            .expect("pgid should be numeric");

        assert_eq!(pgid, pid);
        assert_ne!(pgid, unsafe { libc::getpgrp() });

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
    #[cfg(all(unix, not(target_os = "macos")))]
    fn open_with_unix_backends_uses_first_available_backend() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_path("open-backends-first");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let capture = root.join("capture.txt");

        let script = root.join("fake-xdg-open");
        fs::write(&script, "#!/bin/sh\nprintf 'xdg-open' > \"$1\"\n")
            .expect("failed to write script");
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();

        let result = open_with_unix_backends(
            &capture,
            &[
                (script.to_str().unwrap(), &[][..]),
                ("this-program-does-not-exist-elio", &[][..]),
            ],
        );

        assert!(result.is_ok(), "expected Ok, got {result:?}");

        // Wait for the script to finish writing. The shell redirect `>` creates
        // the file (empty) before printf writes to it, so wait for non-empty
        // content to avoid a TOCTOU race on slow CI.
        for _ in 0..300 {
            match fs::read_to_string(&capture) {
                Ok(s) if !s.is_empty() => break,
                _ => {}
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let recorded = fs::read_to_string(&capture).expect("capture should exist");
        assert_eq!(recorded.trim(), "xdg-open");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn open_with_unix_backends_skips_missing_backend_and_tries_next() {
        let root = temp_path("open-backends-fallback");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let capture = root.join("capture.txt");

        // Use /bin/sh -c with the capture path baked into the command string.
        // Passing it via $1 relies on how each sh implementation populates
        // positional parameters after "-c cmd" — behaviour that differs between
        // Linux dash/bash and FreeBSD sh.  The path comes from temp_path() and
        // contains only alphanumerics, hyphens, and slashes — safe to
        // single-quote.
        let capture_str = capture
            .to_str()
            .expect("capture path should be valid utf-8");
        let cmd = format!("printf 'gio' > {}", shell_quote(capture_str));
        let result = open_with_unix_backends(
            &capture,
            &[
                ("this-program-does-not-exist-elio", &[][..]),
                ("/bin/sh", &["-c", &cmd][..]),
            ],
        );

        assert!(result.is_ok(), "expected Ok after fallback, got {result:?}");

        // Wait for non-empty content — the shell's `>` redirect creates the
        // file before printf writes to it, so existence alone is not enough.
        let mut recorded = String::new();
        for _ in 0..300 {
            match fs::read_to_string(&capture) {
                Ok(s) if !s.is_empty() => {
                    recorded = s;
                    break;
                }
                _ => {}
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert_eq!(recorded.trim(), "gio");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn open_with_unix_backends_returns_session_error_when_all_missing() {
        let result = open_with_unix_backends(
            Path::new("/tmp/anything"),
            &[
                ("this-program-does-not-exist-elio-a", &[][..]),
                ("this-program-does-not-exist-elio-b", &[][..]),
            ],
        );

        assert_eq!(
            result.unwrap_err(),
            "No desktop opener available in this session"
        );
    }

    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn open_with_unix_backends_propagates_non_notfound_errors_immediately() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_path("open-backends-permerror");
        fs::create_dir_all(&root).expect("failed to create temp root");

        // A file that exists but is not executable — spawn returns PermissionDenied.
        let not_executable = root.join("not-executable");
        fs::write(&not_executable, "#!/bin/sh\n").expect("failed to write file");
        let mut perms = fs::metadata(&not_executable).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&not_executable, perms).unwrap();

        let script = root.join("should-not-run");
        fs::write(&script, "#!/bin/sh\n").expect("failed to write script");

        let result = open_with_unix_backends(
            Path::new("/tmp/anything"),
            &[
                (not_executable.to_str().unwrap(), &[][..]),
                (script.to_str().unwrap(), &[][..]),
            ],
        );

        let err = result.unwrap_err();
        assert!(
            err.contains("not-executable"),
            "error should name the failing backend, got: {err}"
        );
        // The second backend should never have been tried.
        assert!(
            !err.contains("should-not-run"),
            "second backend should not appear in error, got: {err}"
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
