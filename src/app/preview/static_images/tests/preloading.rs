use super::*;

#[test]
fn cold_sixel_jpeg_selection_defers_first_keyboard_preview_refresh() {
    let root = temp_root("cold-sixel-jpeg-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.jpg", "c.txt"] {
        let path = root.join(name);
        if name.ends_with(".jpg") {
            write_test_raster_image(&path, ImageFormat::Jpeg, 1600, 900);
        } else {
            fs::write(path, name).expect("failed to write temp file");
        }
    }

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.terminal_images.protocol = ImageProtocol::Sixel;
    app.preview.pdf.pdf_tools_available = true;
    app.file_browser.view_mode = ViewMode::List;
    app.set_ffmpeg_available_for_tests(true);
    app.file_browser.entries = ["a.txt", "b.jpg", "c.txt"]
        .into_iter()
        .map(|name| {
            let path = root.join(name);
            let metadata = fs::metadata(&path).expect("test file metadata should exist");
            Entry {
                path,
                name: name.to_string(),
                name_key: name.to_ascii_lowercase(),
                kind: EntryKind::File,
                symlink: None,
                size: metadata.len(),
                modified: metadata.modified().ok(),
                readonly: false,
            }
        })
        .collect();
    app.file_browser.selected = 0;
    app.refresh_preview();

    let token_before = app.preview.state.token;
    app.move_vertical_keyboard(1);

    assert_eq!(app.file_browser.selected, 1);
    assert_eq!(
        app.preview.state.token, token_before,
        "cold sixel jpeg should defer the first keyboard refresh"
    );
    assert!(
        app.preview.state.deferred_refresh_at.is_some(),
        "cold sixel jpeg should schedule a deferred refresh"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cold_sixel_comic_selection_defers_first_keyboard_preview_refresh() {
    let root = temp_root("cold-sixel-comic-preview-defer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.cbz", "c.txt"] {
        let path = root.join(name);
        fs::write(path, name).expect("failed to write temp file");
    }

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.terminal_images.protocol = ImageProtocol::Sixel;
    app.preview.pdf.pdf_tools_available = true;
    app.file_browser.view_mode = ViewMode::List;
    app.file_browser.entries = ["a.txt", "b.cbz", "c.txt"]
        .into_iter()
        .map(|name| {
            let path = root.join(name);
            let metadata = fs::metadata(&path).expect("test file metadata should exist");
            Entry {
                path,
                name: name.to_string(),
                name_key: name.to_ascii_lowercase(),
                kind: EntryKind::File,
                symlink: None,
                size: metadata.len(),
                modified: metadata.modified().ok(),
                readonly: false,
            }
        })
        .collect();
    app.file_browser.selected = 0;
    app.refresh_preview();

    let token_before = app.preview.state.token;
    app.move_vertical_keyboard(1);

    assert_eq!(app.file_browser.selected, 1);
    assert_eq!(
        app.preview.state.token, token_before,
        "cold sixel comic should defer the first keyboard refresh"
    );
    assert!(
        app.preview.state.deferred_refresh_at.is_some(),
        "cold sixel comic should schedule a deferred refresh"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sixel_preloads_visible_static_images_before_selection_lands_on_them() {
    let root = temp_root("sixel-visible-static-preload");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.jpg", "c.txt"] {
        let path = root.join(name);
        if name.ends_with(".jpg") {
            write_test_raster_image(&path, ImageFormat::Jpeg, 1600, 900);
        } else {
            fs::write(path, name).expect("failed to write temp file");
        }
    }

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.terminal_images.protocol = ImageProtocol::Sixel;
    app.preview.pdf.pdf_tools_available = true;
    app.file_browser.view_mode = ViewMode::List;
    app.set_ffmpeg_available_for_tests(true);
    app.file_browser.entries = ["a.txt", "b.jpg", "c.txt"]
        .into_iter()
        .map(|name| {
            let path = root.join(name);
            let metadata = fs::metadata(&path).expect("test file metadata should exist");
            Entry {
                path,
                name: name.to_string(),
                name_key: name.to_ascii_lowercase(),
                kind: EntryKind::File,
                symlink: None,
                size: metadata.len(),
                modified: metadata.modified().ok(),
                readonly: false,
            }
        })
        .collect();
    app.file_browser.selected = 0;
    app.input.screen_regions.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.screen_regions.metrics.cols = 1;
    app.input.screen_regions.metrics.rows_visible = 6;
    app.refresh_preview();
    app.refresh_static_image_preloads();

    let image_entry = &app.file_browser.entries[1];
    let request = app
        .static_image_overlay_request_for_entry(image_entry)
        .expect("visible jpeg should have a static image overlay request");
    let key = StaticImageKey::from_request(&request);

    assert!(
        app.preview.image.pending_prepares.contains(&key),
        "visible sixel image should be preloaded even before selection reaches it"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn foot_sixel_limits_nearby_static_image_preloads() {
    let root = temp_root("foot-sixel-preload-limit");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.jpg", "c.jpg", "d.jpg", "e.jpg"] {
        let path = root.join(name);
        if name.ends_with(".jpg") {
            write_test_raster_image(&path, ImageFormat::Jpeg, 1600, 900);
        } else {
            fs::write(path, name).expect("failed to write temp file");
        }
    }

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.terminal_images.protocol = ImageProtocol::Sixel;
    app.preview.terminal_images.identity = crate::terminal_images::TerminalIdentity::Foot;
    app.preview.pdf.pdf_tools_available = true;
    app.file_browser.view_mode = ViewMode::List;
    app.set_ffmpeg_available_for_tests(true);
    app.file_browser.entries = ["a.txt", "b.jpg", "c.jpg", "d.jpg", "e.jpg"]
        .into_iter()
        .map(|name| {
            let path = root.join(name);
            let metadata = fs::metadata(&path).expect("test file metadata should exist");
            Entry {
                path,
                name: name.to_string(),
                name_key: name.to_ascii_lowercase(),
                kind: EntryKind::File,
                symlink: None,
                size: metadata.len(),
                modified: metadata.modified().ok(),
                readonly: false,
            }
        })
        .collect();
    app.file_browser.selected = 0;
    app.input.screen_regions.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.screen_regions.metrics.cols = 1;
    app.input.screen_regions.metrics.rows_visible = 10;
    app.refresh_preview();
    app.refresh_static_image_preloads();

    assert_eq!(
        app.preview.image.pending_prepares.len(),
        STATIC_IMAGE_PRELOAD_LIMIT_SLOW_SIXEL
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn windows_terminal_sixel_limits_nearby_static_image_preloads() {
    let root = temp_root("wt-sixel-preload-limit");
    fs::create_dir_all(&root).expect("failed to create temp root");
    for name in ["a.txt", "b.jpg", "c.jpg", "d.jpg", "e.jpg"] {
        let path = root.join(name);
        if name.ends_with(".jpg") {
            write_test_raster_image(&path, ImageFormat::Jpeg, 1600, 900);
        } else {
            fs::write(path, name).expect("failed to write temp file");
        }
    }

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.terminal_images.protocol = ImageProtocol::Sixel;
    app.preview.terminal_images.identity =
        crate::terminal_images::TerminalIdentity::WindowsTerminal;
    app.preview.pdf.pdf_tools_available = true;
    app.file_browser.view_mode = ViewMode::List;
    app.set_ffmpeg_available_for_tests(true);
    app.file_browser.entries = ["a.txt", "b.jpg", "c.jpg", "d.jpg", "e.jpg"]
        .into_iter()
        .map(|name| {
            let path = root.join(name);
            let metadata = fs::metadata(&path).expect("test file metadata should exist");
            Entry {
                path,
                name: name.to_string(),
                name_key: name.to_ascii_lowercase(),
                kind: EntryKind::File,
                symlink: None,
                size: metadata.len(),
                modified: metadata.modified().ok(),
                readonly: false,
            }
        })
        .collect();
    app.file_browser.selected = 0;
    app.input.screen_regions.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.screen_regions.metrics.cols = 1;
    app.input.screen_regions.metrics.rows_visible = 10;
    app.refresh_preview();
    app.refresh_static_image_preloads();

    assert_eq!(
        app.preview.image.pending_prepares.len(),
        STATIC_IMAGE_PRELOAD_LIMIT_SLOW_SIXEL
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
