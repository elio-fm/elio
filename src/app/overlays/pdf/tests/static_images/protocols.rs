use super::*;

#[test]
fn current_extensionless_png_uses_direct_kitty_source_overlay() {
    let root = temp_root("image-inline-noext");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;

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
    assert!(!app.preview.image.pending_prepares.contains(&key));
    assert!(app.pending_image_preview_timer().is_none());
    assert_eq!(app.preview_overlay_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn prepared_full_pane_image_uses_full_pane_kitty_placement() {
    let root = temp_root("image-placement-from-rendered-png");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    app.preview.terminal_images.protocol = ImageProtocol::KittyGraphics;
    app.preview.terminal_images.window = Some(TerminalWindowSize {
        cells_width: 100,
        cells_height: 50,
        pixels_width: 1000,
        pixels_height: 1000,
    });
    app.preview.pdf.pdf_tools_available = true;

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
            sixel_dcs: None,
            sixel_dcs_key: None,
        }),
    });

    assert!(dirty);
    app.preview.image.selection_activation_delay = Duration::ZERO;
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
