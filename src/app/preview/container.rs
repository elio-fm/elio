use super::*;
use crate::appearance;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path, process::Command};
use zip::ZipArchive;

pub(super) fn build_iso_preview(path: &Path) -> Option<PreviewContent> {
    let metadata = read_iso_metadata(path);
    let entries = collect_iso_entries(path);
    if metadata.is_none() && entries.is_none() {
        return None;
    }

    Some(render_iso_preview(
        metadata.unwrap_or_default(),
        entries.unwrap_or_default(),
    ))
}

pub(super) fn build_archive_preview(
    path: &Path,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    let format = detect_archive_format(path);
    if let Some(preview) = build_zip_archive_preview(path, format, type_detail) {
        return Some(preview);
    }
    build_external_archive_preview(path, format, type_detail)
}

pub(super) fn build_torrent_preview(path: &Path) -> Option<PreviewContent> {
    const TORRENT_PREVIEW_LIMIT_BYTES: u64 = 1024 * 1024;
    const TORRENT_CONTENT_SAMPLE_LIMIT: usize = 200;

    let mut bytes = Vec::with_capacity(TORRENT_PREVIEW_LIMIT_BYTES as usize);
    File::open(path)
        .ok()?
        .take(TORRENT_PREVIEW_LIMIT_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;

    let mut index = 0usize;
    let mut metadata = TorrentMetadata::default();
    parse_torrent_root(
        &bytes,
        &mut index,
        &mut metadata,
        TORRENT_CONTENT_SAMPLE_LIMIT,
    )?;
    metadata.finalize();

    let palette = appearance::palette();
    let mut lines = Vec::new();
    let file_count = metadata.file_count.max(1);
    let tracker_count = metadata.announce_tiers.iter().map(Vec::len).sum::<usize>();
    let summary = vec![
        (
            "Name",
            Some(
                metadata
                    .name
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
        ),
        ("Mode", metadata.mode.map(|mode| mode.label().to_string())),
        ("Files", Some(file_count.to_string())),
        ("Size", metadata.total_size.map(crate::app::format_size)),
        (
            "Piece Size",
            metadata.piece_length.map(crate::app::format_size),
        ),
        (
            "Pieces",
            metadata.piece_count.map(|count| count.to_string()),
        ),
        (
            "Trackers",
            (tracker_count > 0).then(|| {
                let tier_count = metadata.announce_tiers.len();
                let tier_label = if tier_count == 1 { "tier" } else { "tiers" };
                format!("{tracker_count} across {tier_count} {tier_label}")
            }),
        ),
        (
            "Privacy",
            metadata.private.map(|is_private| {
                if is_private {
                    "Private".to_string()
                } else {
                    "Public".to_string()
                }
            }),
        ),
    ];
    push_preview_section(&mut lines, "Torrent", &summary, palette);

    let metadata_fields = vec![
        ("Created By", metadata.created_by.clone()),
        ("Comment", metadata.comment.clone()),
    ];
    push_preview_section(&mut lines, "Metadata", &metadata_fields, palette);

    let tracker_fields = metadata.tracker_fields();
    push_preview_owned_values_section(&mut lines, "Trackers", &tracker_fields, palette);

    let content_entries = metadata.content_entries();
    let mut rendered_items = 0usize;
    let mut tree_truncated = false;
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line("Contents", palette));

    if content_entries.is_empty() {
        lines.push(Line::from("Contents unavailable"));
    } else {
        let mut root = ArchiveTreeNode::default();
        for entry in &content_entries {
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
            tree_truncated = rendered_items < content_entries.len();
        }
    }

    let mut notes = Vec::new();
    if metadata.file_sample_truncated {
        notes.push(format!(
            "showing first {} of {} files",
            metadata.files.len(),
            file_count
        ));
    } else if tree_truncated {
        notes.push(format!(
            "contents view truncated after {rendered_items} rows"
        ));
    }

    let mut preview = PreviewContent::new(PreviewKind::Text, lines)
        .with_detail("BitTorrent file")
        .with_directory_counts(file_count, 0, file_count);
    if !notes.is_empty() {
        preview = preview.with_truncation(notes.join("  •  "));
    }
    Some(preview)
}

impl TorrentMode {
    fn label(self) -> &'static str {
        match self {
            Self::SingleFile => "Single-file",
            Self::MultiFile => "Multi-file",
        }
    }
}

