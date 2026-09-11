use super::image_inspection::{
    StaticImageFormat, read_exif_orientation, read_raster_dimensions, read_svg_dimensions,
    static_image_format_for_cached_path,
};
use super::image_rendering::{
    apply_raster_orientation, render_raster_to_jpeg_with_ffmpeg, render_raster_to_png_with_ffmpeg,
    render_svg_to_png_with_magick, render_svg_to_png_with_resvg, should_render_raster_with_ffmpeg,
    shrink_image_to_fit,
};
use crate::terminal_runtime::terminal_images::{
    RenderedImageDimensions, TerminalWindowSize, area_pixel_size, encode_iterm_inline_payload,
    encode_sixel_dcs, fit_image_area,
};
use image::{ImageFormat, ImageReader};
use ratatui::layout::Rect;
use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

const STATIC_IMAGE_INLINE_FALLBACK_PREPARE_MAX_BYTES: u64 = 512 * 1024;
const STATIC_IMAGE_INLINE_EXTERNAL_PREPARE_MAX_BYTES: u64 = 16 * 1024 * 1024;
const STATIC_IMAGE_ITERM_SOURCE_PASSTHROUGH_MAX_BYTES: u64 = 700 * 1024;
const STATIC_IMAGE_RENDER_CACHE_VERSION: usize = 5;

/// Parameters needed to pre-encode a Sixel image alongside its rendered file.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SixelPrepareConfig {
    pub(crate) area_width: u16,
    pub(crate) area_height: u16,
    pub(crate) window_size: TerminalWindowSize,
}

#[derive(Clone, Debug)]
pub(crate) struct ImagePrepareRequest {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) target_width_px: u32,
    pub(crate) target_height_px: u32,
    pub(crate) ffmpeg_available: bool,
    pub(crate) resvg_available: bool,
    pub(crate) magick_available: bool,
    pub(crate) force_render_to_cache: bool,
    pub(crate) prepare_inline_payload: bool,
    pub(crate) sixel_prepare: Option<SixelPrepareConfig>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct StaticImageKey {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) target_width_px: u32,
    pub(crate) target_height_px: u32,
    pub(crate) force_render_to_cache: bool,
    pub(crate) prepare_inline_payload: bool,
}

impl StaticImageKey {
    pub(crate) fn from_parts(
        path: PathBuf,
        size: u64,
        modified: Option<SystemTime>,
        target_width_px: u32,
        target_height_px: u32,
        force_render_to_cache: bool,
        prepare_inline_payload: bool,
    ) -> Self {
        Self {
            path,
            size,
            modified,
            target_width_px,
            target_height_px,
            force_render_to_cache,
            prepare_inline_payload,
        }
    }
}

/// Cache key for a rendered Sixel stream at a specific terminal image size.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SixelDcsKey {
    pub(crate) path: PathBuf,
    pub(crate) area_width: u16,
    pub(crate) area_height: u16,
    pub(crate) cells_width: u16,
    pub(crate) cells_height: u16,
    pub(crate) pixels_width: u32,
    pub(crate) pixels_height: u32,
}

