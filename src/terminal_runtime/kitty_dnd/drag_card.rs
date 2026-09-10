use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    thread,
};

use fontdue::{Font, FontSettings};
use image::{ImageEncoder, codecs::png::PngEncoder};
use ratatui::style::Color;

const FONT_SIZE: f32 = 28.0;
const ICON_SIZE: f32 = 30.0;
const ICON_SLOT_WIDTH: u32 = 32;
const PADDING_X: u32 = 18;
const ICON_TEXT_GAP: u32 = 9;
const WIDE_ICON_TEXT_GAP: u32 = 12;
const RADIUS: f32 = 13.0;
const MAX_TEXT_CHARS: usize = 30;
const PREWARM_DRAG_ICONS: &str = "󰉋󰌺󰆍󰒓󰈙󰿃󰋩󰎆󰀼󰛖󰆼󰈔󰉓";

pub(in crate::terminal_runtime) struct DragImage {
    pub(in crate::terminal_runtime) png: Vec<u8>,
    pub(in crate::terminal_runtime) width: u32,
    pub(in crate::terminal_runtime) height: u32,
}

struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: u32,
    height: u32,
}

#[derive(Clone, Copy)]
struct Point {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

pub(in crate::terminal_runtime) fn prewarm_drag_image_renderer() {
    let _ = thread::Builder::new()
        .name("elio-drag-image-prewarm".to_string())
        .spawn(|| {
            if let Some(fonts) = font_cache().get_or_init(load_font_from_disk).as_ref() {
                let _ = load_glyph_fallbacks_for_text(fonts, PREWARM_DRAG_ICONS);
            }
        });
}

pub(in crate::terminal_runtime) fn render_drag_image(
    icon: &str,
    text: &str,
    icon_color: Color,
    card_color: Color,
    text_color: Color,
) -> Option<DragImage> {
    let base_fonts = loaded_fonts()?;
    let fonts = RenderFontSet {
        base: base_fonts,
        glyph_fallbacks: load_glyph_fallbacks_for_text(base_fonts, icon),
    };
    if !can_render_text(&fonts, icon) {
        return None;
    }

    let style = resolve_card_style(card_color, text_color);
    let text = truncate_text(text, MAX_TEXT_CHARS);
    let icon_width = measure_text(&fonts, icon, ICON_SIZE).ceil() as u32;
    let icon_slot_width = icon_slot_width(icon_width);
    let text_width = measure_text(&fonts, &text, FONT_SIZE).ceil() as u32;
    let icon_text_gap = icon_text_gap(icon_slot_width);
    let width = PADDING_X * 2 + icon_slot_width + icon_text_gap + text_width;
    let height = 48;
    let mut pixels = vec![0u8; width as usize * height as usize * 4];

    let mut canvas = Canvas {
        pixels: &mut pixels,
        width,
        height,
    };

    draw_rounded_rect(
        &mut canvas,
        Rect {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
        },
        RADIUS,
        style.bg,
    );

    let baseline = 32.0;
    draw_text(
        &mut canvas,
        &fonts,
        icon,
        ICON_SIZE,
        Point {
            x: (PADDING_X + icon_x_offset(icon_width, icon_slot_width)) as f32,
            y: baseline + 1.0,
        },
        color_rgba(icon_color, 255),
    );
    draw_text(
        &mut canvas,
        &fonts,
        &text,
        FONT_SIZE,
        Point {
            x: (PADDING_X + icon_slot_width + icon_text_gap) as f32,
            y: baseline,
        },
        style.text,
    );

    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    Some(DragImage { png, width, height })
}

struct CardStyle {
    bg: [u8; 4],
    text: [u8; 4],
}

fn resolve_card_style(card_color: Color, text_color: Color) -> CardStyle {
    const DARK_BG: [u8; 4] = [0, 0, 0, 232];
    const LIGHT_TEXT: [u8; 4] = [231, 237, 245, 255];
    const DARK_TEXT: [u8; 4] = [38, 28, 35, 255];

    let bg_rgb = color_rgb(card_color).unwrap_or([DARK_BG[0], DARK_BG[1], DARK_BG[2]]);
    let bg = [bg_rgb[0], bg_rgb[1], bg_rgb[2], 232];

    CardStyle {
        bg,
        text: readable_text_for(bg, color_rgb(text_color), LIGHT_TEXT, DARK_TEXT),
    }
}

fn readable_text_for(
    bg: [u8; 4],
    preferred: Option<[u8; 3]>,
    light: [u8; 4],
    dark: [u8; 4],
) -> [u8; 4] {
    if let Some([r, g, b]) =
        preferred.filter(|rgb| contrast_ratio(*rgb, [bg[0], bg[1], bg[2]]) >= 4.5)
    {
        return [r, g, b, 255];
    }

    if contrast_ratio([light[0], light[1], light[2]], [bg[0], bg[1], bg[2]])
        >= contrast_ratio([dark[0], dark[1], dark[2]], [bg[0], bg[1], bg[2]])
    {
        light
    } else {
        dark
    }
}

fn contrast_ratio(fg: [u8; 3], bg: [u8; 3]) -> f32 {
    let fg = relative_luminance(fg);
    let bg = relative_luminance(bg);
    let (light, dark) = if fg >= bg { (fg, bg) } else { (bg, fg) };
    (light + 0.05) / (dark + 0.05)
}

fn relative_luminance([r, g, b]: [u8; 3]) -> f32 {
    fn channel(value: u8) -> f32 {
        let value = value as f32 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn font_cache() -> &'static OnceLock<Option<FontSet>> {
    static FONTS: OnceLock<Option<FontSet>> = OnceLock::new();
    &FONTS
}

fn loaded_fonts() -> Option<&'static FontSet> {
    font_cache().get_or_init(load_font_from_disk).as_ref()
}

fn icon_text_gap(icon_width: u32) -> u32 {
    if icon_width >= 30 {
        WIDE_ICON_TEXT_GAP
    } else {
        ICON_TEXT_GAP
    }
}

fn icon_slot_width(icon_width: u32) -> u32 {
    icon_width.max(ICON_SLOT_WIDTH)
}

fn icon_x_offset(icon_width: u32, icon_slot_width: u32) -> u32 {
    icon_slot_width.saturating_sub(icon_width) / 2
}

struct FontSet {
    primary: Font,
    fallbacks: Vec<Font>,
}

struct RenderFontSet<'a> {
    base: &'a FontSet,
    glyph_fallbacks: Vec<Arc<Font>>,
}

