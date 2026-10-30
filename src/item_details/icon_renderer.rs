#[cfg(unix)]
use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    process::{Command, Stdio},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use std::path::PathBuf;

#[cfg(unix)]
use fontdue::{Font, FontSettings};

#[cfg(unix)]
use image::{ImageEncoder, codecs::png::PngEncoder};
use ratatui::style::Color;

#[cfg(unix)]
const ICON_FONT_SIZE: f32 = 56.0;
#[cfg(unix)]
const ICON_HEIGHT: u32 = 72;
#[cfg(unix)]
const MAX_ICON_WIDTH: u32 = 512;
#[cfg(unix)]
type RasterizedGlyphs = Vec<(fontdue::Metrics, Vec<u8>)>;

pub(crate) fn render_icon_png(icon: &str, color: Color) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        write_png(render_with_system_font(icon, color)?)
    }
    #[cfg(not(unix))]
    {
        let _ = icon;
        let _ = color;
        None
    }
}

pub(crate) fn render_icon_png_for_cell_area(
    icon: &str,
    color: Color,
    width: u32,
    height: u32,
) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        if width == 0 || height == 0 || width > MAX_ICON_WIDTH || height > MAX_ICON_WIDTH {
            return None;
        }
        write_png(render_with_system_font_in_canvas(
            icon, color, width, height,
        )?)
    }
    #[cfg(not(unix))]
    {
        let _ = (icon, color, width, height);
        None
    }
}

pub(crate) fn prewarm_icon_renderer() {
    #[cfg(unix)]
    {
        let _ = std::thread::Builder::new()
            .name("elio-item-icon-prewarm".to_owned())
            .spawn(|| {
                for ch in "󰆍󰒓󰈔󰈙".chars() {
                    let _ = font_for_character(ch);
                }
            });
    }
}

#[cfg(unix)]
fn render_with_system_font(icon: &str, color: Color) -> Option<(u32, u32, Vec<u8>)> {
    render_with_system_font_in_canvas(icon, color, 0, ICON_HEIGHT)
}

#[cfg(unix)]
fn render_with_system_font_in_canvas(
    icon: &str,
    color: Color,
    canvas_width: u32,
    canvas_height: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    let fixed_width = canvas_width > 0;
    let mut font_size = if fixed_width {
        ICON_FONT_SIZE * (canvas_height as f32 / ICON_HEIGHT as f32)
    } else {
        ICON_FONT_SIZE
    };
    let (mut glyphs, mut min_x, mut max_x) = rasterize_glyphs(icon, font_size)?;
    let mut padding = 4.0f32 * (font_size / ICON_FONT_SIZE);
    let mut natural_width = (max_x - min_x + padding * 2.0).ceil() as u32;
    if fixed_width && natural_width > canvas_width {
        font_size *= canvas_width as f32 / natural_width as f32;
        (glyphs, min_x, max_x) = rasterize_glyphs(icon, font_size)?;
        padding = 4.0f32 * (font_size / ICON_FONT_SIZE);
        natural_width = (max_x - min_x + padding * 2.0).ceil() as u32;
    }
    let width = if fixed_width {
        canvas_width
    } else {
        natural_width
    };
    if width == 0 || width > MAX_ICON_WIDTH {
        return None;
    }
    let height = if fixed_width {
        canvas_height
    } else {
        ICON_HEIGHT
    };
    let pixel_len = usize::try_from(width)
        .ok()?
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    let mut pixels = vec![0; pixel_len];
    let [r, g, b] = color_rgb(color);
    let x_offset = if fixed_width {
        ((canvas_width as f32 - natural_width as f32) / 2.0).max(0.0)
    } else {
        0.0
    };
    let mut cursor_x = padding - min_x + x_offset;
    for (metrics, bitmap) in glyphs {
        let glyph_x = cursor_x.round() as i32 + metrics.xmin;
        let glyph_y = (height as i32 - metrics.height as i32) / 2;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let x = glyph_x + col as i32;
                let y = glyph_y + row as i32;
                if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                    continue;
                }
                let alpha = bitmap[row * metrics.width + col];
                let offset = (y as usize * width as usize + x as usize) * 4;
                pixels[offset..offset + 4].copy_from_slice(&[r, g, b, alpha]);
            }
        }
        cursor_x += metrics.advance_width.max(0.0);
    }
    Some((width, height, pixels))
}

