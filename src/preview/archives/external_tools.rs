use super::archive_formats::ArchiveFormat;
use super::archive_rendering::{
    ArchiveEntry, ArchiveMetadata, normalize_archive_entries, normalize_archive_path,
};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const ARCHIVE_EXTERNAL_COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
const ARCHIVE_EXTERNAL_COMMAND_POLL: Duration = Duration::from_millis(20);
const SEVEN_ZIP_PROGRAMS: &[&str] = &["7z", "7zz", "7za"];

static ARCHIVE_OUTPUT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn parse_key_value_line(line: &str) -> Option<(&str, &str)> {
    if let Some((key, value)) = line.split_once(" = ") {
        return Some((key.trim(), value.trim()));
    }
    line.strip_suffix(" =").map(|key| (key.trim(), ""))
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

struct ArchiveCommandOutputFile {
    path: PathBuf,
}

impl ArchiveCommandOutputFile {
    fn create(program: &str) -> Option<(Self, File)> {
        let path = archive_command_output_path(program);
        let file = File::create(&path).ok()?;
        Some((Self { path }, file))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ArchiveCommandOutputFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) fn fallback_single_file_archive_entry(
    path: &Path,
    format: ArchiveFormat,
) -> Option<ArchiveEntry> {
    if !matches!(
        format,
        ArchiveFormat::Gzip | ArchiveFormat::Xz | ArchiveFormat::Bzip2 | ArchiveFormat::Zstd
    ) {
        return None;
    }

    let name = path.file_stem()?.to_str()?;
    let path = normalize_archive_path(name, false)?;
    Some(ArchiveEntry {
        path,
        is_dir: false,
    })
}

pub(super) fn collect_archive_entries_with_bsdtar<F>(
    path: &Path,
    canceled: &F,
) -> Option<Vec<ArchiveEntry>>
where
    F: Fn() -> bool,
{
    let output = run_archive_listing_command("bsdtar", &["-tf"], path, canceled)?;
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output).lines(),
        false,
    ))
}

pub(super) fn collect_archive_entries_with_unrar<F>(
    path: &Path,
    canceled: &F,
) -> Option<Vec<ArchiveEntry>>
where
    F: Fn() -> bool,
{
    let output = run_archive_listing_command("unrar", &["lb"], path, canceled)?;
    Some(parse_unrar_bare_listing(&String::from_utf8_lossy(&output)))
}

pub(super) fn collect_archive_listing_with_7z<F>(
    path: &Path,
    canceled: &F,
) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>)>
where
    F: Fn() -> bool,
{
    for &program in SEVEN_ZIP_PROGRAMS {
        if let Some(listing) = run_archive_listing_command(program, &["l", "-slt"], path, canceled)
            .and_then(|output| parse_7z_listing(&String::from_utf8_lossy(&output)))
        {
            return Some(listing);
        }
    }
    None
}

fn run_archive_listing_command<F>(
    program: &str,
    args: &[&str],
    path: &Path,
    canceled: &F,
) -> Option<Vec<u8>>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let (output_guard, output_file) = ArchiveCommandOutputFile::create(program)?;
    let mut child = Command::new(program)
        .args(args)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output_file))
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + ARCHIVE_EXTERNAL_COMMAND_TIMEOUT;
    loop {
        if canceled() || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let output = fs::read(output_guard.path()).ok()?;
                return Some(output);
            }
            Ok(None) => std::thread::sleep(ARCHIVE_EXTERNAL_COMMAND_POLL),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn archive_command_output_path(program: &str) -> PathBuf {
    let counter = ARCHIVE_OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "elio-archive-{program}-{}-{counter}.out",
        std::process::id()
    ))
}

fn parse_7z_listing(output: &str) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>)> {
    let mut metadata = ArchiveMetadata::default();
    let mut entries = Vec::new();
    let mut in_entries = false;
    let mut current = BTreeMap::<String, String>::new();

    for raw_line in output.lines() {
        let line = raw_line.trim_end();
        if line == "----------" {
            in_entries = true;
            continue;
        }

        if !in_entries {
            if let Some((key, value)) = parse_key_value_line(line) {
                match key {
                    "Type" => metadata.format_label = Some(value.to_string()),
                    "Physical Size" => metadata.physical_size = parse_u64(value),
                    "Comment" if !value.is_empty() => metadata.comment = Some(value.to_string()),
                    _ => {}
                }
            }
            continue;
        }

        if line.is_empty() {
            push_7z_entry(&mut current, &mut entries, &mut metadata);
            continue;
        }

        if let Some((key, value)) = parse_key_value_line(line) {
            current.insert(key.to_string(), value.to_string());
        }
    }
    push_7z_entry(&mut current, &mut entries, &mut metadata);

    if entries.is_empty()
        && metadata.format_label.is_none()
        && metadata.physical_size.is_none()
        && metadata.comment.is_none()
    {
        None
    } else {
        Some((metadata, entries))
    }
}

fn push_7z_entry(
    current: &mut BTreeMap<String, String>,
    entries: &mut Vec<ArchiveEntry>,
    metadata: &mut ArchiveMetadata,
) {
    if current.is_empty() {
        return;
    }

    let path = current.get("Path").cloned();
    let is_dir = current.get("Folder").is_some_and(|value| value == "+")
        || current
            .get("Attributes")
            .is_some_and(|value| value.starts_with('D'));

    if let Some(path) = path.and_then(|path| normalize_archive_path(&path, false)) {
        entries.push(ArchiveEntry { path, is_dir });
    }

    if let Some(size) = current.get("Size").and_then(|value| parse_u64(value)) {
        metadata.unpacked_size = Some(metadata.unpacked_size.unwrap_or(0).saturating_add(size));
    }
    if let Some(size) = current
        .get("Packed Size")
        .and_then(|value| parse_u64(value))
    {
        metadata.compressed_size = Some(metadata.compressed_size.unwrap_or(0).saturating_add(size));
    }
    current.clear();
}

fn parse_unrar_bare_listing(output: &str) -> Vec<ArchiveEntry> {
    normalize_archive_entries(output.lines(), false)
}

#[cfg(test)]
#[path = "tests/external_tools.rs"]
mod tests;
