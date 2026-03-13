use super::*;
use crate::{file_info, ui::theme};
use ratatui::text::Line;
use std::{collections::BTreeMap, fs::File, io::Read, path::Path, process::Command};
use zip::ZipArchive;

pub(in crate::preview) fn build_archive_preview(
    path: &Path,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    let format = detect_archive_format(path);
    if let Some(preview) = build_zip_archive_preview(path, format, type_detail) {
        return Some(preview);
    }
    build_external_archive_preview(path, format, type_detail)
}

fn detect_archive_format(path: &Path) -> ArchiveFormat {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default();
    if let Some(kind) = file_info::inspect_compound_archive_name(&name) {
        return match kind {
            file_info::CompoundArchiveKind::TarGzip => ArchiveFormat::TarGzip,
            file_info::CompoundArchiveKind::TarXz => ArchiveFormat::TarXz,
            file_info::CompoundArchiveKind::TarBzip2 => ArchiveFormat::TarBzip2,
            file_info::CompoundArchiveKind::TarZstd => ArchiveFormat::TarZstd,
            file_info::CompoundArchiveKind::CompressedDiskImage {
                compression: file_info::CompressionKind::Gzip,
                ..
            } => ArchiveFormat::Gzip,
            file_info::CompoundArchiveKind::CompressedDiskImage {
                compression: file_info::CompressionKind::Xz,
                ..
            } => ArchiveFormat::Xz,
            file_info::CompoundArchiveKind::CompressedDiskImage {
                compression: file_info::CompressionKind::Bzip2,
                ..
            } => ArchiveFormat::Bzip2,
            file_info::CompoundArchiveKind::CompressedDiskImage {
                compression: file_info::CompressionKind::Zstd,
                ..
            } => ArchiveFormat::Zstd,
        };
    }

    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("zip" | "jar" | "apk" | "aab" | "apkg") => ArchiveFormat::Zip,
        Some("7z") => ArchiveFormat::SevenZip,
        Some("tar") => ArchiveFormat::Tar,
        Some("gz") => ArchiveFormat::Gzip,
        Some("xz") => ArchiveFormat::Xz,
        Some("bz2") => ArchiveFormat::Bzip2,
        Some("zst") => ArchiveFormat::Zstd,
        Some("deb") => ArchiveFormat::Deb,
        Some("rpm") => ArchiveFormat::Rpm,
        Some("appimage") => ArchiveFormat::AppImage,
        _ => ArchiveFormat::Unknown,
    }
}

fn archive_default_label(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Zip => "ZIP archive",
        ArchiveFormat::SevenZip => "7z archive",
        ArchiveFormat::Tar => "TAR archive",
        ArchiveFormat::TarGzip => "TAR.GZ archive",
        ArchiveFormat::TarXz => "TAR.XZ archive",
        ArchiveFormat::TarBzip2 => "TAR.BZ2 archive",
        ArchiveFormat::TarZstd => "TAR.ZST archive",
        ArchiveFormat::Gzip => "Gzip archive",
        ArchiveFormat::Xz => "XZ archive",
        ArchiveFormat::Bzip2 => "Bzip2 archive",
        ArchiveFormat::Zstd => "Zstandard archive",
        ArchiveFormat::Deb => "Debian package",
        ArchiveFormat::Rpm => "RPM package",
        ArchiveFormat::AppImage => "AppImage bundle",
        ArchiveFormat::Unknown => "Archive",
    }
}

fn archive_format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Zip => "ZIP",
        ArchiveFormat::SevenZip => "7z",
        ArchiveFormat::Tar => "TAR",
        ArchiveFormat::TarGzip => "TAR.GZ",
        ArchiveFormat::TarXz => "TAR.XZ",
        ArchiveFormat::TarBzip2 => "TAR.BZ2",
        ArchiveFormat::TarZstd => "TAR.ZST",
        ArchiveFormat::Gzip => "Gzip",
        ArchiveFormat::Xz => "XZ",
        ArchiveFormat::Bzip2 => "Bzip2",
        ArchiveFormat::Zstd => "Zstandard",
        ArchiveFormat::Deb => "DEB",
        ArchiveFormat::Rpm => "RPM",
        ArchiveFormat::AppImage => "AppImage",
        ArchiveFormat::Unknown => "Archive",
    }
}

