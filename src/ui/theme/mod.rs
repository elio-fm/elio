mod appearance;

use crate::app::Entry;
use ratatui::style::Color;
use std::path::Path;

#[cfg(test)]
pub(crate) use self::appearance::classify_path;
pub(crate) use self::appearance::{
    CodePreviewPalette, Palette, code_preview_palette, initialize, palette, resolve_path,
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

pub(super) fn entry_color(entry: &Entry, palette: Palette) -> Color {
    let _ = palette;
    resolve_path(&entry.path, entry.kind).color
}

pub(super) fn entry_symbol(entry: &Entry) -> &'static str {
    resolve_path(&entry.path, entry.kind).icon
}

pub(super) fn path_color(path: &Path, is_dir: bool, palette: Palette) -> Color {
    let _ = palette;
    let kind = if is_dir {
        crate::app::EntryKind::Directory
    } else {
        crate::app::EntryKind::File
    };
    resolve_path(path, kind).color
}

pub(super) fn path_symbol(path: &Path, is_dir: bool) -> &'static str {
    let kind = if is_dir {
        crate::app::EntryKind::Directory
    } else {
        crate::app::EntryKind::File
    };
    resolve_path(path, kind).icon
}
