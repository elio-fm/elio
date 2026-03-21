use super::super::*;
use super::helpers::*;

#[test]
fn refresh_preview_uses_blank_static_image_surface_preview_when_backend_enabled() {
    for (file_name, detail) in [
        ("demo.png", "PNG image"),
        ("demo.jpg", "JPEG image"),
        ("demo.jpeg", "JPEG image"),
        ("demo.gif", "GIF image"),
        ("demo.webp", "WebP image"),
        ("demo.svg", "SVG image"),
    ] {
        let (app, root) = build_selected_static_image_app("image-placeholder", file_name);

        assert_eq!(app.preview_state.content.kind, PreviewKind::Image);
        assert_eq!(app.preview_state.content.detail.as_deref(), Some(detail));
        assert!(app.preview_state.content.lines.is_empty());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn preview_prefers_image_surface_for_supported_static_images_when_backend_enabled() {
    for (file_name, placeholder) in [
        ("demo.png", None),
        ("demo.jpg", Some("Preparing image preview")),
        ("demo.jpeg", Some("Preparing image preview")),
        ("demo.gif", Some("Preparing image preview")),
        ("demo.webp", Some("Preparing image preview")),
        ("demo.svg", Some("Preparing image preview")),
    ] {
        let (app, root) = build_selected_static_image_app("image-surface", file_name);

        assert!(app.preview_prefers_image_surface());
        assert_eq!(
            app.preview_overlay_placeholder_message().as_deref(),
            placeholder
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn preview_prefers_image_surface_for_extensionless_png_when_backend_enabled() {
    let (app, root) = build_selected_extensionless_png_app("image-surface-noext", "background");

    assert_eq!(app.preview_state.content.kind, PreviewKind::Image);
    assert_eq!(
        app.preview_state.content.detail.as_deref(),
        Some("PNG image")
    );
    assert!(app.preview_prefers_static_image_surface());
    assert!(app.preview_prefers_image_surface());
    assert_eq!(app.preview_overlay_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn current_extensionless_png_uses_direct_kitty_source_overlay() {
    let root = temp_root("image-inline-noext");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;

    let path = root.join("background");
    write_test_raster_image(&path, ImageFormat::Png, 600, 300);
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let key = StaticImageKey::from_request(&request);
    match app.prepared_static_image_for_overlay(&request) {
        crate::app::overlays::images::StaticImageOverlayPreparation::Ready(prepared) => {
            assert_eq!(prepared.display_path, path);
            assert_eq!(
                prepared.dimensions,
                RenderedImageDimensions {
                    width_px: 600,
                    height_px: 300,
                }
            );
        }
        _ => panic!("extensionless png should render directly in kitty"),
    }
    assert!(!app.image_preview.pending_prepares.contains(&key));
    assert!(app.pending_image_preview_timer().is_none());
    assert_eq!(app.preview_overlay_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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
fn prepared_full_pane_image_uses_full_pane_kitty_placement() {
    let root = temp_root("image-placement-from-rendered-png");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    app.terminal_images.protocol = ImageProtocol::KittyGraphics;
    app.terminal_images.window = Some(TerminalWindowSize {
        cells_width: 100,
        cells_height: 50,
        pixels_width: 1000,
        pixels_height: 1000,
    });
    app.pdf_preview.pdf_tools_available = true;

    let path = root.join("photo.jpg");
    write_test_raster_image(&path, ImageFormat::Jpeg, 1600, 900);
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let metadata = fs::metadata(&path).expect("image metadata should exist");
    let rendered = root.join("photo-rendered.png");
    write_test_raster_image(&rendered, ImageFormat::Png, 250, 540);

    let dirty = app.apply_image_prepare_build(crate::app::jobs::ImagePrepareBuild {
        path: path.clone(),
        size: metadata.len(),
        modified: None,
        target_width_px: request.target_width_px,
        target_height_px: request.target_height_px,
        force_render_to_cache: false,
        prepare_inline_payload: false,
        canceled: false,
        result: Some(crate::app::overlays::images::PreparedStaticImageAsset {
            display_path: rendered,
            dimensions: RenderedImageDimensions {
                width_px: 250,
                height_px: 540,
            },
            inline_payload: None,
        }),
    });

    assert!(dirty);
    app.image_preview.selection_activation_delay = Duration::ZERO;
    app.sync_image_preview_selection_activation();

    let output = String::from_utf8(
        app.present_preview_overlay()
            .expect("presenting prepared jpeg overlay should not fail"),
    )
    .expect("kitty output should be utf8");
    assert!(output.contains(&format!("c={}", request.area.width)));
    assert!(output.contains(&format!("r={}", request.area.height)));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn immediate_selection_changes_do_not_delay_static_image_activation() {
    let (mut app, root) = build_selected_static_image_app("image-activation", "demo.png");

    app.select_index(0);

    assert!(app.image_selection_activation_ready());
    assert!(app.pending_image_preview_timer().is_none());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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
fn static_image_surface_remains_available_without_pdf_tooling() {
    let (mut app, root) = build_selected_static_image_app("image-no-pdf-tools", "demo.png");
    app.pdf_preview.pdf_tools_available = false;
    app.refresh_preview();

    assert!(app.preview_prefers_static_image_surface());
    assert!(app.preview_prefers_image_surface());
    assert_eq!(app.preview_overlay_placeholder_message(), None);

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
            width_px: 768,
            height_px: 432,
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
            width_px: 768,
            height_px: 432,
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
            width_px: 768,
            height_px: 432,
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
fn refresh_preview_preloads_current_and_visible_nearby_static_images() {
    let (mut app, root) = build_multi_static_image_app(
        "image-preload-window",
        &["a.jpg", "b.txt", "c.png", "d.webp", "e.svg"],
    );
    app.set_selected(2);

    let current_request = app
        .active_static_image_overlay_request()
        .expect("current image request should be available");
    let target_width_px = current_request.target_width_px;
    let target_height_px = current_request.target_height_px;

    let expected = app
        .visible_entry_indices()
        .into_iter()
        .filter_map(|index| app.entries.get(index))
        .filter(|entry| crate::app::overlays::images::static_image_detail_label(entry).is_some())
        .filter(|entry| {
            crate::file_info::inspect_path_cached(
                &entry.path,
                entry.kind,
                entry.size,
                entry.modified,
            )
            .specific_type_label
                != Some("PNG image")
        })
        .map(|entry| {
            StaticImageKey::from_parts(
                entry.path.clone(),
                entry.size,
                entry.modified,
                target_width_px,
                target_height_px,
                false,
                false,
            )
        })
        .collect::<Vec<_>>();

    for key in expected {
        assert!(
            app.image_preview.pending_prepares.contains(&key)
                || app.image_preview.dimensions.contains_key(&key),
            "expected image preload for {:?}",
            key
        );
    }

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
