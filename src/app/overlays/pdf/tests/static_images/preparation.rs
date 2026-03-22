use super::*;

#[test]
fn current_small_jpeg_queues_background_prepare_for_overlay() {
    let root = temp_root("image-inline-jpeg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;

    let path = root.join("photo.jpg");
    write_test_raster_image(&path, ImageFormat::Jpeg, 600, 300);
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let key = StaticImageKey::from_request(&request);
    match app.prepared_static_image_for_overlay(&request) {
        crate::app::overlays::images::StaticImageOverlayPreparation::Pending => {}
        _ => panic!("small jpeg should prepare in the background"),
    }
    assert!(app.image_preview.pending_prepares.contains(&key));
    assert_eq!(
        app.preview_overlay_placeholder_message().as_deref(),
        Some("Preparing image preview")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn current_large_jpeg_queues_background_prepare_when_ffmpeg_is_available() {
    if !crate::app::overlays::inline_image::command_exists("ffmpeg") {
        return;
    }

    let root = temp_root("image-inline-large-jpeg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;

    let path = root.join("photo.jpg");
    write_test_raster_image(&path, ImageFormat::Jpeg, 3200, 1800);
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let key = StaticImageKey::from_request(&request);
    match app.prepared_static_image_for_overlay(&request) {
        crate::app::overlays::images::StaticImageOverlayPreparation::Pending => {}
        _ => panic!("large jpeg should prepare in the background when ffmpeg is available"),
    }
    assert!(app.image_preview.pending_prepares.contains(&key));
    assert_eq!(
        app.preview_overlay_placeholder_message().as_deref(),
        Some("Preparing image preview")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_png_static_image_preparation_succeeds() {
    let (_app, root) = build_selected_extensionless_png_app("image-prepare-noext", "background");
    let path = root.join("background");
    let metadata = fs::metadata(&path).expect("image metadata should exist");
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
    for file_name in ["demo.png", "demo.jpg", "demo.jpeg", "demo.gif", "demo.webp"] {
        let (_app, root) = build_selected_static_image_app("image-cache", file_name);
        let path = root.join(file_name);
        let metadata = fs::metadata(&path).expect("image metadata should exist");
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
        assert_eq!(
            Some(prepared.dimensions),
            read_png_dimensions(&prepared.display_path)
        );
        assert_ne!(prepared.display_path, path);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn svg_static_images_prefer_resvg_when_available() {
    if !crate::app::overlays::inline_image::command_exists("resvg") {
        return;
    }

    let (_app, root) = build_selected_static_image_app("svg-cache", "demo.svg");
    let path = root.join("demo.svg");
    let metadata = fs::metadata(&path).expect("svg metadata should exist");
    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
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
    if !crate::app::overlays::inline_image::command_exists("magick") {
        return;
    }

    let (_app, root) = build_selected_static_image_app("svg-magick-fallback", "demo.svg");
    let path = root.join("demo.svg");
    let metadata = fs::metadata(&path).expect("svg metadata should exist");
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
    if !crate::app::overlays::inline_image::command_exists("resvg") {
        return;
    }

    let root = temp_root("svg-noext-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("logo");
    write_test_svg_image(&path, 600, 300);
    let metadata = fs::metadata(&path).expect("svg metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
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

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
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
