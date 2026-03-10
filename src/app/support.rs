use super::*;
use anyhow::{Context, Result};
use ratatui::layout::Rect;
use std::{
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    env,
    ffi::OsStr,
    fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct DirectoryFingerprint {
    pub digest: u64,
    pub entries: usize,
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

pub(crate) fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

pub(crate) fn format_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    if size < 1000 {
        return format!("{} B", format_with_grouping(size));
    }

    let mut value = size as f64;
    let mut unit = 0usize;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }

    let precision = if value < 10.0 {
        2
    } else if value < 100.0 {
        1
    } else {
        0
    };
    format!("{} {}", format_decimal(value, precision), UNITS[unit])
}

pub(crate) fn format_time_ago(time: SystemTime) -> String {
    let Ok(age) = SystemTime::now().duration_since(time) else {
        return "just now".to_string();
    };
    let seconds = age.as_secs();
    match seconds {
        0..=59 => format!("{seconds}s ago"),
        60..=3599 => format!("{}m ago", seconds / 60),
        3600..=86_399 => format!("{}h ago", seconds / 3600),
        86_400..=2_592_000 => format!("{}d ago", seconds / 86_400),
        _ => format!("{}mo ago", seconds / 2_592_000),
    }
}

pub(super) fn build_sidebar_items() -> Vec<SidebarItem> {
    let mut items = Vec::new();
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));

    items.push(SidebarItem {
        title: "Home".to_string(),
        icon: "󰋜",
        path: home.clone(),
    });

    for (title, folder, icon) in [
        ("Desktop", "Desktop", "󰟀"),
        ("Documents", "Documents", "󰈙"),
        ("Downloads", "Downloads", "󰉍"),
        ("Pictures", "Pictures", "󰉏"),
        ("Music", "Music", "󱍙"),
        ("Videos", "Videos", "󰕧"),
    ] {
        let path = home.join(folder);
        if path.exists() {
            items.push(SidebarItem {
                title: title.to_string(),
                icon,
                path,
            });
        }
    }

    items.push(SidebarItem {
        title: "Root".to_string(),
        icon: "󰋊",
        path: PathBuf::from("/"),
    });

    items
}

pub(super) fn read_entries(dir: &Path, show_hidden: bool) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let item = item?;
        let path = item.path();
        let file_name = item.file_name();
        let name = file_name.to_string_lossy().to_string();
        let name_key = name.to_lowercase();
        let hidden = is_hidden(file_name.as_os_str());
        if hidden && !show_hidden {
            continue;
        }

        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to read metadata for {}", path.display()))?;
        let details = entry_details(&path, &metadata);
        entries.push(Entry {
            path,
            name,
            name_key,
            kind: details.kind,
            size: details.size,
            modified: details.modified,
            readonly: details.readonly,
            hidden,
        });
    }
    Ok(entries)
}

pub(super) fn entries_fingerprint(entries: &[Entry]) -> DirectoryFingerprint {
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

pub(super) fn scan_directory_fingerprint(
    dir: &Path,
    show_hidden: bool,
) -> Result<DirectoryFingerprint> {
    let mut parts = Vec::new();
    for item in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let item = item?;
        let file_name = item.file_name();
        if is_hidden(file_name.as_os_str()) && !show_hidden {
            continue;
        }

        let metadata = match fs::symlink_metadata(item.path()) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
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

pub(super) fn sort_entries(entries: &mut [Entry], mode: SortMode) {
    entries.sort_by(|left, right| match (left.is_dir(), right.is_dir()) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => match mode {
            SortMode::Name => left.name_key.cmp(&right.name_key),
            SortMode::Modified => right
                .modified
                .cmp(&left.modified)
                .then_with(|| left.name_key.cmp(&right.name_key)),
            SortMode::Size => right
                .size
                .cmp(&left.size)
                .then_with(|| left.name_key.cmp(&right.name_key)),
        },
    });
}

fn is_hidden(file_name: &OsStr) -> bool {
    file_name.to_string_lossy().starts_with('.')
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

fn format_with_grouping(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped
}

fn format_decimal(value: f64, precision: usize) -> String {
    let mut formatted = format!("{value:.precision$}");
    if precision == 0 {
        return formatted;
    }

    while formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted
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

pub(super) fn detached_open(program: &str, args: &[&str], target: &Path) -> std::io::Result<()> {
    let mut command = Command::new(program);
    command.args(args);
    command.arg(target);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    command.spawn()?;
    Ok(())
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
        env::temp_dir().join(format!("elio-{label}-{unique}"))
    }

    #[test]
    fn reload_filters_hidden_files_by_default() {
        let root = temp_path("hidden");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("visible.txt"), "hello").expect("failed to write visible file");
        fs::write(root.join(".secret"), "hidden").expect("failed to write hidden file");

        let app = App::new_at(root.clone()).expect("failed to create app");
        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].name, "visible.txt");

        fs::remove_dir_all(root).expect("failed to remove temp root");
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
                hidden: false,
            },
            Entry {
                path: PathBuf::from("alpha"),
                name: "alpha".to_string(),
                name_key: "alpha".to_string(),
                kind: EntryKind::Directory,
                size: 0,
                modified: None,
                readonly: false,
                hidden: false,
            },
        ];

        sort_entries(&mut entries, SortMode::Name);
        assert!(entries[0].is_dir());
        assert!(!entries[1].is_dir());
    }

    #[test]
    fn size_format_is_human_readable() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2_048), "2.05 kB");
        assert_eq!(format_size(5_488), "5.49 kB");
        assert_eq!(format_size(12_345_678), "12.3 MB");
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
    fn lockfiles_are_classified_as_data() {
        assert_eq!(
            crate::appearance::classify_path(Path::new("poetry.lock"), EntryKind::File),
            FileClass::Data
        );
    }

    #[test]
    fn config_and_code_files_are_classified_separately() {
        assert_eq!(
            crate::appearance::classify_path(Path::new("starship.toml"), EntryKind::File),
            FileClass::Config
        );
        assert_eq!(
            crate::appearance::classify_path(Path::new("main.rs"), EntryKind::File),
            FileClass::Code
        );
    }

    #[test]
    fn zoom_is_clamped() {
        let root = temp_path("zoom");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("one.txt"), "hello").expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        assert_eq!(app.zoom_level, 1);

        app.adjust_zoom(10);
        assert_eq!(app.zoom_level, 2);

        app.adjust_zoom(-10);
        assert_eq!(app.zoom_level, 0);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
