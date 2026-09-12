use super::*;

pub(super) fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-static-image-{label}-{unique}"))
}

pub(super) fn configure_terminal_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.preview.terminal_images.protocol = ImageProtocol::KittyGraphics;
    app.preview.terminal_images.window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
}

pub(super) fn configure_iterm_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.preview.terminal_images.protocol = ImageProtocol::ItermInline;
    app.preview.terminal_images.window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
}

pub(super) fn blank_frame_buffer() -> Buffer {
    Buffer::empty(Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 40,
    })
}

pub(super) fn write_test_raster_image(
    path: &Path,
    format: ImageFormat,
    width_px: u32,
    height_px: u32,
) {
    let mut image = RgbaImage::new(width_px, height_px);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([32, 128, 224, 255]);
    }

    DynamicImage::ImageRgba8(image)
        .save_with_format(path, format)
        .expect("failed to write raster test image");
}

pub(super) fn set_single_test_entry(app: &mut App, path: &Path) {
    let metadata = fs::metadata(path).expect("file metadata should exist");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("file name should be valid utf-8");
    app.file_browser.entries = vec![Entry {
        path: path.to_path_buf(),
        name: name.to_string(),
        name_key: name.to_ascii_lowercase(),
        kind: EntryKind::File,
        symlink: None,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        readonly: false,
    }];
    app.file_browser.selected = 0;
    app.input.screen_regions.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.screen_regions.metrics.cols = 1;
    app.input.screen_regions.metrics.rows_visible = 6;
}

pub(super) fn set_single_unmodified_test_entry(app: &mut App, path: &Path) {
    set_single_test_entry(app, path);
    app.file_browser.entries[0].modified = None;
}

pub(super) fn build_selected_static_image_app(
    label: &str,
    file_name: &str,
) -> (App, PathBuf, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let image_path = root.join(file_name);
    write_test_raster_image(&image_path, ImageFormat::Png, 600, 300);

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;
    set_single_test_entry(&mut app, &image_path);
    app.refresh_preview();

    (app, root, image_path)
}

pub(super) fn ready_static_image_overlay(app: &mut App) -> StaticImageOverlayRequest {
    app.preview.image.selection_activation_delay = Duration::ZERO;
    app.sync_image_preview_selection_activation();
    app.active_static_image_overlay_request()
        .expect("static image overlay request should exist")
}

pub(super) fn wait_for_displayed_static_image_overlay(app: &mut App) {
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        let _ = app.process_image_preview_timers();
        app.present_preview_overlay()
            .expect("presenting static image overlay should not fail");
        if app.static_image_overlay_displayed() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for static image overlay");
}

pub(super) fn displayed_sixel_static_image_overlay(
    app: &mut App,
    identity: TerminalIdentity,
) -> StaticImageOverlayRequest {
    let request = ready_static_image_overlay(app);
    app.set_terminal_image_protocol_for_tests(ImageProtocol::Sixel, identity);

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let _ = app.process_background_jobs();
        let _ = app.process_image_preview_timers();
        let output = app
            .present_preview_overlay()
            .expect("initial Sixel image presentation should succeed");
        if !output.is_empty() && app.static_image_overlay_displayed() {
            return request;
        }
        thread::sleep(Duration::from_millis(10));
    }

    panic!("timed out waiting for initial Sixel image presentation");
}
