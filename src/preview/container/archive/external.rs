use super::common::{normalize_archive_path, parse_key_value_line, parse_u64};
use super::*;
use crate::preview::ArchiveFormat;
use std::{collections::BTreeMap, path::Path, process::Command};

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

pub(super) fn collect_archive_entries_with_bsdtar(path: &Path) -> Option<Vec<ArchiveEntry>> {
    let output = Command::new("bsdtar").arg("-tf").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output.stdout).lines(),
        false,
    ))
}

pub(super) fn collect_archive_listing_with_7z(
    path: &Path,
) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>)> {
    let output = Command::new("7z")
        .arg("l")
        .arg("-slt")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_7z_listing(&String::from_utf8_lossy(&output.stdout))
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

#[cfg(test)]
mod tests {
    use super::parse_7z_listing;

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
}