fn build_zip_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    if format != ArchiveFormat::Zip {
        return None;
    }

    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    let total_entries = archive.len();
    let mut entries = Vec::with_capacity(total_entries.min(ARCHIVE_ENTRY_SCAN_LIMIT));
    let mut metadata = ArchiveMetadata {
        format_label: Some(archive_format_name(format).to_string()),
        ..ArchiveMetadata::default()
    };
    let mut manifest = ZipManifestMetadata::default();

    for index in 0..total_entries.min(ARCHIVE_ENTRY_SCAN_LIMIT) {
        let entry = archive.by_index(index).ok()?;
        let is_dir = entry.is_dir();
        let name = entry.name().to_string();
        if let Some(path) = normalize_archive_path(&name, false) {
            entries.push(ArchiveEntry { path, is_dir });
        }
        metadata.unpacked_size = Some(
            metadata
                .unpacked_size
                .unwrap_or(0)
                .saturating_add(entry.size()),
        );
        metadata.compressed_size = Some(
            metadata
                .compressed_size
                .unwrap_or(0)
                .saturating_add(entry.compressed_size()),
        );

        if manifest.is_empty()
            && !is_dir
            && name.eq_ignore_ascii_case("META-INF/MANIFEST.MF")
            && entry.size() <= ZIP_MANIFEST_LIMIT_BYTES
        {
            let mut contents = String::new();
            if entry
                .take(ZIP_MANIFEST_LIMIT_BYTES)
                .read_to_string(&mut contents)
                .is_ok()
            {
                manifest = parse_zip_manifest(&contents);
            }
        }
    }

    let comment = String::from_utf8_lossy(archive.comment());
    let comment = comment.trim();
    if !comment.is_empty() {
        metadata.comment = Some(comment.to_string());
    }

    let detail = type_detail.unwrap_or(archive_default_label(format));
    let scan_truncated = total_entries > ARCHIVE_ENTRY_SCAN_LIMIT;
    Some(render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata,
        entries: Some(entries),
        total_entries_hint: Some(total_entries),
        empty_label: archive_is_empty_label(format),
        unavailable_label: "Unable to read archive contents",
        extra_sections: zip_manifest_sections(&manifest),
        scan_truncated,
    }))
}

fn build_external_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    let detail = type_detail.unwrap_or(archive_default_label(format));
    if let Some(entries) = collect_preferred_archive_entries(path, format) {
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata: ArchiveMetadata {
                format_label: Some(archive_format_name(format).to_string()),
                ..ArchiveMetadata::default()
            },
            entries: Some(entries),
            total_entries_hint: None,
            empty_label: archive_is_empty_label(format),
            unavailable_label: "Unable to read archive contents",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }

    if let Some((metadata, mut entries)) = collect_archive_listing_with_7z(path) {
        if entries.is_empty()
            && let Some(entry) = synthetic_single_file_archive_entry(path, format)
        {
            entries.push(entry);
        }
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata,
            entries: Some(entries),
            total_entries_hint: None,
            empty_label: archive_is_empty_label(format),
            unavailable_label: "Unable to read archive contents",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }

    let entries = collect_fallback_archive_entries(path)?;

    Some(render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata: ArchiveMetadata {
            format_label: Some(archive_format_name(format).to_string()),
            ..ArchiveMetadata::default()
        },
        entries: Some(entries),
        total_entries_hint: None,
        empty_label: archive_is_empty_label(format),
        unavailable_label: "Unable to read archive contents",
        extra_sections: Vec::new(),
        scan_truncated: false,
    }))
}

