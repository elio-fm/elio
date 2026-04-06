use crate::core::{Entry, EntryKind, SortMode};
use anyhow::{Context, Result};
#[cfg(test)]
use std::cell::RefCell;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::{
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
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

// ---------------------------------------------------------------------------
// Restore from trash
// ---------------------------------------------------------------------------

/// Restores a trashed item to its original location.
///
/// Two backends are supported:
///
/// - **FreeDesktop trash** (Linux, BSD, and any macOS installation that uses
///   XDG tools): `entry_path` must be inside a `Trash/files/` directory and a
///   sibling `Trash/info/<name>.trashinfo` file must exist.  The original path
///   is read from that file and the item is moved back.
///
/// - **macOS `~/.Trash`**: Finder records the original location internally
///   when an item is trashed.  The `osascript` "put back" command asks Finder
///   to use that metadata and move the item back — exactly what the Finder
///   "Put Back" menu item does.
///
/// The FreeDesktop path is tried first (it works even on macOS if the XDG
/// layout happens to be present), then the macOS path, then an unsupported
/// error for any other layout (e.g. Windows Recycle Bin).
pub(crate) fn restore_trash_item(entry_path: &Path) -> anyhow::Result<()> {
    // FreeDesktop trash layout: the entry lives inside a `files/` directory,
    // and a sibling `info/` directory holds the `.trashinfo` metadata.
    //
    // Both conditions are required.  Checking only for `info/` two levels up
    // is insufficient: on macOS, `~/.Trash/foo` would compute `~/info`, and
    // if the user happens to have a `~/info` directory for any reason the
    // function would take the FreeDesktop path and fail to find a `.trashinfo`
    // instead of correctly falling through to the Finder backend.
    let parent = entry_path.parent();
    let in_files_dir = parent
        .and_then(|p| p.file_name())
        .is_some_and(|name| name == "files");
    let info_dir = parent
        .and_then(|p| p.parent())
        .map(|trash_root| trash_root.join("info"));

    if in_files_dir && info_dir.as_deref().is_some_and(|d| d.is_dir()) {
        return restore_trash_item_freedesktop(entry_path, info_dir.unwrap());
    }

    // macOS: no .trashinfo metadata, but Finder tracks the original location
    // internally.  Ask it to "put back" the item via osascript.
    #[cfg(target_os = "macos")]
    return restore_trash_item_macos(entry_path);

    // Any other layout (e.g. Windows Recycle Bin) is not supported.
    #[cfg(not(target_os = "macos"))]
    anyhow::bail!("restore is not supported for this trash location")
}

/// FreeDesktop-specific restore: reads the `.trashinfo` sidecar and moves the
/// item back to its original path.
fn restore_trash_item_freedesktop(
    entry_path: &Path,
    info_dir: PathBuf,
) -> anyhow::Result<()> {
    let name = entry_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("cannot determine file name for {:?}", entry_path))?;

    let info_path = info_dir.join(format!("{name}.trashinfo"));
    let content =
        fs::read_to_string(&info_path).with_context(|| format!("cannot read {:?}", info_path))?;

    let original = parse_trashinfo_original_path(&content)
        .ok_or_else(|| anyhow::anyhow!("cannot parse original path from {:?}", info_path))?;

    if original.exists() {
        anyhow::bail!("destination already exists: {:?}", original);
    }

    if let Some(parent) = original.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create parent dir {:?}", parent))?;
    }

    fs::rename(entry_path, &original)
        .with_context(|| format!("cannot move {:?} to {:?}", entry_path, original))?;

    let _ = fs::remove_file(&info_path);

    Ok(())
}

// ---------------------------------------------------------------------------
// macOS restore-origins store
// ---------------------------------------------------------------------------
// Elio trashes files via the `trash` crate, which calls
// NSWorkspace.recycleURLs.  That API stores the original path in a private
// system database that Finder reads for "Put Back" — it does NOT reliably
// write ptbL/ptbN records to ~/.Trash/.DS_Store the way Finder's own drag-
// to-trash action does.  Parsing .DS_Store therefore fails for any file Elio
// trashed, even though Finder's own "Put Back" works fine for those files.
//
// To work around this, whenever Elio trashes a file it immediately records
// the original path in its own JSON store at
//   ~/Library/Application Support/elio/trash-origins.json
// keyed by the expected filename in ~/.Trash.  Restore checks this store
// first.  The DS_Store parser is kept as a fallback for files trashed
// directly by Finder (which does write ptbL).