impl TorrentMetadata {
    fn finalize(&mut self) {
        self.normalize_trackers();
        if self.mode.is_none() {
            self.mode = Some(if self.file_count > 1 {
                TorrentMode::MultiFile
            } else {
                TorrentMode::SingleFile
            });
        }
        if self.file_count == 0 {
            self.file_count = 1;
        }
        if self.mode == Some(TorrentMode::SingleFile)
            && self.files.is_empty()
            && let Some(name) = self.name.clone().filter(|name| !name.is_empty())
        {
            self.files.push(TorrentFileEntry {
                path: name,
                length: self.total_size.unwrap_or(0),
            });
        }
    }

    fn normalize_trackers(&mut self) {
        let mut normalized = self.announce_tiers.drain(..).collect::<Vec<_>>();
        if let Some(primary) = self.announce.take().filter(|value| !value.is_empty()) {
            if normalized.is_empty() {
                normalized.push(vec![primary]);
            } else if !normalized[0].iter().any(|tracker| tracker == &primary) {
                normalized[0].insert(0, primary);
            }
        }

        let mut seen = Vec::<String>::new();
        self.announce_tiers = normalized
            .into_iter()
            .filter_map(|tier| {
                let mut tier_seen = Vec::<String>::new();
                let deduped = tier
                    .into_iter()
                    .filter(|tracker| !tracker.is_empty())
                    .filter(|tracker| {
                        if seen.iter().any(|existing| existing == tracker)
                            || tier_seen.iter().any(|existing| existing == tracker)
                        {
                            return false;
                        }
                        tier_seen.push(tracker.clone());
                        seen.push(tracker.clone());
                        true
                    })
                    .collect::<Vec<_>>();
                (!deduped.is_empty()).then_some(deduped)
            })
            .collect();
    }

    fn tracker_fields(&self) -> Vec<(String, String)> {
        self.announce_tiers
            .iter()
            .enumerate()
            .map(|(index, tier)| (format!("Tier {}", index + 1), tier.join("  •  ")))
            .collect()
    }

    fn content_entries(&self) -> Vec<ArchiveEntry> {
        match self.mode.unwrap_or(TorrentMode::SingleFile) {
            TorrentMode::SingleFile => self
                .files
                .first()
                .map(|file| ArchiveEntry {
                    path: file.path.clone(),
                    is_dir: false,
                })
                .into_iter()
                .collect(),
            TorrentMode::MultiFile => {
                let root = self.name.as_deref().filter(|name| !name.is_empty());
                self.files
                    .iter()
                    .map(|file| ArchiveEntry {
                        path: match root {
                            Some(root) => format!("{root}/{}", file.path),
                            None => file.path.clone(),
                        },
                        is_dir: false,
                    })
                    .collect()
            }
        }
    }
}

