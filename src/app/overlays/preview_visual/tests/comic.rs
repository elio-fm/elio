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
            size: page_size,
            modified: None,
        });
    let request = app
        .active_preview_visual_overlay_request()
        .expect("comic jpeg request should be available");
    let key = StaticImageKey::from_request(&request);

    app.refresh_static_image_preloads();
    assert!(app.preview.image.pending_prepares.contains(&key));
    app.present_preview_overlay()
        .expect("presenting a comic jpeg overlay should not fail");
    assert!(!app.static_image_overlay_displayed());
    wait_for_displayed_preview_overlay(&mut app);

    assert!(app.static_image_overlay_displayed());
    assert!(app.preview.image.dimensions.contains_key(&key));

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
            path: first,
            size: first_size,
            modified: None,
        });

    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
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
    app.input.last_selection_change_at = Instant::now() - std::time::Duration::from_secs(1);
    app.sync_image_preview_selection_activation();

    app.present_preview_overlay()
        .expect("pending comic page transition should not fail");
    assert!(app.static_image_overlay_displayed());
    assert!(app.preview.image.pending_prepares.contains(&next_key));
    assert!(!app.displayed_static_image_matches_active());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_overlay_clears_previous_file_page_while_next_comic_preview_loads() {
    // When navigating to a different comic file, the old file's page image
    // must be cleared immediately even while the new file's preview is still
    // loading.  Keeping cross-file stale images would cause the previous
    // comic's page to remain visible until the user explicitly re-navigates.
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
    app.navigation.entries.clear();
    app.navigation.selected = 0;
    app.input.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
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

    app.navigation.entries = vec![Entry {
        path: archive.clone(),
        name: "issue.cbz".to_string(),
        name_key: "issue.cbz".to_string(),
        kind: EntryKind::File,
        size: archive_metadata.len(),
        modified: archive_metadata.modified().ok(),
        readonly: false,
    }];
    app.navigation.selected = 0;
    app.sync_comic_preview_selection();
    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_navigation_position("Page", 1, 2, None);
    app.preview.state.load_state = Some(PreviewLoadState::Placeholder(archive));
    app.present_preview_overlay()
        .expect("presenting the overlay during cross-file loading should not fail");

    // The old file's page image must be cleared — not kept — when the new
    // entry is a different comic file.
    assert!(!app.static_image_overlay_displayed());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_overlay_clears_previous_file_page_while_next_epub_preview_loads() {
    // Regression test: fixed-layout EPUB page images go through the same
    // stale-overlay guard as comics, but the guard previously only checked
    // comic_preview.session — which is None for EPUBs.  Both displayed_page_
    // source and the current session were None, so None == None returned true
    // and the old EPUB's page image was kept visible when navigating to a
    // different EPUB.  After the fix the guard also consults epub_preview.
    // session, so cross-file EPUB navigation correctly clears the overlay.
    let root = temp_root("epub-overlay-cross-file");
    fs::create_dir_all(&root).expect("failed to create temp root");

    // Two placeholder EPUB files — content doesn't matter, only the extension.
    let epub_a = root.join("book_a.epub");
    let epub_b = root.join("book_b.epub");
    fs::write(&epub_a, b"epub").expect("failed to write epub_a placeholder");
    fs::write(&epub_b, b"epub").expect("failed to write epub_b placeholder");
    let epub_a_meta = fs::metadata(&epub_a).expect("epub_a metadata should exist");
    let epub_b_meta = fs::metadata(&epub_b).expect("epub_b metadata should exist");

    // Fake page image extracted from epub_a (a small PNG).
    let page = root.join("page_a.png");
    write_test_raster_image(&page, ImageFormat::Png, 600, 800);
    let page_size = fs::metadata(&page)
        .expect("page image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    // Advance the preview token so any preview job submitted by App::new_at
    // for the initial directory scan becomes stale and cannot overwrite the
    // manually-set preview content below.
    app.preview.state.token = app.preview.state.token.wrapping_add(1);

    // Select epub_a and display its page image.
    app.navigation.entries = vec![crate::app::Entry {
        path: epub_a.clone(),
        name: "book_a.epub".to_string(),
        name_key: "book_a.epub".to_string(),
        kind: crate::app::EntryKind::File,
        size: epub_a_meta.len(),
        modified: epub_a_meta.modified().ok(),
        readonly: false,
    }];
    app.navigation.selected = 0;
    app.sync_epub_preview_selection();
    app.input.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview.state.content = PreviewContent::new(PreviewKind::Document, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: page,
            size: page_size,
            modified: None,
        });
    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    // Navigate to epub_b: update the entry, sync the EPUB session, and put the
    // preview into the loading (Placeholder) state with no visual yet.
    app.navigation.entries = vec![crate::app::Entry {
        path: epub_b.clone(),
        name: "book_b.epub".to_string(),
        name_key: "book_b.epub".to_string(),
        kind: crate::app::EntryKind::File,
        size: epub_b_meta.len(),
        modified: epub_b_meta.modified().ok(),
        readonly: false,
    }];
    app.navigation.selected = 0;
    app.sync_epub_preview_selection();
    app.preview.state.content =
        PreviewContent::new(PreviewKind::Document, Vec::new()).with_detail("EPUB ebook");
    app.preview.state.load_state = Some(crate::app::state::PreviewLoadState::Placeholder(epub_b));

    app.present_preview_overlay()
        .expect("presenting overlay during cross-EPUB loading should not fail");

    // The previous EPUB's page image must be cleared immediately when the new
    // entry is a different EPUB file.
    assert!(
        !app.static_image_overlay_displayed(),
        "stale page from epub_a must not remain visible while epub_b is loading"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