/// Returns the path to the restore-origins metadata store.
#[cfg(target_os = "macos")]
fn restore_origins_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("elio").join("trash-origins.json"))
}

/// Records `(trash_name, original_path)` pairs in the restore-origins store.
/// `trash_name` is the filename as it will appear in `~/.Trash` (= the
/// original filename when there is no collision).  Best-effort: silently
/// ignored on any I/O error.
#[cfg(target_os = "macos")]
pub(crate) fn save_restore_origins(items: &[(String, PathBuf)]) {
    let Some(path) = restore_origins_path() else {
        return;
    };
    let mut map: std::collections::HashMap<String, String> = fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();

    for (name, original) in items {
        if let Some(s) = original.to_str() {
            map.insert(name.clone(), s.to_owned());
        }
    }

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec_pretty(&map) {
        let _ = fs::write(&path, json);
    }
}

/// Looks up the original path for a file currently named `trash_name`.
/// Also tries stripping macOS collision suffixes (` 2`, ` 3`, …) from the
/// stem, so files renamed on collision can still be matched.
#[cfg(target_os = "macos")]
fn load_restore_origin(trash_name: &str) -> Option<PathBuf> {
    let path = restore_origins_path()?;
    let map: std::collections::HashMap<String, String> =
        serde_json::from_slice(&fs::read(&path).ok()?).ok()?;

    if let Some(orig) = map.get(trash_name) {
        return Some(PathBuf::from(orig));
    }

    // Collision case: "foo 2.txt" → look up "foo.txt".
    let p = Path::new(trash_name);
    let stem = p.file_stem().and_then(|s| s.to_str())?;
    let ext = p.extension().and_then(|e| e.to_str());
    let base_stem = strip_macos_collision_suffix(stem)?;
    let base_name = match ext {
        Some(e) => format!("{base_stem}.{e}"),
        None => base_stem.to_owned(),
    };
    map.get(&base_name).map(|s| PathBuf::from(s))
}

/// Strips a macOS collision suffix (` 2`, ` 3`, …) from a file stem.
/// Returns `Some(base)` if a suffix was stripped, `None` otherwise.
#[cfg(target_os = "macos")]
fn strip_macos_collision_suffix(stem: &str) -> Option<&str> {
    let (base, suffix) = stem.rsplit_once(' ')?;
    let n: u64 = suffix.parse().ok()?;
    (n >= 2).then_some(base)
}

/// Moves `entry_path` to `original_path`, creating parent directories as
/// needed.  Shared by both restore paths (our store and DS_Store fallback).
#[cfg(target_os = "macos")]
fn perform_restore(entry_path: &Path, original_path: &Path) -> anyhow::Result<()> {
    if original_path.exists() {
        anyhow::bail!("destination already exists: {:?}", original_path);
    }
    if let Some(parent) = original_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create parent dir {:?}", parent))?;
    }
    fs::rename(entry_path, original_path)
        .with_context(|| format!("cannot move {:?} to {:?}", entry_path, original_path))
}

/// macOS-specific restore.  Checks the Elio restore-origins store first
/// (populated whenever Elio trashes a file), then falls back to parsing
/// `.DS_Store` for files trashed directly by Finder.
#[cfg(target_os = "macos")]
fn restore_trash_item_macos(entry_path: &Path) -> anyhow::Result<()> {
    let file_name = entry_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("cannot determine file name for {:?}", entry_path))?;
    let trash_dir = entry_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot determine trash dir for {:?}", entry_path))?;
    let ds_store_path = trash_dir.join(".DS_Store");

    // Guard: never treat the metadata file itself as the item to restore.
    if entry_path == ds_store_path {
        anyhow::bail!("cannot restore \".DS_Store\" — it is a system metadata file");
    }

    // ── Primary: our own restore-origins store ──────────────────────────────
    if let Some(original_path) = load_restore_origin(file_name) {
        return perform_restore(entry_path, &original_path);
    }

    // ── Fallback: parse .DS_Store written by Finder ─────────────────────────
    if !ds_store_path.exists() {
        anyhow::bail!(
            "no Put Back metadata found for \"{file_name}\" \
             (the file was not trashed via Finder or Elio)"
        );
    }

    let data = fs::read(&ds_store_path)
        .with_context(|| format!("cannot read {:?}", ds_store_path))?;

    let (parent_dir, original_name) =
        macos_ds_store_find_ptb(&data, file_name).ok_or_else(|| {
            anyhow::anyhow!(
                "no Put Back metadata found for \"{file_name}\" \
                 (the file was not trashed via Finder or Elio)"
            )
        })?;

    // ptbL stores a volume-relative path without a leading slash.
    let original_path = if parent_dir.is_empty() {
        PathBuf::from(format!("/{original_name}"))
    } else {
        PathBuf::from(format!("/{parent_dir}/{original_name}"))
    };

    perform_restore(entry_path, &original_path)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// macOS DS_Store parser
