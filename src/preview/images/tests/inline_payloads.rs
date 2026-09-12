use super::*;
use crate::terminal_images::RenderedImageDimensions;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
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

fn write_large_test_jpeg(path: &Path) {
    let mut image = RgbImage::new(1800, 1200);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = Rgb([
            ((x * 31 + y * 17) & 0xff) as u8,
            ((x * 13 + y * 47) & 0xff) as u8,
            ((x * 71 + y * 5) & 0xff) as u8,
        ]);
    }
    DynamicImage::ImageRgb8(image)
        .save_with_format(path, ImageFormat::Jpeg)
        .expect("failed to write large jpeg");
}

#[test]
fn iterm_png_and_jpeg_static_images_use_direct_source_payloads() {
    for (file_name, format) in [
        ("direct.png", ImageFormat::Png),
        ("direct.jpg", ImageFormat::Jpeg),
    ] {
        let root = temp_root("iterm-direct-static-image");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join(file_name);
        write_test_raster_image(&path, format, 600, 300);
        let metadata = fs::metadata(&path).expect("image metadata should exist");

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
                prepare_inline_payload: true,
                sixel_prepare: None,
            },
            || false,
        )
        .expect("iterm direct static image should prepare successfully");

        assert_eq!(prepared.display_path, path);
        assert_eq!(
            prepared.dimensions,
            RenderedImageDimensions {
                width_px: 600,
                height_px: 300,
            }
        );
        assert!(prepared.inline_payload.is_some());
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn iterm_large_jpeg_static_image_uses_compact_cached_payload() {
    let root = temp_root("iterm-compact-static-image");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("large.jpg");
    write_large_test_jpeg(&path);
    let metadata = fs::metadata(&path).expect("image metadata should exist");
    assert!(metadata.len() > 800 * 1024);

    let prepared = prepare_static_image_asset(
        &ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 360,
            target_height_px: 240,
            ffmpeg_available: false,
            resvg_available: false,
            magick_available: false,
            force_render_to_cache: false,
            prepare_inline_payload: true,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("large iterm jpeg should prepare successfully");

    assert_ne!(prepared.display_path, path);
    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("jpg")
    );
    assert!(
        prepared
            .inline_payload
            .as_ref()
            .is_some_and(|payload| payload.len() < metadata.len() as usize)
    );
    let rendered = image::ImageReader::open(&prepared.display_path)
        .expect("compact jpeg should exist")
        .decode()
        .expect("compact jpeg should decode");
    assert!(rendered.width() <= 360);
    assert!(rendered.height() <= 240);
    assert!(
        fs::metadata(&prepared.display_path)
            .expect("compact jpeg metadata should exist")
            .len()
            < metadata.len()
    );
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
