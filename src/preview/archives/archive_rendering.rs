use super::super::{
    PREVIEW_RENDER_LINE_LIMIT, PreviewContent, PreviewKind, trim_trailing_line_endings,
};
use crate::{
    file_classification::{self, FileClass},
    filesystem::EntryKind,
    preview::appearance as theme,
};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::preview) struct ArchiveEntry {
    pub(in crate::preview) path: String,
    pub(in crate::preview) is_dir: bool,
}

#[derive(Default)]
pub(in crate::preview) struct ArchiveTreeNode {
    pub(in crate::preview) path: String,
    pub(in crate::preview) is_dir: bool,
    pub(in crate::preview) children: BTreeMap<String, ArchiveTreeNode>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ArchiveMetadata {
    pub(super) format_label: Option<String>,
    pub(super) physical_size: Option<u64>,
    pub(super) compressed_size: Option<u64>,
    pub(super) unpacked_size: Option<u64>,
    pub(super) comment: Option<String>,
}

pub(super) fn normalize_archive_path(item: &str, strip_version_suffix: bool) -> Option<String> {
    normalize_archive_entry(item, strip_version_suffix).map(|entry| entry.path)
}
pub(in crate::preview) fn normalize_archive_entries<'a>(
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

pub(in crate::preview) fn insert_archive_tree_entry(
    root: &mut ArchiveTreeNode,
    entry: &ArchiveEntry,
) {
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

pub(in crate::preview) fn ordered_archive_children(
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

pub(in crate::preview) fn render_archive_tree(
    children: &[(&String, &ArchiveTreeNode)],
    prefix: &str,
    remaining: &mut usize,
    rendered_items: &mut usize,
    lines: &mut Vec<Line<'static>>,
    palette: theme::Palette,
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
    palette: theme::Palette,
) -> Line<'static> {
    let connector = if is_last { "└── " } else { "├── " };
    let path = Path::new(&node.path);
    let kind = if node.is_dir {
        EntryKind::Directory
    } else {
        EntryKind::File
    };
    let license_entry = !node.is_dir
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(file_classification::is_canonical_license_file_name);
    let appearance = if license_entry {
        theme::resolve_path_with_class(path, kind, FileClass::License)
    } else {
        theme::resolve_path(path, kind)
    };
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

pub(in crate::preview) fn push_preview_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(&str, Option<String>)],
    palette: theme::Palette,
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
    palette: theme::Palette,
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

pub(in crate::preview) fn push_preview_owned_values_section(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[(String, String)],
    palette: theme::Palette,
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

pub(in crate::preview) fn section_line(title: &str, palette: theme::Palette) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(palette.accent),
    ))
}

fn preview_field_line(
    label: &str,
    value: &str,
    label_width: usize,
    palette: theme::Palette,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<width$} ", width = label_width + 1),
            Style::default().fg(palette.muted),
        ),
        Span::styled(value.to_string(), Style::default().fg(palette.text)),
    ])
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
pub(super) fn render_archive_preview(config: ArchiveRenderConfig) -> PreviewContent {
    let palette = theme::palette();
    let mut lines = Vec::new();
    let entries = config.entries.map(expand_archive_entries);
    let total_items = entries
        .as_ref()
        .map(Vec::len)
        .unwrap_or(0)
        .max(config.total_entries_hint.unwrap_or(0));
    let folder_count = entries
        .as_ref()
        .map(|entries| entries.iter().filter(|entry| entry.is_dir).count())
        .unwrap_or(0);
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
            config
                .metadata
                .compressed_size
                .map(crate::filesystem::format_size),
        ),
        (
            "Unpacked",
            config
                .metadata
                .unpacked_size
                .map(crate::filesystem::format_size),
        ),
        (
            "Archive Size",
            config
                .metadata
                .physical_size
                .map(crate::filesystem::format_size),
        ),
        ("Comment", config.metadata.comment),
    ];
    push_preview_section(&mut lines, "Details", &summary, palette);

    for (title, fields) in config.extra_sections {
        push_preview_values_section(&mut lines, title, &fields, palette);
    }

    let mut rendered_items = 0usize;
    let mut tree_truncated = false;
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(section_line("Contents", palette));

    match &entries {
        None => {
            lines.push(Line::from(config.unavailable_label.to_string()));
        }
        Some(entries) if entries.is_empty() => {
            lines.push(Line::from(if total_items == 0 {
                config.empty_label.to_string()
            } else {
                config.unavailable_label.to_string()
            }));
        }
        Some(entries) => {
            let mut root = ArchiveTreeNode::default();
            for entry in entries {
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
    }

    let entry_count = entries.as_ref().map(Vec::len).unwrap_or(0);
    let mut notes = Vec::new();
    if config.scan_truncated {
        notes.push(format!(
            "scanned first {} of {} entries",
            entry_count, total_items
        ));
    }
    if tree_truncated {
        notes.push(format!(
            "showing first {} of {} entries",
            rendered_items.max(entry_count.min(PREVIEW_RENDER_LINE_LIMIT)),
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

pub(super) struct ArchiveRenderConfig {
    pub(super) detail: String,
    pub(super) metadata: ArchiveMetadata,
    pub(super) entries: Option<Vec<ArchiveEntry>>,
    pub(super) total_entries_hint: Option<usize>,
    pub(super) empty_label: &'static str,
    pub(super) unavailable_label: &'static str,
    pub(super) extra_sections: Vec<(&'static str, Vec<(&'static str, String)>)>,
    pub(super) scan_truncated: bool,
}