fn load_font_from_disk() -> Option<FontSet> {
    let primary_path = kitty_font_family()
        .as_deref()
        .and_then(platform_fonts::resolve_family)
        .or_else(platform_fonts::resolve_monospace)
        .or_else(known_font_fallback)?;
    let primary = load_font(&primary_path)?;

    let fallbacks = ["Symbols Nerd Font Mono", "Symbols Nerd Font"]
        .into_iter()
        .filter_map(platform_fonts::resolve_matching_family)
        .filter(|path| path != &primary_path)
        .filter_map(|path| load_font(&path))
        .collect();

    Some(FontSet { primary, fallbacks })
}

fn load_glyph_fallbacks_for_text(fonts: &FontSet, text: &str) -> Vec<Arc<Font>> {
    let mut seen_chars = HashSet::new();
    let mut fallbacks = Vec::new();

    for ch in text.chars() {
        if !seen_chars.insert(ch)
            || font_set_has_char(fonts, ch)
            || arc_fonts_have_char(&fallbacks, ch)
        {
            continue;
        }

        if let Some(font) = cached_glyph_fallback(ch) {
            fallbacks.push(font);
        }
    }

    fallbacks
}

fn glyph_fallback_cache() -> &'static Mutex<HashMap<char, Option<Arc<Font>>>> {
    static CACHE: OnceLock<Mutex<HashMap<char, Option<Arc<Font>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_glyph_fallback(ch: char) -> Option<Arc<Font>> {
    {
        let mut cache = glyph_fallback_cache()
            .lock()
            .expect("glyph fallback cache lock");
        if let Some(cached) = cache.get(&ch).cloned() {
            return cached;
        }
        if let Some(font) = cache
            .values()
            .filter_map(Option::as_ref)
            .find(|font| font.lookup_glyph_index(ch) != 0)
            .cloned()
        {
            cache.insert(ch, Some(Arc::clone(&font)));
            return Some(font);
        }
    }

    let resolved = load_glyph_fallback(ch);
    glyph_fallback_cache()
        .lock()
        .expect("glyph fallback cache lock")
        .insert(ch, resolved.clone());
    resolved
}