fn detect_archive_format(path: &Path) -> ArchiveFormat {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return ArchiveFormat::TarGzip;
    }
    if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        return ArchiveFormat::TarXz;
    }
    if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") || name.ends_with(".tbz") {
        return ArchiveFormat::TarBzip2;
    }
    if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        return ArchiveFormat::TarZstd;
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
    if let Some((metadata, entries)) = collect_archive_listing_with_7z(path) {
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

    let entries = collect_archive_entries_with_bsdtar(path)
        .or_else(|| collect_archive_entries_with_tar(path))
        .or_else(|| collect_archive_entries_with_unzip(path))?;

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

fn read_iso_metadata(path: &Path) -> Option<IsoMetadata> {
    let mut bytes = Vec::with_capacity(ISO_METADATA_SCAN_BYTES as usize);
    File::open(path)
        .ok()?
        .take(ISO_METADATA_SCAN_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    parse_iso_metadata(&bytes)
}

pub(super) fn parse_iso_metadata(bytes: &[u8]) -> Option<IsoMetadata> {
    let mut metadata = IsoMetadata::default();
    let mut found_descriptor = false;
    let start = ISO_DESCRIPTOR_START_SECTOR * ISO_SECTOR_SIZE;
    if bytes.len() < start + ISO_SECTOR_SIZE {
        return None;
    }

    for descriptor in bytes[start..].chunks_exact(ISO_SECTOR_SIZE) {
        if descriptor.get(1..6) != Some(b"CD001".as_slice()) {
            continue;
        }

        found_descriptor = true;
        match descriptor[0] {
            0 => {
                let boot_id = parse_iso_text_field(descriptor, 7, 39);
                if boot_id
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(ISO_BOOT_SYSTEM_ID))
                {
                    metadata.bootable = true;
                }
            }
            1 => {
                metadata.system_id = parse_iso_text_field(descriptor, 8, 40);
                metadata.volume_id = parse_iso_text_field(descriptor, 40, 72);
                metadata.publisher_id = parse_iso_text_field(descriptor, 318, 446);
                metadata.preparer_id = parse_iso_text_field(descriptor, 446, 574);
                metadata.application_id = parse_iso_text_field(descriptor, 574, 702);
                metadata.created_at = parse_iso_datetime_field(descriptor, 813, 830);
                metadata.modified_at = parse_iso_datetime_field(descriptor, 830, 847);
                metadata.effective_at = parse_iso_datetime_field(descriptor, 864, 881);

                let volume_blocks = parse_iso_u32_le(descriptor, 80);
                let block_size = parse_iso_u16_le(descriptor, 128);
                metadata.total_size = volume_blocks
                    .zip(block_size)
                    .map(|(blocks, block_size)| u64::from(blocks) * u64::from(block_size));
            }
            255 => break,
            _ => {}
        }
    }

    found_descriptor.then_some(metadata)
}

fn collect_iso_entries(path: &Path) -> Option<Vec<ArchiveEntry>> {
    collect_iso_entries_with_bsdtar(path).or_else(|| collect_iso_entries_with_isoinfo(path))
}

fn collect_iso_entries_with_bsdtar(path: &Path) -> Option<Vec<ArchiveEntry>> {
    let output = Command::new("bsdtar").arg("-tf").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output.stdout).lines(),
        true,
    ))
}

fn collect_iso_entries_with_isoinfo(path: &Path) -> Option<Vec<ArchiveEntry>> {
    let output = Command::new("isoinfo")
        .arg("-i")
        .arg(path)
        .arg("-f")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(normalize_archive_entries(
        String::from_utf8_lossy(&output.stdout).lines(),
        true,
    ))
}

pub(super) fn normalize_archive_entries<'a>(
    items: impl IntoIterator<Item = &'a str>,
    strip_version_suffix: bool,
) -> Vec<ArchiveEntry> {
    let mut normalized = BTreeMap::<String, bool>::new();
    for item in items {
        let Some(entry) = normalize_archive_entry(item, strip_version_suffix) else {
            continue;
        };
        insert_archive_entry(&mut normalized, &entry.path, entry.is_dir);
    }

    normalized
        .into_iter()
        .map(|(path, is_dir)| ArchiveEntry { path, is_dir })
        .collect()
}

fn expand_archive_entries(entries: Vec<ArchiveEntry>) -> Vec<ArchiveEntry> {
    let mut normalized = BTreeMap::<String, bool>::new();
    for entry in entries {
        insert_archive_entry(&mut normalized, &entry.path, entry.is_dir);
    }
    normalized
        .into_iter()
        .map(|(path, is_dir)| ArchiveEntry { path, is_dir })
        .collect()
}

fn normalize_archive_entry(item: &str, strip_version_suffix: bool) -> Option<ArchiveEntry> {
    let trimmed = trim_trailing_line_endings(item);
    let trimmed = trimmed.trim();
    if trimmed.is_empty() {
        return None;
    }

    let is_dir = trimmed.ends_with('/') || trimmed.ends_with('\\');
    let trimmed = trimmed
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed == "." {
        return None;
    }

    let mut segments = Vec::new();
    for segment in trimmed.split(['/', '\\']) {
        let segment = if strip_version_suffix {
            strip_iso_version_suffix(segment.trim())
        } else {
            segment.trim()
        };
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return None;
        }
        segments.push(segment.to_string());
    }

    if segments.is_empty() {
        return None;
    }

    Some(ArchiveEntry {
        path: segments.join("/"),
        is_dir,
    })
}