fn collect_preferred_archive_entries(
    path: &Path,
    format: ArchiveFormat,
) -> Option<Vec<ArchiveEntry>> {
    if prefers_tar_listing(format) {
        return collect_archive_entries_with_bsdtar(path)
            .or_else(|| collect_archive_entries_with_tar(path));
    }

    None
}

fn collect_fallback_archive_entries(path: &Path) -> Option<Vec<ArchiveEntry>> {
    collect_archive_entries_with_bsdtar(path)
        .or_else(|| collect_archive_entries_with_tar(path))
        .or_else(|| collect_archive_entries_with_unzip(path))
}

fn prefers_tar_listing(format: ArchiveFormat) -> bool {
    matches!(
        format,
        ArchiveFormat::Tar
            | ArchiveFormat::TarGzip
            | ArchiveFormat::TarXz
            | ArchiveFormat::TarBzip2
            | ArchiveFormat::TarZstd
    )
}

fn synthetic_single_file_archive_entry(path: &Path, format: ArchiveFormat) -> Option<ArchiveEntry> {
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

fn render_archive_preview(config: ArchiveRenderConfig) -> PreviewContent {
    let palette = theme::palette();
    let mut lines = Vec::new();
    let entries = expand_archive_entries(config.entries.unwrap_or_default());
    let total_items = entries.len().max(config.total_entries_hint.unwrap_or(0));
    let folder_count = entries.iter().filter(|entry| entry.is_dir).count();
    let file_count = total_items.saturating_sub(folder_count);

    let summary = vec![
        ("Format", config.metadata.format_label),
        (
            "Entries",
            (total_items > 0).then(|| format!("{total_items} total")),
        ),
        (
            "Folders",
            (folder_count > 0).then(|| folder_count.to_string()),
        ),
        ("Files", (file_count > 0).then(|| file_count.to_string())),
        (
            "Packed",
            config.metadata.compressed_size.map(crate::app::format_size),
        ),
        (
            "Unpacked",
            config.metadata.unpacked_size.map(crate::app::format_size),
        ),
        (
            "Archive Size",
            config.metadata.physical_size.map(crate::app::format_size),
        ),
        ("Comment", config.metadata.comment),
    ];
    push_preview_section(&mut lines, "Summary", &summary, palette);

    for (title, fields) in config.extra_sections {
        push_preview_values_section(&mut lines, title, &fields, palette);
    }

    let mut rendered_items = 0usize;
    let mut tree_truncated = false;
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line("Contents", palette));

    if entries.is_empty() {
        lines.push(Line::from(if total_items == 0 {
            config.empty_label.to_string()
        } else {
            config.unavailable_label.to_string()
        }));
    } else {
        let mut root = ArchiveTreeNode::default();
        for entry in &entries {
            insert_archive_tree_entry(&mut root, entry);
        }
        let available_lines = PREVIEW_RENDER_LINE_LIMIT.saturating_sub(lines.len());
        let mut remaining = available_lines;
        if remaining == 0 {
            tree_truncated = true;
        } else {
            let children = ordered_archive_children(&root.children);
            render_archive_tree(
                &children,
                "",
                &mut remaining,
                &mut rendered_items,
                &mut lines,
                palette,
            );
            tree_truncated = rendered_items < entries.len();
        }
    }

    let mut notes = Vec::new();
    if config.scan_truncated {
        notes.push(format!(
            "scanned first {} of {} entries",
            entries.len(),
            total_items
        ));
    }
    if tree_truncated {
        notes.push(format!(
            "showing first {} of {} entries",
            rendered_items.max(entries.len().min(PREVIEW_RENDER_LINE_LIMIT)),
            total_items
        ));
    }

    let mut preview = PreviewContent::new(PreviewKind::Archive, lines)
        .with_detail(config.detail)
        .with_directory_counts(total_items, folder_count, file_count);
    if !notes.is_empty() {
        preview = preview.with_truncation(notes.join("  •  "));
    }
    preview
}

