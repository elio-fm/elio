use super::*;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::text::Line;

fn configure_iterm_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.preview.terminal_images.protocol = ImageProtocol::ItermInline;
    app.preview.terminal_images.window = Some(TerminalWindowSize {
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
    app.preview.state.content = PreviewContent::new(PreviewKind::Archive, Vec::new())
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
    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
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
    app.preview
        .image
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
    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
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
    app.preview.state.content = PreviewContent::new(PreviewKind::Video, Vec::new())
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
fn non_video_inline_cover_keeps_default_compact_height() {
    let root = temp_root("inline-cover-document");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.state.content = PreviewContent::new(PreviewKind::Document, Vec::new())
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
        Some(6)
    );

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
    app.navigation.entries.clear();
    app.navigation.selected = 0;
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
    let request = app
        .active_preview_visual_overlay_request()
        .expect("document page image request should be available");
    let key = StaticImageKey::from_request(&request);

    app.refresh_static_image_preloads();
    assert!(app.preview.image.pending_prepares.contains(&key));
    app.present_preview_overlay()
        .expect("presenting a document page overlay should not fail");
    assert!(!app.static_image_overlay_displayed());
    wait_for_displayed_preview_overlay(&mut app);

    assert!(app.static_image_overlay_displayed());
    assert!(app.preview.image.dimensions.contains_key(&key));

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
    app.navigation.entries.clear();
    app.navigation.selected = 0;
    app.input.frame_state.preview_panel = Some(Rect {
        x: 1,
        y: 1,
        width: 50,
        height: 24,
    });
    app.input.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 12,
    });
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 15,
        width: 48,
        height: 8,
    });
    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
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
    app.navigation.entries.clear();
    app.navigation.selected = 0;
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
            path: first,
            size: first_size,
            modified: None,
        });

    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    app.preview.state.content = PreviewContent::new(PreviewKind::Document, Vec::new())
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
    app.input.last_selection_change_at = Instant::now() - std::time::Duration::from_secs(1);
    app.sync_image_preview_selection_activation();

    app.present_preview_overlay()
        .expect("pending document page transition should not fail");
    assert!(app.static_image_overlay_displayed());
    assert!(app.preview.image.pending_prepares.contains(&next_key));
    assert!(!app.displayed_static_image_matches_active());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iterm_popup_keeps_the_displayed_image_visible() {
    let root = temp_root("iterm-popup-deferred-erase");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let page = root.join("page.png");
    write_test_raster_image(&page, ImageFormat::Png, 600, 300);
    let page_size = fs::metadata(&page)
        .expect("page image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_iterm_image_support(&mut app);
    app.navigation.entries.clear();
    app.navigation.selected = 0;
    app.input.frame_state.preview_panel = Some(Rect {
        x: 1,
        y: 1,
        width: 50,
        height: 24,
    });
    app.input.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 12,
    });
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 15,
        width: 48,
        height: 8,
    });
    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
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

    app.overlays.help = true;
    assert!(app.iterm_pre_draw_erase().is_empty());
    let out = app
        .present_preview_overlay()
        .expect("iTerm popup redraw should not fail");

    assert!(out.is_empty());
    assert!(app.static_image_overlay_displayed());
    assert!(app.iterm_pre_draw_erase().is_empty());

    app.overlays.help = false;
    let restore = String::from_utf8(
        app.present_preview_overlay()
            .expect("closing the popup should redraw the image"),
    )
    .expect("restored iTerm image output should be valid utf8");

    assert!(restore.contains("\x1b]1337;File=inline=1;"));
    assert!(app.static_image_overlay_displayed());
    assert!(
        app.present_preview_overlay()
            .expect("steady-state redraw should succeed")
            .is_empty()
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn closing_open_with_popup_restores_iterm_inline_image() {
    let root = temp_root("iterm-open-with-popup");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let page = root.join("page.png");
    write_test_raster_image(&page, ImageFormat::Png, 640, 360);
    let page_metadata = fs::metadata(&page).expect("page metadata should exist");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_iterm_image_support(&mut app);
    app.navigation.entries = vec![Entry {
        path: page.clone(),
        name: "page.png".to_string(),
        name_key: "page.png".to_string(),
        kind: EntryKind::File,
        size: page_metadata.len(),
        modified: page_metadata.modified().ok(),
        readonly: false,
    }];
    app.navigation.selected = 0;
    app.preview.image.selection_activation_delay = Duration::ZERO;
    app.input.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 12,
    });
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 15,
        width: 48,
        height: 8,
    });
    app.preview.state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::Inline,
            path: page,
            size: page_metadata.len(),
            modified: page_metadata.modified().ok(),
        });

    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Char('O'))))
        .expect("O should open the open-with overlay");
    assert!(app.overlays.open_with.is_some());
    assert!(app.iterm_pre_draw_erase().is_empty());

    let out = app
        .present_preview_overlay()
        .expect("iTerm open-with popup redraw should not fail");
    assert!(out.is_empty());
    assert!(app.static_image_overlay_displayed());
    assert!(app.iterm_pre_draw_erase().is_empty());

    app.handle_event(Event::Key(KeyEvent::from(KeyCode::Esc)))
        .expect("Esc should close the open-with overlay");
    let restore = String::from_utf8(
        app.present_preview_overlay()
            .expect("closing the open-with popup should redraw the image"),
    )
    .expect("restored iTerm image output should be valid utf8");

    assert!(restore.contains("\x1b]1337;File=inline=1;"));
    assert!(app.static_image_overlay_displayed());
    assert!(
        app.present_preview_overlay()
            .expect("steady-state redraw should succeed")
            .is_empty()
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iterm_pre_draw_erase_detects_cover_layout_change_before_frame_update() {
    let root = temp_root("iterm-cover-layout-change");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let cover = root.join("cover.png");
    write_test_raster_image(&cover, ImageFormat::Png, 600, 900);
    let cover_size = fs::metadata(&cover)
        .expect("cover metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_iterm_image_support(&mut app);
    app.navigation.entries.clear();
    app.navigation.selected = 0;
    app.preview.image.selection_activation_delay = Duration::ZERO;
    app.input.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 23,
        width: 48,
        height: 0,
    });
    app.preview.state.content = PreviewContent::new(PreviewKind::Document, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: cover.clone(),
            size: cover_size,
            modified: None,
        });
    assert!(
        app.active_preview_visual_overlay_request_unchecked()
            .is_some()
    );
    app.sync_image_preview_selection_activation();
    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    app.input.frame_state.preview_panel = Some(Rect {
        x: 1,
        y: 1,
        width: 50,
        height: 24,
    });
    app.input.frame_state.preview_body_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });

    // Simulate the next EPUB section switching to an inline cover + text
    // layout before the frame state has been recomputed by ratatui.
    app.preview.state.content = PreviewContent::new(
        PreviewKind::Document,
        vec![Line::from("Chapter text starts here.")],
    )
    .with_preview_visual(PreviewVisual {
        kind: PreviewVisualKind::Cover,
        layout: PreviewVisualLayout::Inline,
        path: cover,
        size: cover_size,
        modified: None,
    });

    assert!(
        !app.displayed_static_image_matches_active(),
        "current preview layout should no longer match the stale full-height cover"
    );
    assert!(
        !app.iterm_pre_draw_erase().is_empty(),
        "iTerm should erase the previous full-height cover before drawing text"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