impl SixelDcsKey {
    pub(crate) fn new(path: &Path, placement: Rect, window: TerminalWindowSize) -> Self {
        Self {
            path: path.to_path_buf(),
            area_width: placement.width,
            area_height: placement.height,
            cells_width: window.cells_width,
            cells_height: window.cells_height,
            pixels_width: window.pixels_width,
            pixels_height: window.pixels_height,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PreparedStaticImageAsset {
    pub(crate) display_path: PathBuf,
    pub(crate) dimensions: RenderedImageDimensions,
    pub(crate) inline_payload: Option<Arc<str>>,
    pub(crate) sixel_dcs: Option<Arc<[u8]>>,
    pub(crate) sixel_dcs_key: Option<SixelDcsKey>,
}

pub(crate) fn prepare_static_image_asset<F>(
    request: &ImagePrepareRequest,
    canceled: F,
) -> Option<PreparedStaticImageAsset>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }
    let format =
        static_image_format_for_cached_path(&request.path, request.size, request.modified)?;
    let source_dimensions = if format == StaticImageFormat::Svg {
        read_svg_dimensions(&request.path)?
    } else {
        read_raster_dimensions(&request.path)?
    };
    if canceled() {
        return None;
    }
    let target_width_px = request.target_width_px.max(1);
    let target_height_px = request.target_height_px.max(1);
    let key = StaticImageKey::from_parts(
        request.path.clone(),
        request.size,
        request.modified,
        target_width_px,
        target_height_px,
        request.force_render_to_cache,
        request.prepare_inline_payload,
    );
    let inline_payload = |path: &Path| -> Option<Option<Arc<str>>> {
        if !request.prepare_inline_payload {
            return Some(None);
        }
        Some(Some(encode_iterm_inline_payload(path)?))
    };
    // When the job was submitted for a Sixel session, pre-encode the DCS stream
    // so it can be cached and reused at render time without re-decoding.
    let prepare_sixel_dcs = |display_path: &Path| -> (Option<Arc<[u8]>>, Option<SixelDcsKey>) {
        let config = match request.sixel_prepare.as_ref() {
            Some(c) => c,
            None => return (None, None),
        };
        let aspect = source_dimensions.width_px as f32 / source_dimensions.height_px.max(1) as f32;
        let area = Rect {
            x: 0,
            y: 0,
            width: config.area_width,
            height: config.area_height,
        };
        let fitted = fit_image_area(area, config.window_size, aspect);
        let (target_w, target_h) = area_pixel_size(fitted, config.window_size);
        let Ok(dcs) = encode_sixel_dcs(display_path, target_w, target_h) else {
            return (None, None);
        };
        let key = SixelDcsKey::new(display_path, fitted, config.window_size);
        (Some(dcs), Some(key))
    };

    if static_image_supports_iterm_source_passthrough_for_prepare(request, format) {
        let payload = inline_payload(&request.path)?;
        let (sixel_dcs, sixel_dcs_key) = prepare_sixel_dcs(&request.path);
        return Some(PreparedStaticImageAsset {
            dimensions: source_dimensions,
            display_path: request.path.clone(),
            inline_payload: payload,
            sixel_dcs,
            sixel_dcs_key,
        });
    }

    if format == StaticImageFormat::Svg {
        let cache_path = static_image_render_cache_path(&key, StaticImageRenderCacheFormat::Png)?;
        if cache_path.exists() {
            let payload = inline_payload(&cache_path)?;
            let (sixel_dcs, sixel_dcs_key) = prepare_sixel_dcs(&cache_path);
            return Some(PreparedStaticImageAsset {
                dimensions: source_dimensions,
                display_path: cache_path,
                inline_payload: payload,
                sixel_dcs,
                sixel_dcs_key,
            });
        }
        let temp_path = static_image_render_temp_path(&cache_path)?;
        let rendered = (request.resvg_available
            && render_svg_to_png_with_resvg(
                &request.path,
                &temp_path,
                source_dimensions,
                target_width_px,
                target_height_px,
                &canceled,
            ))
            || (request.magick_available
                && render_svg_to_png_with_magick(
                    &request.path,
                    &temp_path,
                    target_width_px,
                    target_height_px,
                    &canceled,
                ));
        if rendered {
            finalize_static_image_render(&temp_path, &cache_path)?;
            let payload = inline_payload(&cache_path)?;
            let (sixel_dcs, sixel_dcs_key) = prepare_sixel_dcs(&cache_path);
            return Some(PreparedStaticImageAsset {
                dimensions: source_dimensions,
                display_path: cache_path,
                inline_payload: payload,
                sixel_dcs,
                sixel_dcs_key,
            });
        }
        let _ = fs::remove_file(temp_path);
        return None;
    }

    let render_format = static_image_render_cache_format(request, format);
    let cache_path = static_image_render_cache_path(&key, render_format)?;
    if cache_path.exists() {
        let payload = inline_payload(&cache_path)?;
        let (sixel_dcs, sixel_dcs_key) = prepare_sixel_dcs(&cache_path);
        return Some(PreparedStaticImageAsset {
            dimensions: source_dimensions,
            display_path: cache_path,
            inline_payload: payload,
            sixel_dcs,
            sixel_dcs_key,
        });
    }
    if canceled() {
        return None;
    }
    let temp_path = static_image_render_temp_path(&cache_path)?;

    let rendered_with_ffmpeg = request.ffmpeg_available
        && should_render_raster_with_ffmpeg(format)
        && match render_format {
            StaticImageRenderCacheFormat::Jpeg => render_raster_to_jpeg_with_ffmpeg(
                &request.path,
                &temp_path,
                target_width_px,
                target_height_px,
                &canceled,
            ),
            StaticImageRenderCacheFormat::Png => render_raster_to_png_with_ffmpeg(
                &request.path,
                &temp_path,
                target_width_px,
                target_height_px,
                static_image_use_fast_png_render(request),
                &canceled,
            ),
        };
    if rendered_with_ffmpeg {
        finalize_static_image_render(&temp_path, &cache_path)?;
        let payload = inline_payload(&cache_path)?;
        let (sixel_dcs, sixel_dcs_key) = prepare_sixel_dcs(&cache_path);
        return Some(PreparedStaticImageAsset {
            dimensions: source_dimensions,
            display_path: cache_path,
            inline_payload: payload,
            sixel_dcs,
            sixel_dcs_key,
        });
    }

    let image = ImageReader::open(&request.path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    if canceled() {
        return None;
    }
    let image = apply_raster_orientation(image, read_exif_orientation(&request.path).unwrap_or(1));
    if canceled() {
        return None;
    }
    let image = shrink_image_to_fit(image, target_width_px, target_height_px);
    if canceled() {
        return None;
    }
    save_dynamic_image(&image, &temp_path, render_format)?;
    finalize_static_image_render(&temp_path, &cache_path)?;
    let payload = inline_payload(&cache_path)?;
    let (sixel_dcs, sixel_dcs_key) = prepare_sixel_dcs(&cache_path);

    Some(PreparedStaticImageAsset {
        dimensions: source_dimensions,
        display_path: cache_path,
        inline_payload: payload,
        sixel_dcs,
        sixel_dcs_key,
    })
}

#[derive(Clone, Copy, Hash)]
enum StaticImageRenderCacheFormat {
    Png,
    Jpeg,
}

impl StaticImageRenderCacheFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }
}