fn insert_archive_entry(entries: &mut BTreeMap<String, bool>, path: &str, is_dir: bool) {
    let mut built = String::new();
    let parts = path.split('/').collect::<Vec<_>>();
    for (index, segment) in parts.iter().enumerate() {
        if !built.is_empty() {
            built.push('/');
        }
        built.push_str(segment);
        let current_is_dir = index < parts.len().saturating_sub(1) || is_dir;
        entries
            .entry(built.clone())
            .and_modify(|existing| *existing |= current_is_dir)
            .or_insert(current_is_dir);
    }
}

pub(super) fn render_iso_preview(
    metadata: IsoMetadata,
    entries: Vec<ArchiveEntry>,
) -> PreviewContent {
    let palette = appearance::palette();
    let mut lines = Vec::new();
    let total_items = entries.len();
    let folder_count = entries.iter().filter(|entry| entry.is_dir).count();
    let file_count = total_items.saturating_sub(folder_count);

    let summary = vec![
        ("Volume", metadata.volume_id.clone()),
        ("System", metadata.system_id.clone()),
        (
            "Image Size",
            metadata.total_size.map(crate::app::format_size),
        ),
        (
            "Bootable",
            metadata
                .bootable
                .then(|| "El Torito".to_string())
                .or_else(|| (total_items > 0).then(|| "No".to_string())),
        ),
        (
            "Entries",
            (total_items > 0).then(|| format!("{total_items} total")),
        ),
        (
            "Folders",
            (folder_count > 0).then(|| format!("{folder_count}")),
        ),
        ("Files", (file_count > 0).then(|| format!("{file_count}"))),
        ("Publisher", metadata.publisher_id.clone()),
        ("Prepared By", metadata.preparer_id.clone()),
        ("Application", metadata.application_id.clone()),
        ("Created", metadata.created_at.clone()),
        ("Modified", metadata.modified_at.clone()),
        ("Effective", metadata.effective_at.clone()),
    ];

    push_preview_section(&mut lines, "Image", &summary, palette);

    let mut rendered_items = 0usize;
    let mut tree_truncated = false;
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line("Contents", palette));

    if entries.is_empty() {
        lines.push(Line::from("Unable to read ISO contents"));
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
            tree_truncated = rendered_items < total_items;
        }
    }

    let mut preview =
        PreviewContent::new(PreviewKind::Archive, lines).with_detail("ISO disk image");
    let truncation_note = match (tree_truncated, total_items, rendered_items) {
        (true, total, rendered) if total > 0 && rendered > 0 => {
            Some(format!("showing first {rendered} of {total} entries"))
        }
        (true, total, _) if total > 0 => {
            Some(format!("showing first {PREVIEW_RENDER_LINE_LIMIT} lines"))
        }
        _ => None,
    };
    if let Some(note) = truncation_note {
        preview = preview.with_truncation(note);
    }
    preview
}

fn render_archive_preview(config: ArchiveRenderConfig) -> PreviewContent {
    let palette = appearance::palette();
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
    push_preview_section(&mut lines, "Archive", &summary, palette);

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

fn insert_archive_tree_entry(root: &mut ArchiveTreeNode, entry: &ArchiveEntry) {
    let mut current = root;
    let mut built = String::new();
    let parts = entry.path.split('/').collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
        if !built.is_empty() {
            built.push('/');
        }
        built.push_str(part);
        let is_last = index == parts.len().saturating_sub(1);
        current = current
            .children
            .entry((*part).to_string())
            .or_insert_with(|| ArchiveTreeNode {
                path: built.clone(),
                is_dir: !is_last || entry.is_dir,
                children: BTreeMap::new(),
            });
        current.path = built.clone();
        current.is_dir |= !is_last || entry.is_dir;
    }
}

fn ordered_archive_children(
    children: &BTreeMap<String, ArchiveTreeNode>,
) -> Vec<(&String, &ArchiveTreeNode)> {
    let mut ordered = children.iter().collect::<Vec<_>>();
    ordered.sort_by(|(left_name, left), (right_name, right)| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then_with(|| left_name.to_lowercase().cmp(&right_name.to_lowercase()))
    });
    ordered
}