fn load_glyph_fallback(ch: char) -> Option<Arc<Font>> {
    let path = platform_fonts::resolve_char(ch)?;
    let font = load_font(&path)?;
    (font.lookup_glyph_index(ch) != 0).then(|| Arc::new(font))
}

fn load_font(path: &Path) -> Option<Font> {
    let bytes = fs::read(path).ok()?;
    Font::from_bytes(bytes, FontSettings::default()).ok()
}

fn kitty_font_family() -> Option<String> {
    let mut visited = Vec::new();
    for path in kitty_config_candidates() {
        if let Some(font) = kitty_font_family_from_config(&path, &mut visited, 0) {
            return Some(font);
        }
    }
    None
}

fn kitty_config_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(config_home).join("kitty/kitty.conf"));
    }
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".config/kitty/kitty.conf"));
    }
    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("kitty/kitty.conf"));
    }
    paths
}

fn kitty_font_family_from_config(
    path: &Path,
    visited: &mut Vec<PathBuf>,
    depth: usize,
) -> Option<String> {
    if depth > 8 || visited.iter().any(|visited| visited == path) {
        return None;
    }
    visited.push(path.to_path_buf());

    let content = fs::read_to_string(path).ok()?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    for line in content.lines() {
        let line = line.split_once('#').map_or(line, |(line, _)| line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(value) = kitty_setting_value(line, "font_family") {
            if !value.eq_ignore_ascii_case("auto") {
                return Some(value.to_string());
            }
        } else if let Some(include) = kitty_setting_value(line, "include") {
            let include = PathBuf::from(include);
            let include = if include.is_absolute() {
                include
            } else {
                base_dir.join(include)
            };
            if let Some(font) = kitty_font_family_from_config(&include, visited, depth + 1) {
                return Some(font);
            }
        }
    }
    None
}

fn kitty_setting_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let value = line.strip_prefix(key)?;
    if !value.starts_with(char::is_whitespace) {
        return None;
    }
    clean_kitty_value(value)
}

fn clean_kitty_value(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.trim_matches(['\'', '"']))
}

#[cfg(not(target_os = "macos"))]
mod platform_fonts {
    use std::{
        path::PathBuf,
        process::{Command, Stdio},
    };

    pub(super) fn resolve_family(family: &str) -> Option<PathBuf> {
        resolve_family_output(family).map(|(_, path)| path)
    }

    pub(super) fn resolve_monospace() -> Option<PathBuf> {
        resolve_family("monospace")
    }

    pub(super) fn resolve_matching_family(family: &str) -> Option<PathBuf> {
        let (matched_family, path) = resolve_family_output(family)?;
        matched_family
            .to_ascii_lowercase()
            .contains(&family.to_ascii_lowercase())
            .then_some(path)
    }

    pub(super) fn resolve_char(ch: char) -> Option<PathBuf> {
        let query = fontconfig_charset_query(ch);
        let output = Command::new("fc-match")
            .arg("-f")
            .arg("%{file}\n")
            .arg(query)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let output = String::from_utf8_lossy(&output.stdout);
        let path = PathBuf::from(output.lines().next()?.trim());
        path.exists().then_some(path)
    }

    pub(super) fn fontconfig_charset_query(ch: char) -> String {
        format!(":charset={:x}", ch as u32)
    }

    fn resolve_family_output(family: &str) -> Option<(String, PathBuf)> {
        let output = Command::new("fc-match")
            .args(["-f", "%{family}\n%{file}\n", family])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let output = String::from_utf8_lossy(&output.stdout);
        let mut lines = output.lines();
        let matched_family = lines.next()?.trim().to_string();
        let path = PathBuf::from(lines.next()?.trim());
        path.exists().then_some((matched_family, path))
    }
}

