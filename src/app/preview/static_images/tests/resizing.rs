use super::*;

#[test]
fn kitty_resize_requests_full_screen_clear_for_displayed_overlay() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("kitty-resize-clear", "demo.png");
    let request = ready_static_image_overlay(&mut app);
    app.preview.image.displayed = Some(DisplayedStaticImagePreview::from_request(
        &request,
        request.area,
        request.area,
    ));
    app.preview.image.displayed_excluded = vec![Rect {
        x: 4,
        y: 5,
        width: 6,
        height: 3,
    }];

    app.handle_terminal_image_resize();

    assert!(app.take_pending_resize_clear());
    assert!(!app.static_image_overlay_displayed());
    assert!(app.preview.image.displayed_excluded.is_empty());
    assert!(!app.take_pending_resize_clear());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iterm_resize_requests_full_screen_clear_for_displayed_overlay() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("iterm-resize-clear", "demo.png");
    let request = ready_static_image_overlay(&mut app);
    app.preview.terminal_images.protocol = ImageProtocol::ItermInline;
    app.preview.image.displayed = Some(DisplayedStaticImagePreview::from_request(
        &request,
        request.area,
        request.area,
    ));

    app.handle_terminal_image_resize();

    assert!(app.take_pending_resize_clear());
    assert!(!app.static_image_overlay_displayed());
    assert!(!app.take_pending_resize_clear());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn konsole_resize_does_not_request_full_screen_clear() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("konsole-resize-no-clear", "demo.png");
    let request = ready_static_image_overlay(&mut app);
    app.preview.terminal_images.protocol = ImageProtocol::KittyDirectGraphics;
    app.preview.image.displayed = Some(DisplayedStaticImagePreview::from_request(
        &request,
        request.area,
        request.area,
    ));

    app.handle_terminal_image_resize();

    assert!(!app.take_pending_resize_clear());
    assert!(app.static_image_overlay_displayed());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sixel_resize_requests_full_screen_clear_for_displayed_overlay() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("sixel-resize-clear", "demo.png");
    let request = ready_static_image_overlay(&mut app);
    app.preview.terminal_images.protocol = ImageProtocol::Sixel;
    app.preview.image.displayed = Some(DisplayedStaticImagePreview::from_request(
        &request,
        request.area,
        request.area,
    ));
    app.preview.image.displayed_excluded = vec![Rect {
        x: 2,
        y: 3,
        width: 4,
        height: 2,
    }];

    app.handle_terminal_image_resize();

    assert!(app.take_pending_resize_clear());
    assert!(!app.static_image_overlay_displayed());
    assert!(app.preview.image.displayed_excluded.is_empty());
    assert!(!app.take_pending_resize_clear());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