// ---------------------------------------------------------------------------
// When Finder moves a file to the Trash it writes `ptbL` (original parent
// directory, volume-relative, no leading slash) and `ptbN` (original file
// name, when renamed on collision) into the `.DS_Store` file in `~/.Trash`.
// These are the same records that Finder's "Put Back" command consults.
//
// DS_Store uses a buddy allocator to store a B-tree of
// (filename, property, type, value) records.  We parse just enough to locate
// ptbL/ptbN for the target filename without pulling in an external dependency.
// ---------------------------------------------------------------------------

/// Searches a `.DS_Store` binary for the `ptbL` (original parent directory)
/// and `ptbN` (original file name) records associated with `file_name`.
///
/// Returns `(parent_dir, original_name)` on success, where `parent_dir` is
/// volume-relative (no leading slash).  Returns `None` if the records are not
/// found or the binary cannot be parsed.
#[cfg(target_os = "macos")]
fn macos_ds_store_find_ptb(data: &[u8], file_name: &str) -> Option<(String, String)> {
    // ── Buddy-allocator header ──────────────────────────────────────────────
    // data[0..4]  — alignment marker \x00\x00\x00\x01
    // data[4..8]  — "Bud1" magic
    // data[8..12] — info_offset (u32 BE, relative to data[4..])
    // data[12..16]— info_size   (u32 BE)
    if data.len() < 36 || &data[4..8] != b"Bud1" {
        return None;
    }
    let info_offset = u32::from_be_bytes(data[8..12].try_into().ok()?) as usize;
    let info_size = u32::from_be_bytes(data[12..16].try_into().ok()?) as usize;

    let info_start = 4usize.checked_add(info_offset)?;
    let info_end = info_start.checked_add(info_size)?;
    if info_end > data.len() || info_end < info_start + 8 {
        return None;
    }
    let info = &data[info_start..info_end];

    // ── Offset table ────────────────────────────────────────────────────────
    // info[0..4]  — num_offsets (u32 BE)
    // info[4..8]  — 0x00000000 (padding)
    // info[8..]   — num_offsets × u32 BE block addresses
    let num_offsets = u32::from_be_bytes(info[0..4].try_into().ok()?) as usize;
    let table_bytes = num_offsets.checked_mul(4)?;
    let table_end = 8usize.checked_add(table_bytes)?;
    if table_end > info.len() {
        return None;
    }
    let mut offsets = Vec::with_capacity(num_offsets);
    for i in 0..num_offsets {
        let o = 8 + i * 4;
        offsets.push(u32::from_be_bytes(info[o..o + 4].try_into().ok()?));
    }

    // Pad offset table to next 256-entry boundary.
    let pad = (256usize.wrapping_sub(num_offsets % 256)) % 256;
    let toc_start = table_end.checked_add(pad.checked_mul(4)?)?;

    // ── Table of Contents ───────────────────────────────────────────────────
    // toc[0..4]  — num_entries (u32 BE)
    // toc[4..]   — entries: name_len (u8) + name + block_id (u32 BE)
    if toc_start + 4 > info.len() {
        return None;
    }
    let num_toc =
        u32::from_be_bytes(info[toc_start..toc_start + 4].try_into().ok()?) as usize;

    let mut pos = toc_start + 4;
    let mut dsdb_block_id: Option<u32> = None;
    for _ in 0..num_toc {
        if pos >= info.len() {
            return None;
        }
        let name_len = info[pos] as usize;
        pos += 1;
        let name_end = pos.checked_add(name_len)?;
        if name_end + 4 > info.len() {
            return None;
        }
        let toc_name = std::str::from_utf8(&info[pos..name_end]).ok()?;
        let block_id =
            u32::from_be_bytes(info[name_end..name_end + 4].try_into().ok()?);
        if toc_name == "DSDB" {
            dsdb_block_id = Some(block_id);
        }
        pos = name_end + 4;
    }

    // ── DSDB block → root B-tree node ───────────────────────────────────────
    let dsdb_block = ds_store_block(data, &offsets, dsdb_block_id?)?;
    if dsdb_block.len() < 4 {
        return None;
    }
    let root_node = u32::from_be_bytes(dsdb_block[0..4].try_into().ok()?);

    // ── Traverse B-tree ─────────────────────────────────────────────────────
    let mut ptbl: Option<String> = None;
    let mut ptbn: Option<String> = None;
    let mut visited = std::collections::HashSet::new();
    ds_store_traverse(
        data,
        &offsets,
        root_node,
        file_name,
        &mut ptbl,
        &mut ptbn,
        &mut visited,
    )?;

    match (ptbl, ptbn) {
        (Some(l), Some(n)) => Some((l, n)),
        // ptbN is absent when the file name was not changed on trashing.
        (Some(l), None) => Some((l, file_name.to_owned())),
        _ => None,
    }
}