#[cfg(target_os = "macos")]
mod platform_fonts {
    use std::{ffi::c_void, path::PathBuf, ptr};

    type Boolean = u8;
    type CFIndex = isize;
    type CFStringEncoding = u32;
    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFURLRef = *const c_void;
    type CTFontRef = *const c_void;
    type CTFontDescriptorRef = *const c_void;
    type CGFloat = f64;

    const K_CF_STRING_ENCODING_UTF8: CFStringEncoding = 0x0800_0100;
    const K_CFURL_POSIX_PATH_STYLE: CFIndex = 0;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CFRange {
        location: CFIndex,
        length: CFIndex,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: CFTypeRef);
        fn CFStringCreateWithBytes(
            alloc: *const c_void,
            bytes: *const u8,
            num_bytes: CFIndex,
            encoding: CFStringEncoding,
            is_external_representation: Boolean,
        ) -> CFStringRef;
        fn CFStringGetCString(
            string: CFStringRef,
            buffer: *mut i8,
            buffer_size: CFIndex,
            encoding: CFStringEncoding,
        ) -> Boolean;
        fn CFStringGetLength(string: CFStringRef) -> CFIndex;
        fn CFStringGetMaximumSizeForEncoding(
            length: CFIndex,
            encoding: CFStringEncoding,
        ) -> CFIndex;
        fn CFURLCopyFileSystemPath(url: CFURLRef, path_style: CFIndex) -> CFStringRef;
    }

    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        static kCTFontURLAttribute: CFStringRef;

        fn CTFontCreateWithName(
            name: CFStringRef,
            size: CGFloat,
            matrix: *const c_void,
        ) -> CTFontRef;
        fn CTFontCreateForString(
            current_font: CTFontRef,
            string: CFStringRef,
            range: CFRange,
        ) -> CTFontRef;
        fn CTFontCopyFamilyName(font: CTFontRef) -> CFStringRef;
        fn CTFontCopyFontDescriptor(font: CTFontRef) -> CTFontDescriptorRef;
        fn CTFontDescriptorCopyAttribute(
            descriptor: CTFontDescriptorRef,
            attribute: CFStringRef,
        ) -> CFTypeRef;
    }

    pub(super) fn resolve_family(family: &str) -> Option<PathBuf> {
        font_path_for_family(family).map(|resolved| resolved.path)
    }

    pub(super) fn resolve_monospace() -> Option<PathBuf> {
        ["Menlo", "Monaco"]
            .into_iter()
            .find_map(|family| font_path_for_family(family).map(|resolved| resolved.path))
    }

    pub(super) fn resolve_matching_family(family: &str) -> Option<PathBuf> {
        let resolved = font_path_for_family(family)?;
        resolved
            .family
            .to_ascii_lowercase()
            .contains(&family.to_ascii_lowercase())
            .then_some(resolved.path)
    }

    pub(super) fn resolve_char(ch: char) -> Option<PathBuf> {
        let base_font =
            create_font_for_family("Menlo").or_else(|| create_font_for_family("Monaco"))?;
        let Some(text) = cf_string_from_str(&ch.to_string()) else {
            release_if_present(base_font);
            return None;
        };
        let length = unsafe { CFStringGetLength(text) };
        let fallback = unsafe {
            CTFontCreateForString(
                base_font,
                text,
                CFRange {
                    location: 0,
                    length,
                },
            )
        };

        let path = font_path_for_font(fallback).map(|resolved| resolved.path);
        release_if_present(fallback);
        release_if_present(text);
        release_if_present(base_font);
        path
    }

    struct ResolvedFont {
        path: PathBuf,
        family: String,
    }

    fn font_path_for_family(family: &str) -> Option<ResolvedFont> {
        let font = create_font_for_family(family)?;
        let resolved = font_path_for_font(font);
        release_if_present(font);
        resolved
    }

    fn create_font_for_family(family: &str) -> Option<CTFontRef> {
        let name = cf_string_from_str(family)?;
        let font = unsafe { CTFontCreateWithName(name, 12.0, ptr::null()) };
        release_if_present(name);
        (!font.is_null()).then_some(font)
    }

    fn font_path_for_font(font: CTFontRef) -> Option<ResolvedFont> {
        if font.is_null() {
            return None;
        }

        let descriptor = unsafe { CTFontCopyFontDescriptor(font) };
        if descriptor.is_null() {
            return None;
        }

        let url = unsafe { CTFontDescriptorCopyAttribute(descriptor, kCTFontURLAttribute) };
        let family = copy_font_family(font);
        let path = font_url_to_path(url as CFURLRef);

        release_if_present(url);
        release_if_present(descriptor);

        Some(ResolvedFont {
            path: path?,
            family: family?,
        })
    }

    fn font_url_to_path(url: CFURLRef) -> Option<PathBuf> {
        if url.is_null() {
            return None;
        }

        let path_string = unsafe { CFURLCopyFileSystemPath(url, K_CFURL_POSIX_PATH_STYLE) };
        let path = cf_string_to_string(path_string).map(PathBuf::from);
        release_if_present(path_string);
        path.filter(|path| path.exists())
    }

    fn copy_font_family(font: CTFontRef) -> Option<String> {
        let family = unsafe { CTFontCopyFamilyName(font) };
        let value = cf_string_to_string(family);
        release_if_present(family);
        value
    }

    fn cf_string_from_str(value: &str) -> Option<CFStringRef> {
        let string = unsafe {
            CFStringCreateWithBytes(
                ptr::null(),
                value.as_ptr(),
                value.len() as CFIndex,
                K_CF_STRING_ENCODING_UTF8,
                0,
            )
        };
        (!string.is_null()).then_some(string)
    }

    fn cf_string_to_string(value: CFStringRef) -> Option<String> {
        if value.is_null() {
            return None;
        }

        let length = unsafe { CFStringGetLength(value) };
        let capacity =
            unsafe { CFStringGetMaximumSizeForEncoding(length, K_CF_STRING_ENCODING_UTF8) } + 1;
        let mut bytes = vec![0i8; capacity as usize];
        let ok = unsafe {
            CFStringGetCString(
                value,
                bytes.as_mut_ptr(),
                capacity,
                K_CF_STRING_ENCODING_UTF8,
            )
        };
        if ok == 0 {
            return None;
        }

        let nul = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        let bytes = bytes[..nul]
            .iter()
            .map(|byte| *byte as u8)
            .collect::<Vec<_>>();
        String::from_utf8(bytes).ok()
    }

    fn release_if_present(value: CFTypeRef) {
        if !value.is_null() {
            unsafe { CFRelease(value) };
        }
    }
}

