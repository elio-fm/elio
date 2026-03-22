use super::*;

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
fn immediate_selection_changes_do_not_delay_static_image_activation() {
    let (mut app, root) = build_selected_static_image_app("image-activation", "demo.png");

    app.select_index(0);

    assert!(app.image_selection_activation_ready());
    assert!(app.pending_image_preview_timer().is_none());

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