fn render_archive_tree(
    children: &[(&String, &ArchiveTreeNode)],
    prefix: &str,
    remaining: &mut usize,
    rendered_items: &mut usize,
    lines: &mut Vec<Line<'static>>,
    palette: appearance::Palette,
) {
    for (index, (name, node)) in children.iter().enumerate() {
        if *remaining == 0 {
            return;
        }

        let is_last = index == children.len().saturating_sub(1);
        lines.push(render_archive_tree_line(
            prefix, name, node, is_last, palette,
        ));
        *remaining = remaining.saturating_sub(1);
        *rendered_items += 1;

        if node.is_dir && !node.children.is_empty() {
            let mut next_prefix = prefix.to_string();
            next_prefix.push_str(if is_last { "    " } else { "│   " });
            let nested = ordered_archive_children(&node.children);
            render_archive_tree(
                &nested,
                &next_prefix,
                remaining,
                rendered_items,
                lines,
                palette,
            );
            if *remaining == 0 {
                return;
            }
        }
    }
}

fn render_archive_tree_line(
    prefix: &str,
    name: &str,
    node: &ArchiveTreeNode,
    is_last: bool,
    palette: appearance::Palette,
) -> Line<'static> {
    let connector = if is_last { "└── " } else { "├── " };
    let appearance = appearance::resolve_path(
        Path::new(&node.path),
        if node.is_dir {
            EntryKind::Directory
        } else {
            EntryKind::File
        },
    );
    let mut display_name = name.to_string();
    if node.is_dir {
        display_name.push('/');
    }

    Line::from(vec![
        Span::styled(
            format!("{prefix}{connector}"),
            Style::default().fg(palette.muted),
        ),
        Span::styled(
            format!("{} ", appearance.icon),
            Style::default()
                .fg(appearance.color)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(display_name, Style::default().fg(palette.text)),
    ])
}

fn push_preview_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(&str, Option<String>)],
    palette: appearance::Palette,
) {
    let visible_fields = fields
        .iter()
        .filter_map(|(label, value)| value.as_deref().map(|value| (*label, value)))
        .collect::<Vec<_>>();
    if visible_fields.is_empty() {
        return;
    }

    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line(title, palette));
    let label_width = visible_fields
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(6);
    for (label, value) in visible_fields {
        lines.push(preview_field_line(label, value, label_width, palette));
    }
}

fn push_preview_values_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(&str, String)],
    palette: appearance::Palette,
) {
    if fields.is_empty() {
        return;
    }

    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line(title, palette));
    let label_width = fields
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(6);
    for (label, value) in fields {
        lines.push(preview_field_line(label, value, label_width, palette));
    }
}

fn push_preview_owned_values_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(String, String)],
    palette: appearance::Palette,
) {
    if fields.is_empty() {
        return;
    }

    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line(title, palette));
    let label_width = fields
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(6);
    for (label, value) in fields {
        lines.push(preview_field_line(label, value, label_width, palette));
    }
}

fn section_line(title: &str, palette: appearance::Palette) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(palette.accent),
    ))
}

fn preview_field_line(
    label: &str,
    value: &str,
    label_width: usize,
    palette: appearance::Palette,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<width$} ", width = label_width + 1),
            Style::default().fg(palette.muted),
        ),
        Span::styled(value.to_string(), Style::default().fg(palette.text)),
    ])
}

fn parse_iso_text_field(bytes: &[u8], start: usize, end: usize) -> Option<String> {
    let field = bytes.get(start..end)?;
    let text = String::from_utf8_lossy(field);
    let trimmed = text.trim_matches(char::from(0)).trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn parse_iso_datetime_field(bytes: &[u8], start: usize, end: usize) -> Option<String> {
    let raw = parse_iso_text_field(bytes, start, end)?;
    format_iso_datetime(&raw)
}

fn format_iso_datetime(value: &str) -> Option<String> {
    let digits = value.as_bytes();
    if digits.len() != 17 || digits[..16].iter().any(|byte| !byte.is_ascii_digit()) {
        return None;
    }
    if digits[..16].iter().all(|byte| *byte == b'0') {
        return None;
    }

    let year = &value[0..4];
    let month = &value[4..6];
    let day = &value[6..8];
    let hour = &value[8..10];
    let minute = &value[10..12];
    let second = &value[12..14];
    Some(format!("{year}-{month}-{day} {hour}:{minute}:{second}"))
}

fn parse_iso_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let raw = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(raw.try_into().ok()?))
}

