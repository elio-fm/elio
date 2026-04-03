use super::*;
use crate::preview::appearance as theme;
use ratatui::text::Line;
use std::{fs::File, io::Read, path::Path};

#[derive(Default)]
struct TorrentMetadata {
    name: Option<String>,
    announce: Option<String>,
    announce_tiers: Vec<Vec<String>>,
    comment: Option<String>,
    created_by: Option<String>,
    piece_length: Option<u64>,
    piece_count: Option<usize>,
    private: Option<bool>,
    mode: Option<TorrentMode>,
    file_count: usize,
    total_size: Option<u64>,
    files: Vec<TorrentFileEntry>,
    file_sample_truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TorrentMode {
    SingleFile,
    MultiFile,
}

#[derive(Clone, Debug)]
struct TorrentFileEntry {
    path: String,
    length: u64,
}

pub(in crate::preview) fn build_torrent_preview(path: &Path) -> Option<PreviewContent> {
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

    let palette = theme::palette();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalize_deduplicates_trackers_and_synthesizes_single_file_contents() {
        let mut metadata = TorrentMetadata {
            name: Some("elio-v1.0.0.tar.gz".to_string()),
            announce: Some("https://tracker.example/primary".to_string()),
            announce_tiers: vec![
                vec![
                    "https://tracker.example/backup".to_string(),
                    "https://tracker.example/primary".to_string(),
                ],
                vec![
                    "".to_string(),
                    "https://tracker.example/backup".to_string(),
                    "https://tracker.example/third".to_string(),
                ],
            ],
            total_size: Some(4_096),
            ..TorrentMetadata::default()
        };

        metadata.finalize();

        assert_eq!(metadata.mode, Some(TorrentMode::SingleFile));
        assert_eq!(
            metadata.announce_tiers,
            vec![
                vec![
                    "https://tracker.example/backup".to_string(),
                    "https://tracker.example/primary".to_string(),
                ],
                vec!["https://tracker.example/third".to_string()],
            ]
        );
        assert_eq!(metadata.file_count, 1);
        assert_eq!(metadata.files.len(), 1);
        assert_eq!(metadata.files[0].path, "elio-v1.0.0.tar.gz");
        assert_eq!(metadata.files[0].length, 4_096);
        assert_eq!(
            metadata.tracker_fields(),
            vec![
                (
                    "Tier 1".to_string(),
                    "https://tracker.example/backup  •  https://tracker.example/primary"
                        .to_string(),
                ),
                (
                    "Tier 2".to_string(),
                    "https://tracker.example/third".to_string(),
                ),
            ]
        );
    }

    #[test]
    fn multifile_content_entries_are_rooted_under_torrent_name() {
        let metadata = TorrentMetadata {
            name: Some("elio-release".to_string()),
            mode: Some(TorrentMode::MultiFile),
            file_count: 2,
            files: vec![
                TorrentFileEntry {
                    path: "docs/readme.txt".to_string(),
                    length: 12,
                },
                TorrentFileEntry {
                    path: "bin/elio".to_string(),
                    length: 34,
                },
            ],
            ..TorrentMetadata::default()
        };

        let entries = metadata.content_entries();

        assert_eq!(
            entries,
            vec![
                ArchiveEntry {
                    path: "elio-release/docs/readme.txt".to_string(),
                    is_dir: false,
                },
                ArchiveEntry {
                    path: "elio-release/bin/elio".to_string(),
                    is_dir: false,
                },
            ]
        );
    }
}