fn static_image_render_cache_format(
    request: &ImagePrepareRequest,
    format: StaticImageFormat,
) -> StaticImageRenderCacheFormat {
    if request.prepare_inline_payload && format == StaticImageFormat::Jpeg {
        StaticImageRenderCacheFormat::Jpeg
    } else {
        StaticImageRenderCacheFormat::Png
    }
}

fn static_image_use_fast_png_render(request: &ImagePrepareRequest) -> bool {
    request.force_render_to_cache && !request.prepare_inline_payload
}

fn save_dynamic_image(
    image: &image::DynamicImage,
    path: &Path,
    format: StaticImageRenderCacheFormat,
) -> Option<()> {
    match format {
        StaticImageRenderCacheFormat::Png => image.save_with_format(path, ImageFormat::Png).ok()?,
        StaticImageRenderCacheFormat::Jpeg => image
            .to_rgb8()
            .save_with_format(path, ImageFormat::Jpeg)
            .ok()?,
    }
    Some(())
}

fn static_image_render_cache_path(
    key: &StaticImageKey,
    format: StaticImageRenderCacheFormat,
) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    STATIC_IMAGE_RENDER_CACHE_VERSION.hash(&mut hasher);
    format.hash(&mut hasher);
    key.path.hash(&mut hasher);
    key.size.hash(&mut hasher);
    key.modified.hash(&mut hasher);
    key.target_width_px.hash(&mut hasher);
    key.target_height_px.hash(&mut hasher);
    key.force_render_to_cache.hash(&mut hasher);
    key.prepare_inline_payload.hash(&mut hasher);
    let cache_dir = env::temp_dir().join(format!(
        "elio-image-preview-v{STATIC_IMAGE_RENDER_CACHE_VERSION}"
    ));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!(
        "image-{:016x}.{}",
        hasher.finish(),
        format.extension()
    )))
}