/// Returns the payload slice for the given block ID, or `None` on any error.
///
/// Block address encoding: `offset = addr & !0x1f` (absolute in `data`),
/// `size = 1 << (addr & 0x1f)`.  The 4 bytes at `data[offset..]` are a
/// block size header; the payload starts at `data[offset + 4..]`.
#[cfg(target_os = "macos")]
fn ds_store_block<'a>(data: &'a [u8], offsets: &[u32], id: u32) -> Option<&'a [u8]> {
    let addr = *offsets.get(id as usize)?;
    if addr == 0 {
        return None;
    }
    let offset = (addr & !0x1f) as usize;
    let size = 1usize << (addr & 0x1f);
    let start = offset.checked_add(4)?;
    let end = start.checked_add(size)?;
    if end > data.len() {
        return None;
    }
    Some(&data[start..end])
}

/// Recursively traverses a B-tree node, collecting `ptbL`/`ptbN` values for
/// `target_name`.  Returns `None` on any parse error.
#[cfg(target_os = "macos")]
fn ds_store_traverse(
    data: &[u8],
    offsets: &[u32],
    node_id: u32,
    target_name: &str,
    ptbl: &mut Option<String>,
    ptbn: &mut Option<String>,
    visited: &mut std::collections::HashSet<u32>,
) -> Option<()> {
    // Guard against cycles in corrupt DS_Store files — skip silently, don't abort.
    if !visited.insert(node_id) {
        return Some(());
    }

    let block = ds_store_block(data, offsets, node_id)?;
    let mut cur = DsStoreCursor::new(block);

    let pair_count = cur.read_u32()?;

    if pair_count == 0 {
        // Leaf node: record count then records.
        let record_count = cur.read_u32()?;
        for _ in 0..record_count {
            // Unknown type in a record means we can't determine its size and
            // must stop reading this node, but don't abort the whole traversal.
            if ds_store_read_record(&mut cur, target_name, ptbl, ptbn).is_none() {
                break;
            }
        }
    } else {
        // Internal node: alternating child_id and record, then one final child.
        for _ in 0..pair_count {
            let child_id = cur.read_u32()?;
            // Child failures don't corrupt our cursor — skip and continue.
            ds_store_traverse(data, offsets, child_id, target_name, ptbl, ptbn, visited);
            // Record failure means we can't find the boundary of this record,
            // so we can't safely continue reading this node.
            if ds_store_read_record(&mut cur, target_name, ptbl, ptbn).is_none() {
                return Some(());
            }
        }
        let last_child = cur.read_u32()?;
        ds_store_traverse(data, offsets, last_child, target_name, ptbl, ptbn, visited);
    }

    Some(())
}

