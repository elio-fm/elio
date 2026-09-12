use super::*;
use crate::terminal_images::RenderedImageDimensions;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-image-preview-{label}-{unique}"))
}

fn write_test_raster_image(path: &Path, format: ImageFormat, width_px: u32, height_px: u32) {
    let mut image = RgbaImage::new(width_px, height_px);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([32, 128, 224, 255]);
    }
    DynamicImage::ImageRgba8(image)
        .save_with_format(path, format)
        .expect("failed to write raster test image");
}

fn write_test_oriented_jpeg(path: &Path, width_px: u32, height_px: u32, orientation: u16) {
    write_test_raster_image(path, ImageFormat::Jpeg, width_px, height_px);
    let jpeg = fs::read(path).expect("failed to read jpeg placeholder");
    let mut exif = Vec::with_capacity(36);
    exif.extend_from_slice(&[0xff, 0xe1, 0x00, 0x22]);
    exif.extend_from_slice(b"Exif\0\0");
    exif.extend_from_slice(b"II");
    exif.extend_from_slice(&42_u16.to_le_bytes());
    exif.extend_from_slice(&8_u32.to_le_bytes());
    exif.extend_from_slice(&1_u16.to_le_bytes());
    exif.extend_from_slice(&0x0112_u16.to_le_bytes());
    exif.extend_from_slice(&3_u16.to_le_bytes());
    exif.extend_from_slice(&1_u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&0_u16.to_le_bytes());
    exif.extend_from_slice(&0_u32.to_le_bytes());
    let mut oriented = Vec::with_capacity(jpeg.len() + exif.len());
    oriented.extend_from_slice(&jpeg[..2]);
    oriented.extend_from_slice(&exif);
    oriented.extend_from_slice(&jpeg[2..]);
    fs::write(path, oriented).expect("failed to write oriented jpeg");
}

#[test]
fn oriented_jpeg_fallback_preview_uses_exif_corrected_dimensions() {
    let root = temp_root("image-oriented-jpeg-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("portrait.jpg");
    write_test_oriented_jpeg(&path, 60, 30, 6);
    let metadata = fs::metadata(&path).expect("jpeg metadata should exist");

    let prepared = prepare_static_image_asset(
        &ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 60,
            target_height_px: 60,
            ffmpeg_available: false,
            resvg_available: false,
            magick_available: true,
            force_render_to_cache: false,
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("oriented jpeg should prepare successfully");

    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 30,
            height_px: 60,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("prepared image should open")
            .with_guessed_format()
            .expect("prepared image format should be detected")
            .into_dimensions()
            .expect("prepared image dimensions should be readable"),
        (30, 60)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn oriented_jpeg_ffmpeg_preview_uses_exif_corrected_dimensions() {
    if !crate::terminal_images::command_exists("ffmpeg") {
        return;
    }

    let root = temp_root("image-oriented-jpeg-ffmpeg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("portrait.jpg");
    write_test_oriented_jpeg(&path, 60, 30, 6);
    let metadata = fs::metadata(&path).expect("jpeg metadata should exist");

    let prepared = prepare_static_image_asset(
        &ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 60,
            target_height_px: 60,
            ffmpeg_available: true,
            resvg_available: false,
            magick_available: true,
            force_render_to_cache: false,
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("oriented jpeg should prepare successfully");

    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 30,
            height_px: 60,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("prepared image should open")
            .with_guessed_format()
            .expect("prepared image format should be detected")
            .into_dimensions()
            .expect("prepared image dimensions should be readable"),
        (30, 60)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn oversized_png_static_images_are_normalized_to_cached_overlays() {
    let root = temp_root("large-png-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("large.png");
    write_test_raster_image(&path, ImageFormat::Png, 3200, 1800);
    let metadata = fs::metadata(&path).expect("png metadata should exist");

    let prepared = prepare_static_image_asset(
        &ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            resvg_available: false,
            magick_available: true,
            force_render_to_cache: false,
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("large png should prepare successfully");

    assert_ne!(prepared.display_path, path);
    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 3200,
            height_px: 1800,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("rendered image should open")
            .with_guessed_format()
            .expect("rendered image format should be detected")
            .into_dimensions()
            .expect("rendered image should be readable"),
        (768, 432)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn forced_png_preview_renders_a_cached_overlay_asset() {
    let root = temp_root("forced-png-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("page.png");
    write_test_raster_image(&path, ImageFormat::Png, 3200, 1800);
    let metadata = fs::metadata(&path).expect("png metadata should exist");

    let prepared = prepare_static_image_asset(
        &ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            resvg_available: false,
            magick_available: true,
            force_render_to_cache: true,
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("forced png preview should prepare successfully");

    assert_ne!(prepared.display_path, path);
    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 3200,
            height_px: 1800,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("rendered image should open")
            .with_guessed_format()
            .expect("rendered image format should be detected")
            .into_dimensions()
            .expect("rendered image should be readable"),
        (768, 432)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn oversized_extensionless_png_static_images_are_normalized_to_cached_overlays() {
    let root = temp_root("large-png-noext-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("background");
    write_test_raster_image(&path, ImageFormat::Png, 3200, 1800);
    let metadata = fs::metadata(&path).expect("png metadata should exist");

    let prepared = prepare_static_image_asset(
        &ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            resvg_available: false,
            magick_available: true,
            force_render_to_cache: false,
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("large extensionless png should prepare successfully");

    assert_ne!(prepared.display_path, path);
    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 3200,
            height_px: 1800,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("rendered image should open")
            .with_guessed_format()
            .expect("rendered image format should be detected")
            .into_dimensions()
            .expect("rendered image should be readable"),
        (768, 432)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