fn known_font_fallback() -> Option<PathBuf> {
    [
        "/usr/share/fonts/TTF/MesloLGMNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/MesloLGLNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/FantasqueSansMNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/vscode.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.exists())
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    truncated.push_str("...");
    truncated
}

fn measure_text(fonts: &RenderFontSet<'_>, text: &str, size: f32) -> f32 {
    text.chars()
        .map(|ch| {
            font_for_char_or_primary(fonts, ch)
                .metrics(ch, size)
                .advance_width
                .max(0.0)
        })
        .sum()
}

fn can_render_text(fonts: &RenderFontSet<'_>, text: &str) -> bool {
    text.chars().all(|ch| font_for_char(fonts, ch).is_some())
}

fn font_set_has_char(fonts: &FontSet, ch: char) -> bool {
    fonts.primary.lookup_glyph_index(ch) != 0 || fonts_have_char(&fonts.fallbacks, ch)
}

fn fonts_have_char(fonts: &[Font], ch: char) -> bool {
    fonts.iter().any(|font| font.lookup_glyph_index(ch) != 0)
}

fn arc_fonts_have_char(fonts: &[Arc<Font>], ch: char) -> bool {
    fonts.iter().any(|font| font.lookup_glyph_index(ch) != 0)
}

fn font_for_char_or_primary<'a>(fonts: &'a RenderFontSet<'_>, ch: char) -> &'a Font {
    font_for_char(fonts, ch).unwrap_or(&fonts.base.primary)
}

fn font_for_char<'a>(fonts: &'a RenderFontSet<'_>, ch: char) -> Option<&'a Font> {
    if fonts.base.primary.lookup_glyph_index(ch) != 0 {
        return Some(&fonts.base.primary);
    }
    fonts
        .base
        .fallbacks
        .iter()
        .find(|font| font.lookup_glyph_index(ch) != 0)
        .or_else(|| {
            fonts
                .glyph_fallbacks
                .iter()
                .find(|font| font.lookup_glyph_index(ch) != 0)
                .map(Arc::as_ref)
        })
}

fn draw_text(
    canvas: &mut Canvas<'_>,
    fonts: &RenderFontSet<'_>,
    text: &str,
    size: f32,
    origin: Point,
    rgba: [u8; 4],
) {
    let mut cursor = origin.x;
    for ch in text.chars() {
        let font = font_for_char_or_primary(fonts, ch);
        let (metrics, bitmap) = font.rasterize(ch, size);
        let glyph_x = cursor + metrics.xmin as f32;
        let glyph_y = origin.y - metrics.ymin as f32 - metrics.height as f32;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + col];
                if alpha == 0 {
                    continue;
                }
                blend_pixel(
                    canvas,
                    glyph_x + col as f32,
                    glyph_y + row as f32,
                    [
                        rgba[0],
                        rgba[1],
                        rgba[2],
                        ((alpha as u16 * rgba[3] as u16) / 255) as u8,
                    ],
                );
            }
        }
        cursor += metrics.advance_width;
    }
}