fn parse_iso_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let raw = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes(raw.try_into().ok()?))
}

fn strip_iso_version_suffix(segment: &str) -> &str {
    let Some((base, suffix)) = segment.rsplit_once(';') else {
        return segment;
    };
    if !base.is_empty() && !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
        base
    } else {
        segment
    }
}

fn parse_torrent_root(
    bytes: &[u8],
    index: &mut usize,
    metadata: &mut TorrentMetadata,
    content_sample_limit: usize,
) -> Option<()> {
    expect_byte(bytes, index, b'd')?;
    while peek_byte(bytes, *index)? != b'e' {
        let key = parse_bencode_bytes(bytes, index)?;
        match key {
            b"announce" => metadata.announce = parse_bencode_string(bytes, index),
            b"announce-list" => parse_torrent_announce_list(bytes, index, metadata)?,
            b"comment" => metadata.comment = parse_bencode_string(bytes, index),
            b"comment.utf-8" => metadata.comment = parse_bencode_string(bytes, index),
            b"created by" => metadata.created_by = parse_bencode_string(bytes, index),
            b"info" => parse_torrent_info(bytes, index, metadata, content_sample_limit)?,
            _ => skip_bencode_value(bytes, index)?,
        }
    }
    expect_byte(bytes, index, b'e')
}

fn parse_torrent_info(
    bytes: &[u8],
    index: &mut usize,
    metadata: &mut TorrentMetadata,
    content_sample_limit: usize,
) -> Option<()> {
    expect_byte(bytes, index, b'd')?;
    while peek_byte(bytes, *index)? != b'e' {
        let key = parse_bencode_bytes(bytes, index)?;
        match key {
            b"name" => metadata.name = parse_bencode_string(bytes, index),
            b"name.utf-8" => metadata.name = parse_bencode_string(bytes, index),
            b"length" => {
                let length = parse_bencode_int(bytes, index)?;
                if length >= 0 {
                    metadata.total_size = Some(length as u64);
                    if metadata.file_count == 0 {
                        metadata.file_count = 1;
                    }
                    metadata.mode = Some(TorrentMode::SingleFile);
                }
            }
            b"files" => parse_torrent_files(bytes, index, metadata, content_sample_limit)?,
            b"piece length" => {
                let length = parse_bencode_int(bytes, index)?;
                if length >= 0 {
                    metadata.piece_length = Some(length as u64);
                }
            }
            b"pieces" => {
                let pieces = parse_bencode_bytes(bytes, index)?;
                metadata.piece_count = Some(pieces.len().div_ceil(20));
            }
            b"private" => {
                metadata.private = Some(parse_bencode_int(bytes, index)? != 0);
            }
            _ => skip_bencode_value(bytes, index)?,
        }
    }
    expect_byte(bytes, index, b'e')
}

fn parse_torrent_files(
    bytes: &[u8],
    index: &mut usize,
    metadata: &mut TorrentMetadata,
    content_sample_limit: usize,
) -> Option<()> {
    expect_byte(bytes, index, b'l')?;
    let mut file_count = 0usize;
    let mut total_size = 0u64;
    while peek_byte(bytes, *index)? != b'e' {
        let file = parse_torrent_file_entry(bytes, index)?;
        file_count += 1;
        total_size = total_size.saturating_add(file.length);
        if metadata.files.len() < content_sample_limit {
            metadata.files.push(file);
        } else {
            metadata.file_sample_truncated = true;
        }
    }
    expect_byte(bytes, index, b'e')?;
    metadata.file_count = file_count;
    metadata.total_size = Some(total_size);
    metadata.mode = Some(TorrentMode::MultiFile);
    Some(())
}

fn parse_torrent_file_entry(bytes: &[u8], index: &mut usize) -> Option<TorrentFileEntry> {
    expect_byte(bytes, index, b'd')?;
    let mut length = 0u64;
    let mut path = None;
    let mut path_utf8 = None;
    while peek_byte(bytes, *index)? != b'e' {
        let key = parse_bencode_bytes(bytes, index)?;
        match key {
            b"length" => {
                let value = parse_bencode_int(bytes, index)?;
                if value >= 0 {
                    length = value as u64;
                }
            }
            b"path" => path = parse_bencode_path(bytes, index),
            b"path.utf-8" => path_utf8 = parse_bencode_path(bytes, index),
            _ => skip_bencode_value(bytes, index)?,
        }
    }
    expect_byte(bytes, index, b'e')?;
    Some(TorrentFileEntry {
        path: path_utf8
            .or(path)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("file-{}", *index)),
        length,
    })
}

