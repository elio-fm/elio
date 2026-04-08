use super::*;
use std::sync::{Arc, Barrier};

#[test]
fn page_image_overlay_request_uses_asset_metadata_without_forcing_render_cache() {
    let root = temp_root("request-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let asset_path = root.join("page.jpg");
    write_test_raster_image(&asset_path, ImageFormat::Jpeg, 600, 300);
    let asset_size = fs::metadata(&asset_path)
        .expect("asset metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.set_ffmpeg_available_for_tests(true);
    app.navigation.entries = vec![Entry {
        path: root.join("book.cbz"),
        name: "book.cbz".to_string(),
        name_key: "book.cbz".to_string(),
        kind: EntryKind::File,
        size: 134 * 1024 * 1024,
        modified: None,
        readonly: false,
    }];
    app.navigation.selected = 0;
    app.input.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview.state.content = PreviewContent::new(PreviewKind::Archive, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::Inline,
            path: asset_path.clone(),
            size: asset_size,
            modified: None,
        });

    let request = app
        .active_preview_visual_overlay_request()
        .expect("preview visual overlay request should be available");

    assert_eq!(request.path, asset_path);
    assert_eq!(request.size, asset_size);
    assert_eq!(request.modified, None);
    assert!(!request.force_render_to_cache);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn oversized_page_overlay_request_forces_rendered_cache() {
    let root = temp_root("page-force-render");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let page = root.join("page.jpg");
    write_test_raster_image(&page, ImageFormat::Jpeg, 1600, 900);

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.set_ffmpeg_available_for_tests(true);
    app.navigation.entries.clear();
    app.navigation.selected = 0;
    app.input.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: page,
            size: 20 * 1024 * 1024,
            modified: None,
        });

    let request = app
        .active_preview_visual_overlay_request()
        .expect("comic preview visual overlay request should be available");

    assert!(request.force_render_to_cache);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn concurrent_inline_raster_prepares_keep_shared_render_cache_readable() {
    let root = temp_root("concurrent-inline-render");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let page = root.join("page.jpg");
    write_test_raster_image(&page, ImageFormat::Jpeg, 2200, 3200);
    let metadata = fs::metadata(&page).expect("page image metadata should exist");
    let request = Arc::new(crate::app::jobs::ImagePrepareRequest {
        path: page,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        target_width_px: 384,
        target_height_px: 640,
        ffmpeg_available: false,
        resvg_available: false,
        magick_available: false,
        force_render_to_cache: false,
        prepare_inline_payload: false,
        sixel_prepare: None,
    });
    let barrier = Arc::new(Barrier::new(7));
    let mut handles = Vec::new();
    for _ in 0..6 {
        let request = Arc::clone(&request);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for _ in 0..6 {
                let prepared =
                    crate::app::overlays::images::prepare_static_image_asset(&request, || false)
                        .expect("shared render cache should prepare");
                let dimensions =
                    crate::app::overlays::inline_image::read_png_dimensions(&prepared.display_path)
                        .expect("shared render cache should contain a readable png");
                assert!(dimensions.width_px > 0);
                assert!(dimensions.height_px > 0);
                assert!(dimensions.width_px <= 384);
                assert!(dimensions.height_px <= 640);
                assert!(
                    prepared
                        .display_path
                        .parent()
                        .is_some_and(|parent| parent.ends_with("elio-image-preview-v3"))
                );
            }
        }));
    }

    barrier.wait();
    for handle in handles {
        handle
            .join()
            .expect("concurrent inline render worker should finish");
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn current_comic_prepare_build_marks_preview_dirty() {
    let root = temp_root("comic-prepare-dirty");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let source = root.join("page.jpg");
    let rendered = root.join("page-rendered.png");
    write_test_raster_image(&source, ImageFormat::Jpeg, 1600, 900);
    write_test_raster_image(&rendered, ImageFormat::Png, 768, 432);
    let metadata = fs::metadata(&source).expect("source image metadata should exist");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.set_ffmpeg_available_for_tests(true);
    app.navigation.entries.clear();
    app.navigation.selected = 0;
    app.input.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: source.clone(),
            size: metadata.len(),
            modified: None,
        });

    let dirty = app.apply_image_prepare_build(crate::app::jobs::ImagePrepareBuild {
        path: source,
        size: metadata.len(),
        modified: None,
        target_width_px: image_target_width_px(
            app.input
                .frame_state
                .preview_media_area
                .expect("preview media area should exist"),
            app.cached_terminal_window(),
        ),
        target_height_px: image_target_height_px(
            app.input
                .frame_state
                .preview_media_area
                .expect("preview media area should exist"),
            app.cached_terminal_window(),
        ),
        force_render_to_cache: false,
        prepare_inline_payload: false,
        canceled: false,
        result: Some(crate::app::overlays::images::PreparedStaticImageAsset {
            display_path: rendered,
            dimensions: crate::app::overlays::inline_image::RenderedImageDimensions {
                width_px: 768,
                height_px: 432,
            },
            inline_payload: None,
            sixel_dcs: None,
            sixel_dcs_key: None,
        }),
    });

    assert!(dirty);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