/// Reads one B-tree record and, if it belongs to `target_name`, stores the
/// `ptbL` or `ptbN` value.  Returns `None` on any parse error.
#[cfg(target_os = "macos")]
fn ds_store_read_record(
    cur: &mut DsStoreCursor<'_>,
    target_name: &str,
    ptbl: &mut Option<String>,
    ptbn: &mut Option<String>,
) -> Option<()> {
    // Filename: u32 code-unit count + UTF-16BE data.
    let name_len = cur.read_u32()? as usize;
    let name_bytes = cur.read_bytes(name_len * 2)?;
    let name = decode_utf16be(name_bytes)?;

    // Property code and type code (4 ASCII bytes each).
    let prop4: [u8; 4] = cur.read_bytes(4)?.try_into().ok()?;
    let typ4: [u8; 4] = cur.read_bytes(4)?.try_into().ok()?;

    let is_target = name == target_name;
    let is_ptbl = prop4 == *b"ptbL";
    let is_ptbn = prop4 == *b"ptbN";

    match (&prop4, &typ4) {
        (_, b"ustr") => {
            let val_len = cur.read_u32()? as usize;
            let val_bytes = cur.read_bytes(val_len * 2)?;
            if is_target && (is_ptbl || is_ptbn) {
                let val = decode_utf16be(val_bytes)?;
                if is_ptbl {
                    *ptbl = Some(val);
                } else {
                    *ptbn = Some(val);
                }
            }
        }
        (_, b"bool") => {
            cur.skip(1)?;
        }
        (_, b"shor") => {
            cur.skip(2)?;
        }
        (_, b"long") | (_, b"type") => {
            cur.skip(4)?;
        }
        (_, b"comp") | (_, b"dutc") => {
            cur.skip(8)?;
        }
        // BKGD blob has no length prefix — it is always exactly 12 bytes.
        (b"BKGD", b"blob") => {
            cur.skip(12)?;
        }
        (_, b"blob") => {
            let len = cur.read_u32()? as usize;
            cur.skip(len)?;
        }
        _ => {
            // Unknown type — cannot determine record size, so abort traversal.
            return None;
        }
    }

    Some(())
}

/// Cursor over a `&[u8]` slice with big-endian integer reads.
#[cfg(target_os = "macos")]
struct DsStoreCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

#[cfg(target_os = "macos")]
impl<'a> DsStoreCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        let end = self.pos.checked_add(n)?;
        if end > self.data.len() {
            return None;
        }
        self.pos = end;
        Some(())
    }

    fn read_u32(&mut self) -> Option<u32> {
        let b = self.read_bytes(4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        if end > self.data.len() {
            return None;
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Some(slice)
    }
}

/// Decodes a UTF-16BE byte sequence into a `String`.
/// Returns `None` if the byte count is odd or the data is not valid UTF-16.
#[cfg(target_os = "macos")]
fn decode_utf16be(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units).ok()
}

