use super::format::{
    StaticImageFormat, read_exif_orientation, read_raster_dimensions, read_svg_dimensions,
    static_image_format_for_overlay_request, static_image_format_for_prepare_request,
};
use super::render::{
    apply_raster_orientation, render_raster_to_png_with_ffmpeg, render_svg_to_png_with_magick,
    render_svg_to_png_with_resvg, should_render_raster_with_ffmpeg, shrink_image_to_fit,
};
use super::{
    PreparedStaticImageAsset, STATIC_IMAGE_INLINE_EXTERNAL_PREPARE_MAX_BYTES,
    STATIC_IMAGE_INLINE_FALLBACK_PREPARE_MAX_BYTES, STATIC_IMAGE_RENDER_CACHE_VERSION, SixelDcsKey,
    StaticImageKey, StaticImageOverlayRequest,
};
use crate::app::jobs;
use crate::app::overlays::inline_image::{
    area_pixel_size, encode_iterm_inline_payload, encode_sixel_dcs, fit_image_area,
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

pub(crate) fn prepare_static_image_asset<F>(
    request: &jobs::ImagePrepareRequest,
    canceled: F,
) -> Option<PreparedStaticImageAsset>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }
    let format = static_image_format_for_prepare_request(request)?;
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
        let cache_path = static_image_render_cache_path(&key)?;
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

    let cache_path = static_image_render_cache_path(&key)?;
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

    if request.ffmpeg_available
        && should_render_raster_with_ffmpeg(format)
        && render_raster_to_png_with_ffmpeg(
            &request.path,
            &temp_path,
            target_width_px,
            target_height_px,
            request.force_render_to_cache,
            &canceled,
        )
    {
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
    image.save_with_format(&temp_path, ImageFormat::Png).ok()?;
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

fn static_image_render_cache_path(key: &StaticImageKey) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    STATIC_IMAGE_RENDER_CACHE_VERSION.hash(&mut hasher);
    key.path.hash(&mut hasher);
    key.size.hash(&mut hasher);
    key.modified.hash(&mut hasher);
    key.target_width_px.hash(&mut hasher);
    key.target_height_px.hash(&mut hasher);
    key.force_render_to_cache.hash(&mut hasher);
    let cache_dir = env::temp_dir().join(format!(
        "elio-image-preview-v{STATIC_IMAGE_RENDER_CACHE_VERSION}"
    ));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!("image-{:016x}.png", hasher.finish())))
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

pub(super) fn static_image_can_prepare_inline(
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

pub(super) fn static_image_supports_iterm_source_passthrough(
    request: &StaticImageOverlayRequest,
) -> bool {
    static_image_format_for_overlay_request(request)
        .is_some_and(|format| static_image_supports_iterm_source_format(&request.path, format))
        && !request.force_render_to_cache
}

fn static_image_supports_iterm_source_passthrough_for_prepare(
    request: &jobs::ImagePrepareRequest,
    format: StaticImageFormat,
) -> bool {
    request.prepare_inline_payload
        && !request.force_render_to_cache
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
