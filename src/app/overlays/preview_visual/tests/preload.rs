use super::*;

#[test]
fn cached_adjacent_comic_page_queues_background_image_prepare() {
    let root = temp_root("comic-adjacent-image-prepare");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    fs::write(&archive, b"cbz").expect("failed to write archive placeholder");
    let archive_metadata = fs::metadata(&archive).expect("archive metadata should exist");
    let current_page = root.join("page-1.jpg");
    let next_page = root.join("page-2.jpg");
    write_test_raster_image(&current_page, ImageFormat::Jpeg, 1600, 900);
    write_test_raster_image(&next_page, ImageFormat::Jpeg, 1600, 900);
    let next_page_metadata = fs::metadata(&next_page).expect("next page metadata should exist");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
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
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.sync_comic_preview_selection();
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_navigation_position("Page", 0, 2, None)
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: current_page,
            size: 11 * 1024,
            modified: None,
        });
    app.apply_current_comic_preview_metadata();

    let adjacent_preview = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_navigation_position("Page", 1, 2, None)
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: next_page,
            size: next_page_metadata.len(),
            modified: next_page_metadata.modified().ok(),
        });
    let entry = app
        .selected_entry()
        .cloned()
        .expect("selected entry should exist");
    app.cache_preview_result(
        &entry,
        &preview::PreviewRequestOptions::ComicPage(1),
        &adjacent_preview,
    );
    let adjacent_request = app.preview_visual_overlay_request_for_visual(
        PreviewKind::Comic,
        adjacent_preview
            .preview_visual
            .as_ref()
            .expect("adjacent preview should have a visual"),
        app.frame_state
            .preview_media_area
            .expect("preview media area should exist"),
    );
    let adjacent_key = StaticImageKey::from_request(&adjacent_request);

    app.refresh_static_image_preloads();

    assert!(app.image_preview.pending_prepares.contains(&adjacent_key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cached_adjacent_epub_section_queues_background_image_prepare() {
    let root = temp_root("epub-adjacent-image-prepare");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let epub = root.join("story.epub");
    fs::write(&epub, b"epub").expect("failed to write epub placeholder");
    let epub_metadata = fs::metadata(&epub).expect("epub metadata should exist");
    let current_page = root.join("page-1.jpg");
    let next_page = root.join("page-2.jpg");
    write_test_raster_image(&current_page, ImageFormat::Jpeg, 1600, 900);
    write_test_raster_image(&next_page, ImageFormat::Jpeg, 1600, 900);
    let next_page_metadata = fs::metadata(&next_page).expect("next page metadata should exist");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.entries = vec![Entry {
        path: epub.clone(),
        name: "story.epub".to_string(),
        name_key: "story.epub".to_string(),
        kind: EntryKind::File,
        size: epub_metadata.len(),
        modified: epub_metadata.modified().ok(),
        readonly: false,
    }];
    app.selected = 0;
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.sync_epub_preview_selection();
    app.preview_state.content = PreviewContent::new(PreviewKind::Document, Vec::new())
        .with_ebook_section(0, 2, Some("Page 1".to_string()))
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: current_page,
            size: 11 * 1024,
            modified: None,
        });
    app.apply_current_epub_preview_metadata();

    let adjacent_preview = PreviewContent::new(PreviewKind::Document, Vec::new())
        .with_ebook_section(1, 2, Some("Page 2".to_string()))
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: next_page,
            size: next_page_metadata.len(),
            modified: next_page_metadata.modified().ok(),
        });
    let entry = app
        .selected_entry()
        .cloned()
        .expect("selected entry should exist");
    app.cache_preview_result(
        &entry,
        &preview::PreviewRequestOptions::EpubSection(1),
        &adjacent_preview,
    );
    let adjacent_request = app.preview_visual_overlay_request_for_visual(
        PreviewKind::Document,
        adjacent_preview
            .preview_visual
            .as_ref()
            .expect("adjacent preview should have a visual"),
        app.frame_state
            .preview_media_area
            .expect("preview media area should exist"),
    );
    let adjacent_key = StaticImageKey::from_request(&adjacent_request);

    app.refresh_static_image_preloads();

    assert!(app.image_preview.pending_prepares.contains(&adjacent_key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn stale_adjacent_comic_preview_result_immediately_queues_image_prepare() {
    let root = temp_root("comic-stale-adjacent-image-prepare");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    let page_one = raster_image_bytes(ImageFormat::Jpeg, 1600, 900);
    let page_two = raster_image_bytes(ImageFormat::Jpeg, 1600, 900);
    write_binary_zip_entries(&archive, &[("1.jpg", &page_one), ("2.jpg", &page_two)]);

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    wait_for_preview_prefetch(&mut app);

    let mut adjacent_key = None;
    for _ in 0..200 {
        let _ = app.process_preview_prefetch_timers();
        let _ = app.process_background_jobs();
        if !app.has_cached_comic_preview_page(&archive, 1) {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        adjacent_key = app
            .nearby_comic_preview_visual_overlay_requests()
            .into_iter()
            .next()
            .map(|request| StaticImageKey::from_request(&request));
        if adjacent_key.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let adjacent_key = adjacent_key.expect("adjacent comic preview should be cached");
    assert!(
        app.image_preview.pending_prepares.contains(&adjacent_key)
            || app.image_preview.dimensions.contains_key(&adjacent_key)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