struct ArchiveRenderConfig {
    detail: String,
    metadata: ArchiveMetadata,
    entries: Option<Vec<ArchiveEntry>>,
    total_entries_hint: Option<usize>,
    empty_label: &'static str,
    unavailable_label: &'static str,
    extra_sections: Vec<(&'static str, Vec<(&'static str, String)>)>,
    scan_truncated: bool,
}

fn archive_is_empty_label(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Zip => "Archive is empty",
        ArchiveFormat::SevenZip => "Archive is empty",
        ArchiveFormat::Tar
        | ArchiveFormat::TarGzip
        | ArchiveFormat::TarXz
        | ArchiveFormat::TarBzip2
        | ArchiveFormat::TarZstd
        | ArchiveFormat::Gzip
        | ArchiveFormat::Xz
        | ArchiveFormat::Bzip2
        | ArchiveFormat::Zstd
        | ArchiveFormat::Deb
        | ArchiveFormat::Rpm
        | ArchiveFormat::AppImage
        | ArchiveFormat::Unknown => "Archive is empty",
    }
}

fn collect_archive_entries_with_bsdtar(path: &Path) -> Option<Vec<ArchiveEntry>> {
    let output = Command::new("bsdtar").arg("-tf").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output.stdout).lines(),
        false,
    ))
}

fn collect_archive_entries_with_tar(path: &Path) -> Option<Vec<ArchiveEntry>> {
    let output = Command::new("tar").arg("-tf").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output.stdout).lines(),
        false,
    ))
}

fn collect_archive_entries_with_unzip(path: &Path) -> Option<Vec<ArchiveEntry>> {
    let output = Command::new("unzip").arg("-Z1").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output.stdout).lines(),
        false,
    ))
}

fn collect_archive_listing_with_7z(path: &Path) -> Option<(ArchiveMetadata, Vec<ArchiveEntry>)> {
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

fn parse_key_value_line(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(" = ")?;
    Some((key.trim(), value.trim()))
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

fn normalize_archive_path(item: &str, strip_version_suffix: bool) -> Option<String> {
    normalize_archive_entry(item, strip_version_suffix).map(|entry| entry.path)
}

fn parse_zip_manifest(contents: &str) -> ZipManifestMetadata {
    let mut fields = BTreeMap::<String, String>::new();
    let mut current_key: Option<String> = None;

    for line in contents.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(key) = &current_key
                && let Some(value) = fields.get_mut(key)
            {
                value.push_str(rest);
            }
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            current_key = None;
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim().to_string();
        current_key = Some(key.clone());
        fields.insert(key, value);
    }

    ZipManifestMetadata {
        title: fields
            .get("Implementation-Title")
            .cloned()
            .or_else(|| fields.get("Bundle-Name").cloned()),
        version: fields
            .get("Implementation-Version")
            .cloned()
            .or_else(|| fields.get("Bundle-Version").cloned()),
        main_class: fields.get("Main-Class").cloned(),
        created_by: fields.get("Created-By").cloned(),
        automatic_module: fields.get("Automatic-Module-Name").cloned(),
    }
}

impl ZipManifestMetadata {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.version.is_none()
            && self.main_class.is_none()
            && self.created_by.is_none()
            && self.automatic_module.is_none()
    }
}

fn zip_manifest_sections(
    manifest: &ZipManifestMetadata,
) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    if manifest.is_empty() {
        return Vec::new();
    }

    let mut fields = Vec::new();
    if let Some(value) = &manifest.title {
        fields.push(("Title", value.clone()));
    }
    if let Some(value) = &manifest.version {
        fields.push(("Version", value.clone()));
    }
    if let Some(value) = &manifest.main_class {
        fields.push(("Main-Class", value.clone()));
    }
    if let Some(value) = &manifest.automatic_module {
        fields.push(("Module", value.clone()));
    }
    if let Some(value) = &manifest.created_by {
        fields.push(("Created By", value.clone()));
    }
    vec![("Manifest", fields)]
}
