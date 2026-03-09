use super::*;
use anyhow::{Context, Result};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::{
    cmp::Ordering,
    env,
    ffi::OsStr,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

const PREVIEW_LIMIT_BYTES: usize = 8 * 1024;
const PREVIEW_MAX_LINES: usize = 24;

pub(super) fn build_preview(entry: &Entry) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            entry.badge().to_string(),
            Style::default()
                .fg(folder_color(entry))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            entry.name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(format!("Type: {}", entry.kind_label())));
    lines.push(Line::from(format!("Size: {}", format_size(entry.size))));
    lines.push(Line::from(format!(
        "Modified: {}",
        entry
            .modified
            .map(format_time_ago)
            .unwrap_or_else(|| "unknown".to_string())
    )));
    lines.push(Line::from(format!(
        "Permissions: {}",
        if entry.readonly {
            "readonly"
        } else {
            "read/write"
        }
    )));
    lines.push(Line::from(format!(
        "Hidden: {}",
        if entry.hidden { "yes" } else { "no" }
    )));
    lines.push(Line::from(String::new()));

    if entry.is_dir() {
        lines.push(Line::from(Span::styled(
            "Contents",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        match fs::read_dir(&entry.path) {
            Ok(children) => {
                let mut count = 0usize;
                for child in children
                    .flatten()
                    .take(PREVIEW_MAX_LINES.saturating_sub(lines.len()))
                {
                    count += 1;
                    let name = child.file_name().to_string_lossy().to_string();
                    lines.push(Line::from(format!("• {}", name)));
                }
                if count == 0 {
                    lines.push(Line::from("Folder is empty"));
                }
            }
            Err(_) => lines.push(Line::from("Folder preview unavailable")),
        }
        return lines;
    }

    lines.push(Line::from(Span::styled(
        "Preview",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    match read_text_preview(&entry.path) {
        Ok(Some(text)) => {
            for line in text
                .lines()
                .take(PREVIEW_MAX_LINES.saturating_sub(lines.len()))
            {
                lines.push(Line::from(line.to_string()));
            }
            if lines.len() <= 7 {
                lines.push(Line::from("File is empty"));
            }
        }
        Ok(None) => lines.push(Line::from("Binary file or unsupported text encoding")),
        Err(_) => lines.push(Line::from("Preview unavailable")),
    }
    lines
}

pub(crate) fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

pub(crate) fn format_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", size, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
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

pub(crate) fn folder_color(entry: &Entry) -> Color {
    match entry.kind {
        EntryKind::Directory => Color::Rgb(65, 143, 222),
        EntryKind::File => match extension_class(&entry.path) {
            "image" => Color::Rgb(86, 156, 214),
            "audio" => Color::Rgb(138, 110, 214),
            "video" => Color::Rgb(204, 112, 79),
            "archive" => Color::Rgb(191, 142, 74),
            "code" => Color::Rgb(76, 152, 120),
            _ => Color::Rgb(98, 109, 122),
        },
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
        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else {
            EntryKind::File
        };
        let size = if metadata.is_file() { metadata.len() } else { 0 };
        entries.push(Entry {
            path,
            name,
            name_key,
            kind,
            size,
            modified: metadata.modified().ok(),
            readonly: metadata.permissions().readonly(),
            hidden,
        });
    }
    Ok(entries)
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

pub(super) fn extension_class(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" | "js" | "ts" | "tsx" | "jsx" | "py" | "go" | "c" | "cpp" | "h" | "java" | "json"
        | "toml" | "yaml" | "yml" | "md" | "sh" => "code",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "avif" => "image",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "audio",
        "mp4" | "mkv" | "mov" | "webm" | "avi" => "video",
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" => "archive",
        "txt" | "log" | "ini" | "csv" => "text",
        _ => "file",
    }
}

fn read_text_preview(path: &Path) -> Result<Option<String>> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0; PREVIEW_LIMIT_BYTES];
    let count = file.read(&mut buffer)?;
    buffer.truncate(count);

    if buffer.is_empty() {
        return Ok(Some(String::new()));
    }
    if buffer.contains(&0) {
        return Ok(None);
    }

    match String::from_utf8(buffer) {
        Ok(text) => Ok(Some(text)),
        Err(_) => Ok(None),
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
        assert_eq!(format_size(2048), "2.0 KB");
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
