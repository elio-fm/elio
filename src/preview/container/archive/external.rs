use super::common::{normalize_archive_path, parse_key_value_line, parse_u64};
use super::*;
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
mod tests {
    use super::{parse_7z_listing, parse_unrar_bare_listing, run_archive_listing_command};
    #[cfg(unix)]
    use std::time::{Duration, Instant};
    use std::{fs, path::PathBuf};

    #[test]
    fn parse_7z_listing_collects_external_fallback_metadata_and_entries() {
        let output = r#"
Path = app.AppImage
Type = SquashFS
Physical Size = 12345
Comment = portable build

----------
Path = AppRun
Folder = -
Size = 12
Packed Size = 10

Path = usr/bin/elio
Folder = -
Size = 52
Packed Size = 20

Path = usr/share/icons
Folder = +
Size = 0
Packed Size = 0
"#;

        let (metadata, entries) =
            parse_7z_listing(output).expect("7z listing should parse archive metadata");

        assert_eq!(metadata.format_label.as_deref(), Some("SquashFS"));
        assert_eq!(metadata.physical_size, Some(12_345));
        assert_eq!(metadata.comment.as_deref(), Some("portable build"));
        assert_eq!(metadata.unpacked_size, Some(64));
        assert_eq!(metadata.compressed_size, Some(30));
        assert_eq!(entries.len(), 3);
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "AppRun" && !entry.is_dir)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "usr/bin/elio" && !entry.is_dir)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "usr/share/icons" && entry.is_dir)
        );
    }

    #[test]
    fn parse_unrar_bare_listing_normalizes_nested_entries() {
        let output = r#"
./docs/readme.txt
src\main.rs
../ignored.txt
images/
"#;

        let entries = parse_unrar_bare_listing(output);

        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "docs/readme.txt" && !entry.is_dir)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "src/main.rs" && !entry.is_dir)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "images" && entry.is_dir)
        );
        assert!(!entries.iter().any(|entry| entry.path.contains("ignored")));
    }

    #[cfg(unix)]
    #[test]
    fn external_archive_command_observes_cancellation_while_running() {
        let started_at = Instant::now();
        let output = run_archive_listing_command(
            "sh",
            &["-c", "sleep 5", "sh"],
            std::path::Path::new("ignored.zip"),
            &|| started_at.elapsed() >= Duration::from_millis(40),
        );

        assert!(output.is_none());
        assert!(
            started_at.elapsed() < Duration::from_secs(1),
            "canceled archive command should not wait for the child process to finish"
        );
    }

    #[cfg(unix)]
    #[test]
    fn external_archive_command_does_not_inherit_stdin() {
        let started_at = Instant::now();
        let output = run_archive_listing_command(
            "sh",
            &["-c", "read ignored || exit 7", "sh"],
            std::path::Path::new("ignored.rar"),
            &|| false,
        );

        assert!(output.is_none());
        assert!(
            started_at.elapsed() < Duration::from_secs(1),
            "archive commands must not block the UI waiting for interactive password input"
        );
    }

    #[test]
    fn external_archive_command_cleans_temp_file_when_spawn_fails() {
        let program = "elio-definitely-missing-archive-tool-for-cleanup-test";
        remove_archive_temp_outputs(program);

        let output =
            run_archive_listing_command(program, &[], std::path::Path::new("ignored.zip"), &|| {
                false
            });

        assert!(output.is_none());
        assert!(
            archive_temp_outputs(program).is_empty(),
            "failed spawn should clean up its temp output file"
        );
    }

    fn archive_temp_outputs(program: &str) -> Vec<PathBuf> {
        let prefix = format!("elio-archive-{program}-{}-", std::process::id());
        fs::read_dir(std::env::temp_dir())
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".out"))
            })
            .collect()
    }

    fn remove_archive_temp_outputs(program: &str) {
        for path in archive_temp_outputs(program) {
            let _ = fs::remove_file(path);
        }
    }
}
