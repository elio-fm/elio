use crate::app::{Entry, folder_color};
use ratatui::style::Color;
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub bg: Color,
    pub chrome: Color,
    pub chrome_alt: Color,
    pub panel: Color,
    pub panel_alt: Color,
    pub surface: Color,
    pub elevated: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub accent_text: Color,
    pub selected_bg: Color,
    pub selected_border: Color,
    pub sidebar_active: Color,
    pub button_bg: Color,
    pub button_disabled_bg: Color,
    pub path_bg: Color,
}

pub(super) fn palette() -> Palette {
    Palette {
        bg: Color::Rgb(10, 14, 20),
        chrome: Color::Rgb(16, 21, 30),
        chrome_alt: Color::Rgb(24, 32, 43),
        panel: Color::Rgb(18, 25, 35),
        panel_alt: Color::Rgb(14, 20, 28),
        surface: Color::Rgb(22, 30, 41),
        elevated: Color::Rgb(27, 37, 50),
        border: Color::Rgb(49, 67, 87),
        text: Color::Rgb(238, 243, 248),
        muted: Color::Rgb(158, 172, 189),
        accent: Color::Rgb(102, 186, 255),
        accent_soft: Color::Rgb(34, 57, 79),
        accent_text: Color::Rgb(207, 234, 255),
        selected_bg: Color::Rgb(36, 56, 78),
        selected_border: Color::Rgb(112, 196, 255),
        sidebar_active: Color::Rgb(31, 47, 65),
        button_bg: Color::Rgb(29, 39, 52),
        button_disabled_bg: Color::Rgb(20, 27, 37),
        path_bg: Color::Rgb(28, 37, 49),
    }
}

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
    if entry.is_dir() {
        palette.accent
    } else {
        folder_color(entry)
    }
}

pub(super) fn entry_symbol(entry: &Entry) -> &'static str {
    if entry.is_dir() { "󰉋" } else { "󰈔" }
}

pub(super) fn path_color(path: &Path, is_dir: bool, palette: Palette) -> Color {
    if is_dir {
        return palette.accent;
    }

    match extension_class(path) {
        "image" => Color::Rgb(86, 156, 214),
        "audio" => Color::Rgb(138, 110, 214),
        "video" => Color::Rgb(204, 112, 79),
        "archive" => Color::Rgb(191, 142, 74),
        "code" => Color::Rgb(76, 152, 120),
        _ => Color::Rgb(98, 109, 122),
    }
}

fn extension_class(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" | "js" | "ts" | "tsx" | "jsx" | "py" | "go" | "c" | "cpp" | "h" | "java" | "json"
        | "toml" | "yaml" | "yml" | "md" | "sh" => "code",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "avif" => "image",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "audio",
        "mp4" | "mkv" | "mov" | "webm" | "avi" => "video",
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" => "archive",
        "txt" | "log" | "ini" | "csv" => "text",
        _ => "file",
    }
}
