use super::*;
use crate::preview::appearance as theme;
use ratatui::text::Line;

pub(super) fn render_archive_preview(config: ArchiveRenderConfig) -> PreviewContent {
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
            config.metadata.compressed_size.map(crate::fs::format_size),
        ),
        (
            "Unpacked",
            config.metadata.unpacked_size.map(crate::fs::format_size),
        ),
        (
            "Archive Size",
            config.metadata.physical_size.map(crate::fs::format_size),
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
