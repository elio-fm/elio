use super::*;
use crate::terminal_runtime::terminal_images::{
    RenderedImageDimensions, command_exists, read_png_dimensions,
};
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

fn write_test_transparent_png(path: &Path, width_px: u32, height_px: u32) {
    let mut image = RgbaImage::new(width_px, height_px);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = if (x + y) % 2 == 0 {
            Rgba([32, 128, 224, 255])
        } else {
            Rgba([0, 0, 0, 0])
        };
    }
    DynamicImage::ImageRgba8(image)
        .save_with_format(path, ImageFormat::Png)
        .expect("failed to write transparent png");
}

fn write_test_svg_image(path: &Path, width_px: u32, height_px: u32) {
    fs::write(
        path,
        format!(
            r#"<svg viewBox="0 0 {width_px} {height_px}" xmlns="http://www.w3.org/2000/svg"></svg>"#
        ),
    )
    .expect("failed to write svg placeholder");
}

fn write_test_image(root: &Path, file_name: &str) {
    let path = root.join(file_name);
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("png") => write_test_raster_image(&path, ImageFormat::Png, 600, 300),
        Some("ico") => write_test_raster_image(&path, ImageFormat::Ico, 64, 64),
        Some("jpg" | "jpeg") => write_test_raster_image(&path, ImageFormat::Jpeg, 600, 300),
        Some("gif") => write_test_raster_image(&path, ImageFormat::Gif, 600, 300),
        Some("webp") => write_test_raster_image(&path, ImageFormat::WebP, 600, 300),
        Some("svg") => write_test_svg_image(&path, 600, 300),
        _ => panic!("unsupported test image extension: {file_name}"),
    }
}

#[test]
fn extensionless_png_static_image_preparation_succeeds() {
    let root = temp_root("image-prepare-noext");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("background");
    write_test_raster_image(&path, ImageFormat::Png, 600, 300);
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
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("static image should prepare successfully");

    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 600,
            height_px: 300,
        }
    );
    assert_ne!(prepared.display_path, path);
    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("rendered image should open")
            .with_guessed_format()
            .expect("rendered image format should be detected")
            .into_dimensions()
            .expect("rendered image should be readable"),
        (600, 300)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn raster_static_images_use_png_display_paths() {
    for file_name in [
        "demo.png",
        "demo.ico",
        "demo.jpg",
        "demo.jpeg",
        "demo.gif",
        "demo.webp",
    ] {
        let root = temp_root("image-cache");
        fs::create_dir_all(&root).expect("failed to create temp root");
        write_test_image(&root, file_name);
        let path = root.join(file_name);
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
                prepare_inline_payload: false,
                sixel_prepare: None,
            },
            || false,
        )
        .expect("static image should prepare successfully");

        assert_eq!(
            prepared
                .display_path
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("png")
        );
        assert!(
            read_png_dimensions(&prepared.display_path).is_some(),
            "display path should be a readable PNG"
        );
        assert_ne!(prepared.display_path, path);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn svg_static_images_prefer_resvg_when_available() {
    if !command_exists("resvg") {
        return;
    }

    let root = temp_root("svg-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    write_test_image(&root, "demo.svg");
    let path = root.join("demo.svg");
    let metadata = fs::metadata(&path).expect("svg metadata should exist");
    let prepared = prepare_static_image_asset(
        &ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            resvg_available: true,
            magick_available: false,
            force_render_to_cache: false,
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("svg image should prepare successfully");

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
            width_px: 600,
            height_px: 300,
        }
    );
    assert_ne!(prepared.display_path, path);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn svg_static_images_fall_back_to_magick_when_resvg_is_unavailable() {
    if !command_exists("magick") {
        return;
    }

    let root = temp_root("svg-magick-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    write_test_image(&root, "demo.svg");
    let path = root.join("demo.svg");
    let metadata = fs::metadata(&path).expect("svg metadata should exist");
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
    .expect("svg image should prepare successfully via magick fallback");

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
            width_px: 600,
            height_px: 300,
        }
    );
    assert_ne!(prepared.display_path, path);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_svg_static_image_preparation_succeeds() {
    if !command_exists("resvg") {
        return;
    }

    let root = temp_root("svg-noext-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("logo");
    write_test_svg_image(&path, 600, 300);
    let metadata = fs::metadata(&path).expect("svg metadata should exist");

    let prepared = prepare_static_image_asset(
        &ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            resvg_available: true,
            magick_available: false,
            force_render_to_cache: false,
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("extensionless svg should prepare successfully");

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
            width_px: 600,
            height_px: 300,
        }
    );
    assert_ne!(prepared.display_path, path);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn png_static_image_preparation_preserves_alpha_channel() {
    let root = temp_root("png-alpha");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("alpha.png");
    write_test_transparent_png(&path, 8, 8);
    let metadata = fs::metadata(&path).expect("png metadata should exist");

    let prepared = prepare_static_image_asset(
        &ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 8,
            target_height_px: 8,
            ffmpeg_available: true,
            resvg_available: false,
            magick_available: true,
            force_render_to_cache: false,
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        || false,
    )
    .expect("png should prepare successfully");

    let pixels = image::ImageReader::open(&prepared.display_path)
        .expect("prepared image should open")
        .with_guessed_format()
        .expect("prepared image format should be detected")
        .decode()
        .expect("prepared image should decode")
        .into_rgba8();

    assert_eq!(pixels.get_pixel(1, 0).0[3], 0);
    assert_eq!(pixels.get_pixel(0, 0).0[3], 255);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
