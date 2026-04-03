use super::*;
use crate::core::{Entry, EntryKind};
use crate::ui::theme;
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::fs;

pub(super) fn build_directory_preview(entry: &Entry) -> PreviewContent {
    match fs::read_dir(&entry.path) {
        Ok(children) => {
            // Collect at most PREVIEW_RENDER_LINE_LIMIT + 1 entries so we can detect
            // truncation without reading potentially thousands of entries (e.g. /proc).
            // Use child.file_type() instead of path.is_dir() to avoid an extra stat()
            // syscall per entry — file_type() uses d_type from getdents64 directly.
            let mut items = children
                .flatten()
                .take(PREVIEW_RENDER_LINE_LIMIT + 1)
                .map(|child| {
                    let path = child.path();
                    let file_name = child.file_name().to_string_lossy().to_string();
                    let is_dir = child.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                    (file_name, path, is_dir)
                })
                .collect::<Vec<_>>();
            let scan_truncated = items.len() > PREVIEW_RENDER_LINE_LIMIT;
            if scan_truncated {
                items.pop();
            }
            items.sort_by(|left, right| {
                right
                    .2
                    .cmp(&left.2)
                    .then_with(|| left.0.to_lowercase().cmp(&right.0.to_lowercase()))
            });

            if items.is_empty() {
                return super::status_preview(
                    PreviewKind::Directory,
                    "0 items",
                    [Line::from("Folder is empty")],
                );
            }

            let palette = theme::palette();
            let total_items = items.len();
            let folder_count = items.iter().filter(|item| item.2).count();
            let file_count = total_items.saturating_sub(folder_count);
            let mut lines = Vec::new();
            for (name, path, is_dir) in items.into_iter() {
                let appearance = theme::resolve_path(
                    &path,
                    if is_dir {
                        EntryKind::Directory
                    } else {
                        EntryKind::File
                    },
                );
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", appearance.icon),
                        Style::default()
                            .fg(appearance.color)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                    Span::styled(name, Style::default().fg(palette.text)),
                ]));
            }

            let detail = if scan_truncated {
                format!("{total_items}+ items")
            } else {
                format!("{total_items} items")
            };
            let mut preview = PreviewContent::new(PreviewKind::Directory, lines)
                .with_detail(detail)
                .with_directory_counts(total_items, folder_count, file_count);
            if scan_truncated {
                preview = preview
                    .with_truncation(format!("showing first {PREVIEW_RENDER_LINE_LIMIT} items"));
            }
            preview
        }
        Err(error) => super::unavailable_directory_preview(&error),
    }
}
