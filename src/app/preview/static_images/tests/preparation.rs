use super::*;

#[test]
fn current_small_jpeg_queues_background_prepare_for_overlay() {
    let root = temp_root("image-inline-jpeg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;

    let path = root.join("photo.jpg");
    write_test_raster_image(&path, ImageFormat::Jpeg, 600, 300);
    set_single_unmodified_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let key = StaticImageKey::from_request(&request);
    match app.prepared_static_image_for_overlay(&request) {
        crate::app::preview::static_images::StaticImageOverlayPreparation::Pending => {}
        _ => panic!("small jpeg should prepare in the background"),
    }
    assert!(app.preview.image.pending_prepares.contains(&key));
    assert_eq!(app.preview_overlay_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn current_large_jpeg_queues_background_prepare_when_ffmpeg_is_available() {
    if !crate::terminal_runtime::terminal_images::command_exists("ffmpeg") {
        return;
    }

    let root = temp_root("image-inline-large-jpeg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;

    let path = root.join("photo.jpg");
    write_test_raster_image(&path, ImageFormat::Jpeg, 3200, 1800);
    set_single_unmodified_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let key = StaticImageKey::from_request(&request);
    match app.prepared_static_image_for_overlay(&request) {
        crate::app::preview::static_images::StaticImageOverlayPreparation::Pending => {}
        _ => panic!("large jpeg should prepare in the background when ffmpeg is available"),
    }
    assert!(app.preview.image.pending_prepares.contains(&key));
    assert_eq!(app.preview_overlay_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