fn parse_torrent_announce_list(
    bytes: &[u8],
    index: &mut usize,
    metadata: &mut TorrentMetadata,
) -> Option<()> {
    expect_byte(bytes, index, b'l')?;
    while peek_byte(bytes, *index)? != b'e' {
        match peek_byte(bytes, *index)? {
            b'l' => {
                let tier = parse_bencode_string_list(bytes, index)?;
                if !tier.is_empty() {
                    metadata.announce_tiers.push(tier);
                }
            }
            b'0'..=b'9' => {
                if let Some(tracker) =
                    parse_bencode_string(bytes, index).filter(|value| !value.is_empty())
                {
                    metadata.announce_tiers.push(vec![tracker]);
                }
            }
            _ => skip_bencode_value(bytes, index)?,
        }
    }
    expect_byte(bytes, index, b'e')
}

fn parse_bencode_path(bytes: &[u8], index: &mut usize) -> Option<String> {
    let segments = parse_bencode_string_list(bytes, index)?;
    let joined = segments
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    (!joined.is_empty()).then_some(joined)
}

fn parse_bencode_string_list(bytes: &[u8], index: &mut usize) -> Option<Vec<String>> {
    expect_byte(bytes, index, b'l')?;
    let mut items = Vec::new();
    while peek_byte(bytes, *index)? != b'e' {
        items.push(parse_bencode_string(bytes, index)?);
    }
    expect_byte(bytes, index, b'e')?;
    Some(items)
}

fn skip_bencode_value(bytes: &[u8], index: &mut usize) -> Option<()> {
    match peek_byte(bytes, *index)? {
        b'd' => {
            *index += 1;
            while peek_byte(bytes, *index)? != b'e' {
                parse_bencode_bytes(bytes, index)?;
                skip_bencode_value(bytes, index)?;
            }
            *index += 1;
            Some(())
        }
        b'l' => {
            *index += 1;
            while peek_byte(bytes, *index)? != b'e' {
                skip_bencode_value(bytes, index)?;
            }
            *index += 1;
            Some(())
        }
        b'i' => {
            parse_bencode_int(bytes, index)?;
            Some(())
        }
        b'0'..=b'9' => {
            parse_bencode_bytes(bytes, index)?;
            Some(())
        }
        _ => None,
    }
}

fn parse_bencode_string(bytes: &[u8], index: &mut usize) -> Option<String> {
    let value = parse_bencode_bytes(bytes, index)?;
    Some(String::from_utf8_lossy(value).into_owned())
}

fn parse_bencode_bytes<'a>(bytes: &'a [u8], index: &mut usize) -> Option<&'a [u8]> {
    let start = *index;
    while peek_byte(bytes, *index)?.is_ascii_digit() {
        *index += 1;
    }
    let colon = *index;
    expect_byte(bytes, index, b':')?;
    let len = std::str::from_utf8(&bytes[start..colon])
        .ok()?
        .parse::<usize>()
        .ok()?;
    let end = (*index).checked_add(len)?;
    let slice = bytes.get(*index..end)?;
    *index = end;
    Some(slice)
}

fn parse_bencode_int(bytes: &[u8], index: &mut usize) -> Option<i64> {
    expect_byte(bytes, index, b'i')?;
    let start = *index;
    while peek_byte(bytes, *index)? != b'e' {
        *index += 1;
    }
    let value = std::str::from_utf8(bytes.get(start..*index)?)
        .ok()?
        .parse()
        .ok()?;
    expect_byte(bytes, index, b'e')?;
    Some(value)
}

fn peek_byte(bytes: &[u8], index: usize) -> Option<u8> {
    bytes.get(index).copied()
}

fn expect_byte(bytes: &[u8], index: &mut usize, expected: u8) -> Option<()> {
    if peek_byte(bytes, *index)? == expected {
        *index += 1;
        Some(())
    } else {
        None
    }
}