fn static_image_render_temp_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let stem = path.file_stem()?.to_string_lossy();
    let extension = path.extension().and_then(|extension| extension.to_str());
    let file_name = match extension {
        Some(extension) if !extension.is_empty() => {
            format!(".{stem}.tmp-{}-{unique}.{extension}", std::process::id())
        }
        _ => format!(".{stem}.tmp-{}-{unique}", std::process::id()),
    };
    Some(parent.join(file_name))
}

fn finalize_static_image_render(temp_path: &Path, cache_path: &Path) -> Option<()> {
    match fs::rename(temp_path, cache_path) {
        Ok(()) => Some(()),
        Err(_) if cache_path.exists() => {
            let _ = fs::remove_file(temp_path);
            Some(())
        }
        Err(_) => {
            let _ = fs::remove_file(temp_path);
            None
        }
    }
}

pub(crate) fn static_image_can_prepare_inline(
    size: u64,
    format: StaticImageFormat,
    ffmpeg_available: bool,
) -> bool {
    match format {
        StaticImageFormat::Png => true,
        StaticImageFormat::Ico => true,
        StaticImageFormat::Jpeg | StaticImageFormat::Gif | StaticImageFormat::Webp => {
            if ffmpeg_available {
                size <= STATIC_IMAGE_INLINE_EXTERNAL_PREPARE_MAX_BYTES
            } else {
                size <= STATIC_IMAGE_INLINE_FALLBACK_PREPARE_MAX_BYTES
            }
        }
        StaticImageFormat::Svg => false,
    }
}

pub(crate) fn static_image_supports_iterm_source_passthrough(
    path: &Path,
    size: u64,
    modified: Option<SystemTime>,
    force_render_to_cache: bool,
) -> bool {
    if force_render_to_cache || size > STATIC_IMAGE_ITERM_SOURCE_PASSTHROUGH_MAX_BYTES {
        return false;
    }
    static_image_format_for_cached_path(path, size, modified)
        .is_some_and(|format| static_image_supports_iterm_source_format(path, format))
}

fn static_image_supports_iterm_source_passthrough_for_prepare(
    request: &ImagePrepareRequest,
    format: StaticImageFormat,
) -> bool {
    request.prepare_inline_payload
        && !request.force_render_to_cache
        && request.size <= STATIC_IMAGE_ITERM_SOURCE_PASSTHROUGH_MAX_BYTES
        && static_image_supports_iterm_source_format(&request.path, format)
}

fn static_image_supports_iterm_source_format(path: &Path, format: StaticImageFormat) -> bool {
    match format {
        StaticImageFormat::Png => true,
        StaticImageFormat::Ico => false,
        StaticImageFormat::Jpeg => read_exif_orientation(path).unwrap_or(1) == 1,
        StaticImageFormat::Gif | StaticImageFormat::Webp | StaticImageFormat::Svg => false,
    }
}

#[cfg(test)]
#[path = "tests/image_preparation_formats.rs"]
mod format_tests;
#[cfg(test)]
#[path = "tests/inline_payloads.rs"]
mod inline_payload_tests;
#[cfg(test)]
#[path = "tests/image_normalization.rs"]
mod normalization_tests;
#[cfg(test)]
#[path = "tests/image_preparation.rs"]
mod tests;