fn parse_trashinfo_original_path(content: &str) -> Option<PathBuf> {
    for line in content.lines() {
        if let Some(encoded) = line.trim().strip_prefix("Path=") {
            return Some(PathBuf::from(percent_decode(encoded)));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2]))
        {
            out.push(hi << 4 | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
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

    // ── restore_trash_item_freedesktop ────────────────────────────────────────

    /// Builds a minimal FreeDesktop trash layout under `root`:
    ///   root/
    ///     files/<name>  ← the trashed item (a regular file)
    ///     info/<name>.trashinfo
    ///
    /// Returns `(trash_files_dir, trash_info_dir, item_path)`.
    #[cfg(unix)]
    fn make_freedesktop_trash(
        root: &PathBuf,
        name: &str,
        original: &Path,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let files_dir = root.join("files");
        let info_dir = root.join("info");
        fs::create_dir_all(&files_dir).expect("failed to create trash files dir");
        fs::create_dir_all(&info_dir).expect("failed to create trash info dir");
        let item_path = files_dir.join(name);
        fs::write(&item_path, b"trashed content").expect("failed to write trashed item");
        let trashinfo = format!(
            "[Trash Info]\nPath={}\nDeletionDate=2024-01-01T00:00:00\n",
            original.to_str().unwrap()
        );
        fs::write(info_dir.join(format!("{name}.trashinfo")), trashinfo)
            .expect("failed to write trashinfo");
        (files_dir, info_dir, item_path)
    }

    #[test]
    #[cfg(unix)]
    fn restore_freedesktop_moves_item_to_original_path_and_removes_trashinfo() {
        let root = temp_path("restore-fd-ok");
        let restore_target = temp_path("restore-fd-ok-dest");
        fs::create_dir_all(&root).expect("failed to create trash root");
        fs::create_dir_all(&restore_target).expect("failed to create restore target dir");

        let original = restore_target.join("report.pdf");
        let (_, info_dir, item_path) =
            make_freedesktop_trash(&root, "report.pdf", &original);

        let result = restore_trash_item(&item_path);
        assert!(result.is_ok(), "restore should succeed: {:?}", result);
        assert!(original.exists(), "file should be at original location");
        assert!(!item_path.exists(), "trashed item should be gone");
        assert!(
            !info_dir.join("report.pdf.trashinfo").exists(),
            "trashinfo should be removed"
        );

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&restore_target).ok();
    }

    #[test]
    #[cfg(unix)]
    fn restore_freedesktop_fails_when_destination_already_exists() {
        let root = temp_path("restore-fd-conflict");
        let restore_target = temp_path("restore-fd-conflict-dest");
        fs::create_dir_all(&root).expect("failed to create trash root");
        fs::create_dir_all(&restore_target).expect("failed to create restore target dir");

        let original = restore_target.join("conflict.txt");
        fs::write(&original, b"already here").expect("failed to write blocking file");

        let (_, _, item_path) =
            make_freedesktop_trash(&root, "conflict.txt", &original);

        let err = restore_trash_item(&item_path).unwrap_err();
        assert!(
            err.to_string().contains("destination already exists"),
            "unexpected error: {err}"
        );

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&restore_target).ok();
    }

    #[test]
    #[cfg(unix)]
    fn restore_freedesktop_fails_when_trashinfo_is_missing() {
        let root = temp_path("restore-fd-no-info");
        let files_dir = root.join("files");
        let info_dir = root.join("info");
        fs::create_dir_all(&files_dir).expect("failed to create files dir");
        fs::create_dir_all(&info_dir).expect("failed to create info dir");

        let item_path = files_dir.join("orphan.txt");
        fs::write(&item_path, b"no metadata").expect("failed to write orphan item");
        // Deliberately do NOT write a .trashinfo file.

        let err = restore_trash_item(&item_path).unwrap_err();
        assert!(
            err.to_string().contains("orphan.txt.trashinfo"),
            "error should mention the missing trashinfo, got: {err}"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn restore_fails_for_path_outside_any_known_trash_layout() {
        // A path with no info/ sibling on a non-macOS platform bails with the
        // "not supported" message rather than a confusing I/O error.
        let tmp = temp_path("restore-unsupported");
        fs::create_dir_all(&tmp).expect("failed to create temp dir");
        let fake_item = tmp.join("item.txt");
        fs::write(&fake_item, b"content").expect("failed to write item");

        // We're testing the non-macOS bail path, so guard accordingly.
        #[cfg(not(target_os = "macos"))]
        {
            let err = restore_trash_item(&fake_item).unwrap_err();
            assert!(
                err.to_string().contains("not supported"),
                "unexpected error: {err}"
            );
        }

        fs::remove_dir_all(&tmp).ok();
    }

    /// Regression test for false-positive FreeDesktop detection.
    ///
    /// On macOS, `~/.Trash/foo` computes `~/info` as the candidate info dir.
    /// If the user happens to have a `~/info` directory, the old code would
    /// take the FreeDesktop path and then fail looking for a `.trashinfo` file
    /// instead of falling through to the Finder backend.
    ///
    /// The fix requires the entry's immediate parent to be named `files` before
    /// treating the layout as FreeDesktop, so a `~/.Trash`-style path is never
    /// misidentified even when a coincidental `info/` exists nearby.
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn restore_does_not_misdetect_freedesktop_when_info_dir_exists_at_wrong_level() {
        // Build: root/Trash/foo  and  root/info/  (the decoy)
        // `foo`'s parent is `Trash` (not `files`), so the FreeDesktop path
        // must NOT be taken even though root/info/ exists.
        let root = temp_path("restore-false-positive");
        let trash_dir = root.join("Trash");
        let decoy_info = root.join("info");
        fs::create_dir_all(&trash_dir).expect("failed to create trash dir");
        fs::create_dir_all(&decoy_info).expect("failed to create decoy info dir");

        let item_path = trash_dir.join("foo.txt");
        fs::write(&item_path, b"content").expect("failed to write item");

        let err = restore_trash_item(&item_path).unwrap_err();
        assert!(
            err.to_string().contains("not supported"),
            "should bail as unsupported, not attempt FreeDesktop restore: {err}"
        );

        fs::remove_dir_all(&root).ok();
    }

    // ── macOS DS_Store restore helpers ────────────────────────────────────────

    #[test]
    #[cfg(target_os = "macos")]
    fn decode_utf16be_decodes_ascii_string() {
        // "Hi" as UTF-16BE: U+0048, U+0069
        let bytes = b"\x00H\x00i";
        assert_eq!(decode_utf16be(bytes), Some("Hi".to_string()));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn decode_utf16be_decodes_non_ascii() {
        // "é" (U+00E9) as UTF-16BE
        let bytes = b"\x00\xe9";
        assert_eq!(decode_utf16be(bytes), Some("é".to_string()));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn decode_utf16be_rejects_odd_byte_count() {
        assert_eq!(decode_utf16be(b"\x00H\x00"), None);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn decode_utf16be_empty_slice_gives_empty_string() {
        assert_eq!(decode_utf16be(b""), Some(String::new()));
    }
}