#[cfg(unix)]
fn rasterize_glyphs(icon: &str, font_size: f32) -> Option<(RasterizedGlyphs, f32, f32)> {
    let mut glyphs = Vec::new();
    let mut advance_x = 0.0f32;
    let mut min_x = 0.0f32;
    let mut max_x = 0.0f32;
    for ch in icon.chars() {
        let font = font_for_character(ch)?;
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        let glyph_left = advance_x + metrics.xmin as f32;
        min_x = min_x.min(glyph_left);
        max_x = max_x.max(glyph_left + metrics.width as f32);
        advance_x += metrics.advance_width.max(0.0);
        max_x = max_x.max(advance_x);
        glyphs.push((metrics, bitmap));
    }
    Some((glyphs, min_x, max_x))
}

#[cfg(unix)]
fn font_for_character(ch: char) -> Option<std::sync::Arc<Font>> {
    use std::collections::HashMap;

    static FONTS: OnceLock<Mutex<HashMap<PathBuf, std::sync::Arc<Font>>>> = OnceLock::new();
    let path = font_path_for_character(ch)?;
    let fonts = FONTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(font) = fonts.lock().ok()?.get(&path).cloned() {
        return Some(font);
    }
    let bytes = fs::read(&path).ok()?;
    let font = std::sync::Arc::new(Font::from_bytes(bytes, FontSettings::default()).ok()?);
    fonts.lock().ok()?.insert(path, font.clone());
    Some(font)
}

#[cfg(unix)]
fn color_rgb(color: Color) -> [u8; 3] {
    match color {
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Black => [0, 0, 0],
        Color::Red => [220, 76, 76],
        Color::Green => [87, 201, 87],
        Color::Yellow => [245, 216, 91],
        Color::Blue => [91, 168, 255],
        Color::Magenta => [255, 134, 216],
        Color::Cyan => [36, 217, 184],
        Color::Gray => [140, 151, 168],
        Color::DarkGray => [48, 48, 48],
        Color::LightRed => [255, 107, 107],
        Color::LightGreen => [141, 223, 109],
        Color::LightYellow => [255, 216, 102],
        Color::LightBlue => [156, 188, 255],
        Color::LightMagenta => [215, 142, 255],
        Color::LightCyan => [138, 231, 255],
        Color::White | Color::Reset | Color::Indexed(_) => [231, 237, 245],
    }
}

#[cfg(unix)]
fn write_png((width, height, pixels): (u32, u32, Vec<u8>)) -> Option<PathBuf> {
    static NEXT_ICON_FILE: AtomicU64 = AtomicU64::new(0);

    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    for _ in 0..8 {
        let serial = NEXT_ICON_FILE.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "elio-item-icon-{}-{serial}.png",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(&png).ok()?;
                return Some(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

#[cfg(unix)]
fn font_path_for_character(ch: char) -> Option<PathBuf> {
    static FONT_PATHS: OnceLock<Mutex<std::collections::HashMap<char, PathBuf>>> = OnceLock::new();
    let cache = FONT_PATHS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Some(path) = cache.lock().ok()?.get(&ch).cloned() {
        return Some(path);
    }

    let charset = format!("charset={:x}", ch as u32);
    let path = if is_private_use_character(ch) {
        fontconfig_match(
            &format!("Symbols Nerd Font:{charset}"),
            Some("Symbols Nerd Font"),
        )
        .or_else(|| fontconfig_nerd_font_fallback(&charset))
        .or_else(|| fontconfig_match(&format!(":{charset}"), None))?
    } else {
        fontconfig_match(&format!(":{charset}"), None)?
    };
    cache.lock().ok()?.insert(ch, path.clone());
    Some(path)
}

#[cfg(unix)]
fn fontconfig_nerd_font_fallback(charset: &str) -> Option<PathBuf> {
    let output = Command::new("fc-match")
        .arg("-s")
        .arg("-f")
        .arg("%{family}\n%{file}\n")
        .arg(format!(":{charset}"))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let output = String::from_utf8_lossy(&output.stdout);
    let mut lines = output.lines().map(str::to_owned);
    while let (Some(family), Some(path)) = (lines.next(), lines.next()) {
        if family.contains("Nerd Font") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(unix)]
fn fontconfig_match(query: &str, required_family: Option<&str>) -> Option<PathBuf> {
    let output = Command::new("fc-match")
        .arg("-f")
        .arg("%{family}\n%{file}\n")
        .arg(query)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8_lossy(&output.stdout);
    let mut lines = output.lines();
    let family = lines.next()?;
    if required_family.is_some_and(|required| !family.contains(required)) {
        return None;
    }
    let path = PathBuf::from(lines.next()?);
    path.exists().then_some(path)
}

#[cfg(unix)]
fn is_private_use_character(ch: char) -> bool {
    matches!(ch as u32, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD)
}