fn draw_rounded_rect(canvas: &mut Canvas<'_>, rect: Rect, radius: f32, rgba: [u8; 4]) {
    let left = rect.x.floor().max(0.0) as u32;
    let top = rect.y.floor().max(0.0) as u32;
    let right = (rect.x + rect.width).ceil().min(canvas.width as f32) as u32;
    let bottom = (rect.y + rect.height).ceil().min(canvas.height as f32) as u32;
    for py in top..bottom {
        for px in left..right {
            let alpha = rounded_rect_coverage(px as f32 + 0.5, py as f32 + 0.5, rect, radius);
            if alpha > 0.0 {
                let mut c = rgba;
                c[3] = (rgba[3] as f32 * alpha).round() as u8;
                blend_pixel(canvas, px as f32, py as f32, c);
            }
        }
    }
}

fn rounded_rect_coverage(px: f32, py: f32, rect: Rect, radius: f32) -> f32 {
    let cx = px.clamp(rect.x + radius, rect.x + rect.width - radius);
    let cy = py.clamp(rect.y + radius, rect.y + rect.height - radius);
    let dx = px - cx;
    let dy = py - cy;
    let dist = (dx * dx + dy * dy).sqrt();
    (radius + 0.5 - dist).clamp(0.0, 1.0)
}

fn blend_pixel(canvas: &mut Canvas<'_>, x: f32, y: f32, src: [u8; 4]) {
    let x = x.round() as i32;
    let y = y.round() as i32;
    if x < 0 || y < 0 || x >= canvas.width as i32 || y >= canvas.height as i32 || src[3] == 0 {
        return;
    }
    let idx = (y as u32 * canvas.width + x as u32) as usize * 4;
    let sa = src[3] as f32 / 255.0;
    let da = canvas.pixels[idx + 3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= f32::EPSILON {
        return;
    }
    for (channel, src_channel) in src.iter().copied().enumerate().take(3) {
        let sc = src_channel as f32 / 255.0;
        let dc = canvas.pixels[idx + channel] as f32 / 255.0;
        canvas.pixels[idx + channel] =
            (((sc * sa + dc * da * (1.0 - sa)) / out_a) * 255.0).round() as u8;
    }
    canvas.pixels[idx + 3] = (out_a * 255.0).round() as u8;
}

fn color_rgba(color: Color, alpha: u8) -> [u8; 4] {
    let [r, g, b] = color_rgb(color).unwrap_or([231, 237, 245]);
    [r, g, b, alpha]
}

fn color_rgb(color: Color) -> Option<[u8; 3]> {
    Some(match color {
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
        Color::White => [231, 237, 245],
        Color::Indexed(_) | Color::Reset => return None,
    })
}

#[cfg(test)]
#[path = "tests/drag_card.rs"]
mod tests;
