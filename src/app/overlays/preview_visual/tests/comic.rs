use super::*;

#[test]
fn comic_page_ffmpeg_render_uses_fast_raster_args() {
    let default_args: &[&str] = &[];
    let comic_args: &[&str] = &["-compression_level", "1", "-sws_flags", "fast_bilinear"];

    assert_eq!(
        crate::app::overlays::images::ffmpeg_raster_render_args(false),
        default_args
    );
    assert_eq!(
        crate::app::overlays::images::ffmpeg_raster_render_args(true),
        comic_args
    );
}

#[test]
fn comic_jpeg_page_prepares_in_background_before_display() {
    let root = temp_root("comic-jpeg-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let page = root.join("page.jpg");
    write_test_raster_image(&page, ImageFormat::Jpeg, 1600, 900);
    let page_size = fs::metadata(&page)
        .expect("page image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.set_ffmpeg_available_for_tests(true);
    app.entries.clear();
    app.selected = 0;
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: page,
            size: page_size,
            modified: None,
        });
    let request = app
        .active_preview_visual_overlay_request()
        .expect("comic jpeg request should be available");
    let key = StaticImageKey::from_request(&request);

    app.refresh_static_image_preloads();
    assert!(app.image_preview.pending_prepares.contains(&key));
    app.present_preview_overlay()
        .expect("presenting a comic jpeg overlay should not fail");
    assert!(!app.static_image_overlay_displayed());
    wait_for_displayed_preview_overlay(&mut app);

    assert!(app.static_image_overlay_displayed());
    assert!(app.image_preview.dimensions.contains_key(&key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_overlay_keeps_previous_page_visible_while_next_page_waits() {
    let root = temp_root("comic-overlay-pending");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let first = root.join("001.png");
    let second = root.join("002.jpg");
    write_test_raster_image(&first, ImageFormat::Png, 600, 300);
    write_test_raster_image(&second, ImageFormat::Jpeg, 1600, 900);
    let first_size = fs::metadata(&first)
        .expect("first image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.set_ffmpeg_available_for_tests(true);
    app.entries.clear();
    app.selected = 0;
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: first,
            size: first_size,
            modified: None,
        });

    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: second,
            size: 20 * 1024 * 1024,
            modified: None,
        });
    let next_request = app
        .active_preview_visual_overlay_request()
        .expect("next comic preview request should be available");
    let next_key = StaticImageKey::from_request(&next_request);
    app.refresh_static_image_preloads();
    app.last_selection_change_at = Instant::now() - std::time::Duration::from_secs(1);
    app.sync_image_preview_selection_activation();

    app.present_preview_overlay()
        .expect("pending comic page transition should not fail");
    assert!(app.static_image_overlay_displayed());
    assert!(app.image_preview.pending_prepares.contains(&next_key));
    assert!(!app.displayed_static_image_matches_active());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_overlay_keeps_previous_page_visible_while_next_page_preview_loads() {
    let root = temp_root("comic-overlay-preview-loading");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    fs::write(&archive, b"cbz").expect("failed to write archive placeholder");
    let archive_metadata = fs::metadata(&archive).expect("archive metadata should exist");
    let first = root.join("001.png");
    write_test_raster_image(&first, ImageFormat::Png, 600, 300);
    let first_size = fs::metadata(&first)
        .expect("first image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.entries.clear();
    app.selected = 0;
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_navigation_position("Page", 0, 2, None)
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: first,
            size: first_size,
            modified: None,
        });
    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    app.entries = vec![Entry {
        path: archive.clone(),
        name: "issue.cbz".to_string(),
        name_key: "issue.cbz".to_string(),
        kind: EntryKind::File,
        size: archive_metadata.len(),
        modified: archive_metadata.modified().ok(),
        readonly: false,
    }];
    app.selected = 0;
    app.sync_comic_preview_selection();
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_navigation_position("Page", 1, 2, None);
    app.preview_state.load_state = Some(PreviewLoadState::Placeholder(archive));
    app.present_preview_overlay()
        .expect("comic preview loading transition should not clear the overlay");

    assert!(app.static_image_overlay_displayed());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
