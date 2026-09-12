use super::*;

#[test]
fn kitty_png_overlay_uses_source_path_for_direct_display() {
    let (mut app, root, image_path) = build_selected_static_image_app("direct-source", "demo.png");
    let request = ready_static_image_overlay(&mut app);
    let key = StaticImageKey::from_request(&request);

    match app.prepared_static_image_for_overlay(&request) {
        StaticImageOverlayPreparation::Ready(prepared) => {
            assert_eq!(prepared.display_path, image_path);
            assert_eq!(
                prepared.dimensions,
                RenderedImageDimensions {
                    width_px: 600,
                    height_px: 300,
                }
            );
            assert!(prepared.inline_payload.is_none());
        }
        StaticImageOverlayPreparation::Pending => {
            panic!("png source path should display directly in kitty")
        }
        StaticImageOverlayPreparation::Failed => {
            panic!("png source path should not fail direct display")
        }
    }

    assert!(app.preview.image.dimensions.contains_key(&key));
    assert!(!app.preview.image.pending_prepares.contains(&key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn konsole_png_overlay_uses_source_path_for_direct_display() {
    let (mut app, root, image_path) =
        build_selected_static_image_app("konsole-direct-source", "demo.png");
    app.preview.terminal_images.protocol = ImageProtocol::KittyDirectGraphics;
    let request = ready_static_image_overlay(&mut app);
    let key = StaticImageKey::from_request(&request);

    match app.prepared_static_image_for_overlay(&request) {
        StaticImageOverlayPreparation::Ready(prepared) => {
            assert_eq!(prepared.display_path, image_path);
            assert_eq!(
                prepared.dimensions,
                RenderedImageDimensions {
                    width_px: 600,
                    height_px: 300,
                }
            );
            assert!(prepared.inline_payload.is_none());
        }
        StaticImageOverlayPreparation::Pending => {
            panic!("png source path should display directly in Konsole")
        }
        StaticImageOverlayPreparation::Failed => {
            panic!("png source path should not fail direct Konsole display")
        }
    }

    assert!(app.preview.image.dimensions.contains_key(&key));
    assert!(!app.preview.image.pending_prepares.contains(&key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cached_rendered_overlay_reuses_cached_path_and_inline_payload() {
    let (mut app, root, image_path) = build_selected_static_image_app("cache-reuse", "demo.png");
    let mut request = ready_static_image_overlay(&mut app);
    request.force_render_to_cache = true;
    request.prepare_inline_payload = true;
    let key = StaticImageKey::from_request(&request);
    let rendered_path = root.join("demo-rendered.png");
    write_test_raster_image(&rendered_path, ImageFormat::Png, 320, 180);
    let payload: Arc<str> = Arc::from("YWJj");

    app.preview.image.dimensions.insert(
        key.clone(),
        RenderedImageDimensions {
            width_px: 320,
            height_px: 180,
        },
    );
    app.preview
        .image
        .remember_rendered_image(key.clone(), rendered_path.clone());
    app.preview
        .image
        .remember_inline_payload(key.clone(), Arc::clone(&payload));

    let protocol = app.preview.terminal_images.protocol;
    let prepared = app
        .preview
        .image
        .cached_prepared_image(&key, &request, protocol)
        .expect("cached rendered overlay should be reused");

    assert_eq!(prepared.display_path, rendered_path);
    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 320,
            height_px: 180,
        }
    );
    assert_eq!(prepared.inline_payload.as_deref(), Some(payload.as_ref()));
    assert_ne!(prepared.display_path, image_path);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn repeated_present_static_image_overlay_is_a_noop_when_nothing_changed() {
    let (mut app, root, _image_path) = build_selected_static_image_app("no-op-render", "demo.png");
    app.preview.image.selection_activation_delay = Duration::ZERO;
    app.sync_image_preview_selection_activation();

    let mut first = Vec::new();
    let first_state = app
        .present_static_image_overlay(ImageProtocol::KittyGraphics, &[], false, &mut first)
        .expect("first static image presentation should succeed");
    assert_eq!(first_state, OverlayPresentState::Displayed);
    assert!(!first.is_empty());
    assert!(app.static_image_overlay_displayed());

    let mut second = Vec::new();
    let second_state = app
        .present_static_image_overlay(ImageProtocol::KittyGraphics, &[], false, &mut second)
        .expect("repeat static image presentation should succeed");
    assert_eq!(second_state, OverlayPresentState::Displayed);
    assert!(second.is_empty(), "unchanged image should not redraw");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn newly_shown_static_image_preview_prefers_image_surface_before_frame_area_exists() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("shown-image-surface-before-area", "demo.png");

    app.toggle_preview_pane();
    app.input.screen_regions.preview_content_area = None;
    app.toggle_preview_pane();

    assert!(app.active_static_image_overlay_request().is_none());
    assert!(
        app.preview_will_use_static_image_surface_after_layout(),
        "first visible frame after showing preview should reserve the image surface instead of drawing text fallback into cells that Kitty will use for placeholders"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
