use super::*;

fn configure_iterm_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.terminal_images.protocol = ImageProtocol::ItermInline;
    app.terminal_images.window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
}

#[test]
fn page_image_visual_uses_full_preview_height() {
    let root = temp_root("full-height");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview_state.content = PreviewContent::new(PreviewKind::Archive, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: root.join("page.jpg"),
            size: 11 * 1024,
            modified: None,
        });

    assert_eq!(
        app.preview_visual_rows(Rect {
            x: 0,
            y: 0,
            width: 48,
            height: 20,
        }),
        Some(20)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn failed_full_height_page_image_falls_back_to_text_layout() {
    let root = temp_root("failed-full-height");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: root.join("page.jpg"),
            size: 11 * 1024,
            modified: None,
        });
    let area = Rect {
        x: 0,
        y: 0,
        width: 48,
        height: 20,
    };
    let request = StaticImageOverlayRequest {
        path: root.join("page.jpg"),
        size: 11 * 1024,
        modified: None,
        area,
        target_width_px: image_target_width_px(area, app.cached_terminal_window()),
        target_height_px: image_target_height_px(area, app.cached_terminal_window()),
        mode: StaticImageOverlayMode::Inline,
        force_render_to_cache: false,
        prepare_inline_payload: false,
    };
    app.image_preview
        .failed_images
        .insert(StaticImageKey::from_request(&request));

    assert_eq!(app.preview_visual_rows(area), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn inline_page_image_leaves_room_for_summary_text() {
    let root = temp_root("inline-page");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.set_ffmpeg_available_for_tests(true);
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::Inline,
            path: root.join("page.jpg"),
            size: 11 * 1024,
            modified: None,
        });

    assert_eq!(
        app.preview_visual_rows(Rect {
            x: 0,
            y: 0,
            width: 48,
            height: 20,
        }),
        Some(14)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn inline_cover_uses_more_of_the_preview_panel_height() {
    let root = temp_root("inline-cover");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.set_ffmpeg_available_for_tests(true);
    app.preview_state.content = PreviewContent::new(PreviewKind::Video, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::Cover,
            layout: PreviewVisualLayout::Inline,
            path: root.join("cover.png"),
            size: 11 * 1024,
            modified: None,
        });

    assert_eq!(
        app.preview_visual_rows(Rect {
            x: 0,
            y: 0,
            width: 48,
            height: 20,
        }),
        Some(10)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn page_image_placeholder_message_stays_silent() {
    let root = temp_root("page-placeholder");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_media_area = Some(Rect {
        x: 0,
        y: 0,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: root.join("page.jpg"),
            size: 11 * 1024,
            modified: None,
        });

    assert_eq!(app.preview_visual_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn document_page_image_prepares_in_background_before_display() {
    let root = temp_root("document-jpeg-background");
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
    app.preview_state.content = PreviewContent::new(PreviewKind::Document, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: page,
            size: page_size,
            modified: None,
        });
    let request = app
        .active_preview_visual_overlay_request()
        .expect("document page image request should be available");
    let key = StaticImageKey::from_request(&request);

    app.refresh_static_image_preloads();
    assert!(app.image_preview.pending_prepares.contains(&key));
    app.present_preview_overlay()
        .expect("presenting a document page overlay should not fail");
    assert!(!app.static_image_overlay_displayed());
    wait_for_displayed_preview_overlay(&mut app);

    assert!(app.static_image_overlay_displayed());
    assert!(app.image_preview.dimensions.contains_key(&key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iterm_inline_page_image_clear_area_covers_preview_body_without_header() {
    let root = temp_root("iterm-inline-clear-area");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let page = root.join("page.png");
    write_test_raster_image(&page, ImageFormat::Png, 900, 1400);
    let page_size = fs::metadata(&page)
        .expect("page image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_iterm_image_support(&mut app);
    app.entries.clear();
    app.selected = 0;
    app.frame_state.preview_panel = Some(Rect {
        x: 1,
        y: 1,
        width: 50,
        height: 24,
    });
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 12,
    });
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 15,
        width: 48,
        height: 8,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::Inline,
            path: page,
            size: page_size,
            modified: None,
        });

    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);

    assert_eq!(
        app.displayed_static_image_clear_area(),
        Some(Rect {
            x: 2,
            y: 3,
            width: 48,
            height: 20,
        })
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn document_overlay_keeps_previous_page_visible_while_next_page_waits() {
    let root = temp_root("document-overlay-pending");
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
    app.preview_state.content = PreviewContent::new(PreviewKind::Document, Vec::new())
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

    app.preview_state.content = PreviewContent::new(PreviewKind::Document, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: second,
            size: 20 * 1024 * 1024,
            modified: None,
        });
    let next_request = app
        .active_preview_visual_overlay_request()
        .expect("next document preview request should be available");
    let next_key = StaticImageKey::from_request(&next_request);
    app.refresh_static_image_preloads();
    app.last_selection_change_at = Instant::now() - std::time::Duration::from_secs(1);
    app.sync_image_preview_selection_activation();

    app.present_preview_overlay()
        .expect("pending document page transition should not fail");
    assert!(app.static_image_overlay_displayed());
    assert!(app.image_preview.pending_prepares.contains(&next_key));
    assert!(!app.displayed_static_image_matches_active());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iterm_popup_clear_defers_page_image_erase_until_next_draw() {
    let root = temp_root("iterm-popup-deferred-erase");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let page = root.join("page.png");
    write_test_raster_image(&page, ImageFormat::Png, 600, 300);
    let page_size = fs::metadata(&page)
        .expect("page image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_iterm_image_support(&mut app);
    app.entries.clear();
    app.selected = 0;
    app.frame_state.preview_panel = Some(Rect {
        x: 1,
        y: 1,
        width: 50,
        height: 24,
    });
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 12,
    });
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 15,
        width: 48,
        height: 8,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::Inline,
            path: page,
            size: page_size,
            modified: None,
        });

    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    app.help_open = true;
    app.present_preview_overlay()
        .expect("iTerm popup clear should not fail");

    assert!(!app.static_image_overlay_displayed());
    let erase = String::from_utf8(app.iterm_pre_draw_erase())
        .expect("iTerm erase output should be valid utf8");
    assert!(erase.contains("\x1b[23;3H"));
    assert!(!erase.contains("\x1b[2;2H"));
    assert!(app.iterm_pre_draw_erase().is_empty());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
