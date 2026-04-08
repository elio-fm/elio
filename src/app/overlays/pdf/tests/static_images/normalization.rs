use super::*;

#[test]
fn oriented_jpeg_fallback_preview_uses_exif_corrected_dimensions() {
    let root = temp_root("image-oriented-jpeg-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("portrait.jpg");
    write_test_oriented_jpeg(&path, 60, 30, 6);
    let metadata = fs::metadata(&path).expect("jpeg metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
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
    if !crate::app::overlays::inline_image::command_exists("ffmpeg") {
        return;
    }

    let root = temp_root("image-oriented-jpeg-ffmpeg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("portrait.jpg");
    write_test_oriented_jpeg(&path, 60, 30, 6);
    let metadata = fs::metadata(&path).expect("jpeg metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
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

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
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

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
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

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
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
