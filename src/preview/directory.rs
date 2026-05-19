use super::{appearance, *};
use crate::{
    core::{Entry, EntryKind},
    fs as browser_support,
};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::fs;

struct DirectoryPreviewItem {
    entry: Entry,
    target_label: Option<String>,
}

pub(super) fn build_directory_preview(entry: &Entry) -> PreviewContent {
    match fs::read_dir(&entry.path) {
        Ok(children) => {
            // Collect at most PREVIEW_RENDER_LINE_LIMIT + 1 entries so we can detect
            // truncation without reading potentially thousands of entries (e.g. /proc).
            let mut items = children
                .flatten()
                .filter_map(directory_preview_item_from_child)
                .take(PREVIEW_RENDER_LINE_LIMIT + 1)
                .collect::<Vec<_>>();
            let scan_truncated = items.len() > PREVIEW_RENDER_LINE_LIMIT;
            if scan_truncated {
                items.pop();
            }
            items.sort_by(|left, right| {
                right
                    .entry
                    .is_dir()
                    .cmp(&left.entry.is_dir())
                    .then_with(|| left.entry.name_key.cmp(&right.entry.name_key))
                    .then_with(|| left.entry.name.cmp(&right.entry.name))
            });

            if items.is_empty() {
                return super::status_preview(
                    PreviewKind::Directory,
                    "0 items",
                    [Line::from("Folder is empty")],
                );
            }

            let palette = appearance::palette();
            let total_items = items.len();
            let folder_count = items.iter().filter(|item| item.entry.is_dir()).count();
            let file_count = total_items.saturating_sub(folder_count);
            let mut lines = Vec::new();
            for item in items.into_iter() {
                let path_appearance = directory_preview_item_appearance(&item);
                let name = directory_preview_item_label(&item.entry, item.target_label.as_deref());
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", path_appearance.icon),
                        Style::default()
                            .fg(path_appearance.color)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                    Span::styled(name, Style::default().fg(palette.text)),
                ]));
            }

            let mut preview = PreviewContent::new(PreviewKind::Directory, lines)
                .with_directory_counts(total_items, folder_count, file_count);
            if !scan_truncated {
                preview = preview.with_detail(format!("{total_items} items"));
            }
            if scan_truncated {
                preview =
                    preview.with_truncation(format!("{PREVIEW_RENDER_LINE_LIMIT} items shown"));
            }
            preview
        }
        Err(error) => super::unavailable_directory_preview(&error),
    }
}

fn directory_preview_item_from_child(child: fs::DirEntry) -> Option<DirectoryPreviewItem> {
    let path = child.path();
    let name = child.file_name().to_string_lossy().to_string();
    // DirEntry::file_type() avoids following each child for normal entries.
    // Symlinks intentionally go through entry_from_path() so preview rows can
    // show the link target and resolved target kind.
    let file_type = child.file_type().ok()?;

    if file_type.is_symlink() {
        let entry = browser_support::entry_from_path(path, name).ok()?;
        let target_label = entry
            .symlink
            .as_ref()
            .map(browser_support::symlink_target_display_label);
        return Some(DirectoryPreviewItem {
            entry,
            target_label,
        });
    }

    let kind = if file_type.is_dir() {
        EntryKind::Directory
    } else {
        EntryKind::File
    };
    Some(DirectoryPreviewItem {
        entry: Entry {
            path,
            name_key: name.to_lowercase(),
            name,
            kind,
            ..Entry::default()
        },
        target_label: None,
    })
}

fn directory_preview_item_appearance(
    item: &DirectoryPreviewItem,
) -> appearance::PathAppearance<'static> {
    if item.entry.is_symlink() {
        appearance::resolve_entry(&item.entry)
    } else {
        appearance::resolve_path(&item.entry.path, item.entry.kind)
    }
}

fn directory_preview_item_label(entry: &Entry, target_label: Option<&str>) -> String {
    let name = browser_support::sanitize_terminal_text(&entry.name);
    target_label
        .map(|target| format!("{name} -> {target}"))
        .unwrap_or(name)
}
