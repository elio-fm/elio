use super::*;
use crate::preview::PreviewKind;

fn write_test_svg_image(path: &Path, width_px: u32, height_px: u32) {
    fs::write(
        path,
        format!(
            r#"<svg viewBox="0 0 {width_px} {height_px}" xmlns="http://www.w3.org/2000/svg"></svg>"#
        ),
    )
    .expect("failed to write svg placeholder");
}

fn write_test_image(path: &Path) {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("png") => write_test_raster_image(path, ImageFormat::Png, 600, 300),
        Some("ico") => write_test_raster_image(path, ImageFormat::Ico, 64, 64),
        Some("jpg" | "jpeg") => write_test_raster_image(path, ImageFormat::Jpeg, 600, 300),
        Some("gif") => write_test_raster_image(path, ImageFormat::Gif, 600, 300),
        Some("webp") => write_test_raster_image(path, ImageFormat::WebP, 600, 300),
        Some("svg") => write_test_svg_image(path, 600, 300),
        Some("txt") => fs::write(path, "plain text").expect("failed to write text placeholder"),
        _ => panic!("unsupported test image path: {}", path.display()),
    }
}

fn build_selected_static_image_app(label: &str, file_name: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(file_name);
    write_test_image(&path);
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();
    (app, root)
}

fn build_selected_extensionless_png_app(label: &str, file_name: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(file_name);
    write_test_raster_image(&path, ImageFormat::Png, 600, 300);
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();
    (app, root)
}

fn build_multi_static_image_app(label: &str, file_names: &[&str]) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    for file_name in file_names {
        write_test_image(&root.join(file_name));
    }
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.frame_state.metrics.cols = 1;
    app.input.frame_state.metrics.rows_visible = 6;
    app.refresh_preview();
    (app, root)
}

#[test]
fn refresh_preview_uses_blank_static_image_surface_preview_when_backend_enabled() {
    for (file_name, detail) in [
        ("demo.png", "PNG image"),
        ("demo.ico", "ICO image"),
        ("demo.jpg", "JPEG image"),
        ("demo.jpeg", "JPEG image"),
        ("demo.gif", "GIF image"),
        ("demo.webp", "WebP image"),
        ("demo.svg", "SVG image"),
    ] {
        let (app, root) = build_selected_static_image_app("image-placeholder", file_name);

        assert_eq!(app.preview.state.content.kind, PreviewKind::Image);
        assert_eq!(app.preview.state.content.detail.as_deref(), Some(detail));
        assert!(app.preview.state.content.lines.is_empty());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn preview_prefers_image_surface_for_supported_static_images_when_backend_enabled() {
    for file_name in [
        "demo.png",
        "demo.ico",
        "demo.jpg",
        "demo.jpeg",
        "demo.gif",
        "demo.webp",
        "demo.svg",
    ] {
        let (app, root) = build_selected_static_image_app("image-surface", file_name);

        assert!(app.preview_prefers_image_surface());
        assert_eq!(app.preview_overlay_placeholder_message(), None);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn preview_prefers_image_surface_for_extensionless_png_when_backend_enabled() {
    let (app, root) = build_selected_extensionless_png_app("image-surface-noext", "background");

    assert_eq!(app.preview.state.content.kind, PreviewKind::Image);
    assert_eq!(
        app.preview.state.content.detail.as_deref(),
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
    app.preview.pdf.pdf_tools_available = false;
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
        .filter_map(|index| app.navigation.entries.get(index))
        .filter(|entry| crate::preview::images::static_image_detail_label(entry).is_some())
        .filter(|entry| {
            crate::file_classification::inspect_path_cached(
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
            app.preview.image.pending_prepares.contains(&key)
                || app.preview.image.dimensions.contains_key(&key),
            "expected image preload for {:?}",
            key
        );
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
