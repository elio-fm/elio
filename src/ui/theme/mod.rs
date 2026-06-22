mod appearance;
mod builtin_themes;

use crate::core::{Entry, EntryKind, FileClass, SymlinkInfo};
use ratatui::style::Color;
use std::path::Path;

pub(crate) use self::appearance::{
    Palette, code_preview_palette, initialize, palette, resolve_browser_entry, resolve_entry,
    resolve_path, resolve_path_with_class,
};

pub(super) fn mix_color(base: Color, tint: Color, tint_weight: u8) -> Color {
    match (base, tint) {
        (Color::Rgb(br, bg, bb), Color::Rgb(tr, tg, tb)) => {
            let weight = u16::from(tint_weight);
            let base_weight = 255 - weight;
            Color::Rgb(
                ((u16::from(br) * base_weight + u16::from(tr) * weight) / 255) as u8,
                ((u16::from(bg) * base_weight + u16::from(tg) * weight) / 255) as u8,
                ((u16::from(bb) * base_weight + u16::from(tb) * weight) / 255) as u8,
            )
        }
        _ => base,
    }
}

/// Badge character and color for a file's git status, shown next to its name
/// in the browser. Colors mirror the diff highlighting in git previews.
pub(super) fn git_status_badge(status: crate::app::GitFileStatus) -> (char, Color) {
    use crate::app::GitFileStatus;
    let color = match status {
        GitFileStatus::Added => Color::Green,
        GitFileStatus::Modified => Color::Yellow,
        GitFileStatus::Deleted => Color::Red,
        GitFileStatus::Untracked => Color::Cyan,
        GitFileStatus::Renamed => Color::Magenta,
        GitFileStatus::Conflicted => Color::LightRed,
    };
    (status.badge(), color)
}

pub(super) fn entry_color(entry: &Entry, palette: Palette) -> Color {
    let _ = palette;
    resolve_entry(entry).color
}

pub(super) fn entry_symbol(entry: &Entry) -> &'static str {
    resolve_entry(entry).icon
}

pub(super) fn path_color(path: &Path, is_dir: bool, palette: Palette) -> Color {
    let _ = palette;
    let kind = if is_dir {
        EntryKind::Directory
    } else {
        EntryKind::File
    };
    resolve_path(path, kind).color
}

pub(super) fn path_symbol(path: &Path, is_dir: bool) -> &'static str {
    let kind = if is_dir {
        EntryKind::Directory
    } else {
        EntryKind::File
    };
    resolve_path(path, kind).icon
}

pub(super) fn path_symbol_with_symlink(
    path: &Path,
    is_dir: bool,
    symlink: Option<&SymlinkInfo>,
) -> &'static str {
    let kind = if is_dir {
        EntryKind::Directory
    } else {
        EntryKind::File
    };
    match symlink_file_class(symlink) {
        Some(class) => resolve_path_with_class(path, kind, class).icon,
        None => resolve_path(path, kind).icon,
    }
}

pub(super) fn path_color_with_symlink(
    path: &Path,
    is_dir: bool,
    symlink: Option<&SymlinkInfo>,
    palette: Palette,
) -> Color {
    let _ = palette;
    let kind = if is_dir {
        EntryKind::Directory
    } else {
        EntryKind::File
    };
    match symlink_file_class(symlink) {
        Some(class) => resolve_path_with_class(path, kind, class).color,
        None => resolve_path(path, kind).color,
    }
}

fn symlink_file_class(symlink: Option<&SymlinkInfo>) -> Option<FileClass> {
    let symlink = symlink?;
    match symlink.target_kind {
        Some(EntryKind::Directory) => Some(FileClass::SymlinkDirectory),
        None => Some(FileClass::BrokenSymlink),
        Some(EntryKind::File) => None,
    }
}
