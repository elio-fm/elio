use super::*;

#[test]
fn exclusion_only_updates_redraw_without_clearing_the_existing_image() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("excluded-redraw", "demo.png");
    app.preview.image.selection_activation_delay = Duration::ZERO;
    app.sync_image_preview_selection_activation();

    let mut initial = Vec::new();
    app.present_static_image_overlay(ImageProtocol::KittyGraphics, &[], false, &mut initial)
        .expect("initial static image presentation should succeed");

    let excluded = [Rect {
        x: 4,
        y: 5,
        width: 6,
        height: 3,
    }];
    let mut updated = Vec::new();
    let state = app
        .present_static_image_overlay(ImageProtocol::KittyGraphics, &excluded, false, &mut updated)
        .expect("excluded-only redraw should succeed");
    let output = String::from_utf8(updated).expect("kitty redraw should be utf8");

    assert_eq!(state, OverlayPresentState::Displayed);
    assert!(
        !output.is_empty(),
        "changed exclusions should trigger a redraw"
    );
    assert!(
        !output.contains("\u{1b}_Ga=d,d=A,q=2\u{1b}\\"),
        "exclusion-only redraw should not clear the previous image first"
    );
    assert_eq!(app.preview.image.displayed_excluded, excluded);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn open_with_overlay_updates_kitty_exclusions_and_closing_it_restores_them() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("open-with-exclusions", "demo.png");
    app.preview.image.selection_activation_delay = Duration::ZERO;
    app.sync_image_preview_selection_activation();

    let mut initial = Vec::new();
    app.present_static_image_overlay(ImageProtocol::KittyGraphics, &[], false, &mut initial)
        .expect("initial static image presentation should succeed");
    assert!(app.preview.image.displayed_excluded.is_empty());

    app.inject_open_with_for_test("Preview", "/usr/bin/true", vec![], false);
    let popup = Rect {
        x: 4,
        y: 5,
        width: 12,
        height: 4,
    };
    let erase = app.modal_image_post_draw_erase(&[popup], &blank_frame_buffer());
    assert!(
        !erase.is_empty(),
        "opening a transparent popup over a Kitty placeholder image should erase covered cells"
    );
    app.input.screen_regions.open_with_panel = Some(popup);

    let with_popup = String::from_utf8(
        app.present_preview_overlay()
            .expect("open-with popup redraw should succeed"),
    )
    .expect("kitty redraw should be valid utf8");
    assert!(
        !with_popup.is_empty(),
        "opening the open-with popup should redraw the kitty image"
    );
    assert_eq!(app.preview.image.displayed_excluded, vec![popup]);

    app.overlays.open_with = None;
    app.input.screen_regions.open_with_panel = None;

    let restored = String::from_utf8(
        app.present_preview_overlay()
            .expect("closing the open-with popup should redraw the kitty image"),
    )
    .expect("kitty redraw should be valid utf8");
    assert!(
        !restored.is_empty(),
        "closing the open-with popup should redraw the kitty image"
    );
    assert!(
        app.preview.image.displayed_excluded.is_empty(),
        "closing the popup should remove kitty exclusions"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sixel_popup_skips_post_draw_masking_for_foot_performance() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("sixel-popup-mask", "demo.png");
    let request = displayed_sixel_static_image_overlay(&mut app, TerminalIdentity::Foot);
    assert!(app.static_image_overlay_displayed());

    app.inject_open_with_for_test("Preview", "/usr/bin/true", vec![], false);
    let popup = Rect {
        x: request.area.x.saturating_add(1),
        y: request.area.y.saturating_add(1),
        width: request.area.width.saturating_sub(2).max(1),
        height: request.area.height.saturating_sub(2).max(1),
    };
    let erase = app.modal_image_post_draw_erase(&[popup], &blank_frame_buffer());
    assert!(
        erase.is_empty(),
        "Sixel should not use post-draw modal masking because Foot processes those erases slowly"
    );

    let out = app
        .present_preview_overlay()
        .expect("Sixel popup redraw should not fail");
    assert!(
        out.is_empty(),
        "Sixel should not repaint raster image bytes while the popup is open"
    );
    assert!(app.static_image_overlay_displayed());

    app.overlays.open_with = None;
    app.input.screen_regions.open_with_panel = None;

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut restored = Vec::new();
    while Instant::now() < deadline {
        let _ = app.process_background_jobs();
        restored = app
            .present_preview_overlay()
            .expect("closing the popup should repaint the Sixel image");
        if !restored.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !restored.is_empty(),
        "closing the popup should repaint the masked Sixel image"
    );
    assert!(app.static_image_overlay_displayed());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn foot_sixel_popup_erases_collision_and_repaints_after_close() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("foot-sixel-popup-collision", "demo.png");
    let request = displayed_sixel_static_image_overlay(&mut app, TerminalIdentity::Foot);
    assert!(app.static_image_overlay_displayed());

    app.inject_open_with_for_test("Preview", "/usr/bin/true", vec![], false);
    let popup = Rect {
        x: request.area.x.saturating_add(1),
        y: request.area.y.saturating_add(1),
        width: request.area.width.saturating_sub(2).max(1),
        height: request.area.height.saturating_sub(2).max(1),
    };
    let mask = app.modal_image_post_draw_erase(&[popup], &blank_frame_buffer());
    assert!(
        mask.is_empty(),
        "Foot Sixel should avoid fragile transparent-cell post-draw masking"
    );

    let (collisions, erase) = app.sixel_modal_collision_erase(&[popup]);
    assert_eq!(collisions, vec![popup]);
    assert!(
        !erase.is_empty(),
        "Foot Sixel should erase the popup/image collision behind modal overlays"
    );
    assert!(
        app.static_image_overlay_displayed(),
        "the image remains logically displayed so only the popup intersection is punched out"
    );
    assert!(
        app.present_preview_overlay()
            .expect("Sixel popup redraw should not fail")
            .is_empty(),
        "Foot Sixel should not repaint image bytes over an open popup"
    );

    app.overlays.open_with = None;
    app.input.screen_regions.open_with_panel = None;

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut restored = Vec::new();
    while Instant::now() < deadline {
        let _ = app.process_background_jobs();
        let _ = app.process_image_preview_timers();
        restored = app
            .present_preview_overlay()
            .expect("closing the popup should repaint the Sixel image");
        if !restored.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !restored.is_empty(),
        "closing the popup should repaint the erased Foot Sixel collision"
    );
    assert!(app.static_image_overlay_displayed());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn windows_terminal_sixel_popup_erases_collision_and_repaints_after_close() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("wt-sixel-popup-collision", "demo.png");
    let request = displayed_sixel_static_image_overlay(&mut app, TerminalIdentity::WindowsTerminal);
    assert!(app.static_image_overlay_displayed());

    app.inject_open_with_for_test("Preview", "/usr/bin/true", vec![], false);
    let popup = Rect {
        x: request.area.x.saturating_add(1),
        y: request.area.y.saturating_add(1),
        width: request.area.width.saturating_sub(2).max(1),
        height: request.area.height.saturating_sub(2).max(1),
    };
    let mask = app.modal_image_post_draw_erase(&[popup], &blank_frame_buffer());
    assert!(
        mask.is_empty(),
        "Windows Terminal Sixel should avoid fragile transparent-cell post-draw masking"
    );

    let (collisions, erase) = app.sixel_modal_collision_erase(&[popup]);
    assert_eq!(collisions, vec![popup]);
    assert!(
        !erase.is_empty(),
        "Windows Terminal Sixel should erase the popup/image collision behind modal overlays"
    );
    assert!(
        app.static_image_overlay_displayed(),
        "the image remains logically displayed so only the popup intersection is punched out"
    );
    assert!(
        app.present_preview_overlay()
            .expect("Windows Terminal Sixel popup redraw should not fail")
            .is_empty(),
        "Windows Terminal Sixel should not repaint image bytes over an open popup"
    );

    app.overlays.open_with = None;
    app.input.screen_regions.open_with_panel = None;

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut restored = Vec::new();
    while Instant::now() < deadline {
        let _ = app.process_background_jobs();
        let _ = app.process_image_preview_timers();
        restored = app
            .present_preview_overlay()
            .expect("closing the popup should repaint the Sixel image");
        if !restored.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !restored.is_empty(),
        "closing the popup should repaint the erased Windows Terminal Sixel collision"
    );
    assert!(app.static_image_overlay_displayed());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn foot_sixel_focus_gain_with_popup_does_not_clear_displayed_image_without_resize() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("foot-sixel-focus-popup", "demo.png");
    let request = ready_static_image_overlay(&mut app);
    app.set_terminal_image_protocol_for_tests(ImageProtocol::Sixel, TerminalIdentity::Foot);
    app.handle_terminal_image_focus_gained();
    assert!(!app.take_pending_resize_clear());

    app.preview.image.displayed = Some(DisplayedStaticImagePreview::from_request(
        &request,
        request.area,
        request.area,
    ));
    app.inject_open_with_for_test("Preview", "/usr/bin/true", vec![], false);

    app.handle_terminal_image_focus_gained();
    assert!(
        !app.take_pending_resize_clear(),
        "focus gained without a terminal-size change must not take the resize-clear path"
    );
    assert!(
        app.static_image_overlay_displayed(),
        "Foot Sixel image state should survive focus refocus while a popup is open"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn foot_sixel_resize_under_popup_repaints_image_behind_modal() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("foot-sixel-resize-popup", "demo.png");
    let request = ready_static_image_overlay(&mut app);
    app.set_terminal_image_protocol_for_tests(ImageProtocol::Sixel, TerminalIdentity::Foot);
    app.preview.image.displayed = Some(DisplayedStaticImagePreview::from_request(
        &request,
        request.area,
        request.area,
    ));
    app.inject_open_with_for_test("Preview", "/usr/bin/true", vec![], false);
    let popup = Rect {
        x: request.area.x.saturating_add(1),
        y: request.area.y.saturating_add(1),
        width: request.area.width.saturating_sub(2).max(1),
        height: request.area.height.saturating_sub(2).max(1),
    };

    app.handle_terminal_image_resize();
    app.preview.terminal_images.window = Some(TerminalWindowSize {
        cells_width: 120,
        cells_height: 40,
        pixels_width: 1920,
        pixels_height: 1080,
    });
    assert!(app.take_pending_resize_clear());
    assert!(
        !app.static_image_overlay_displayed(),
        "resize clear removes stale Sixel geometry before redraw"
    );
    assert!(
        app.should_repaint_sixel_under_modal(&[popup]),
        "Foot Sixel should repaint the resized image behind an open popup"
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut image_bytes = Vec::new();
    while Instant::now() < deadline {
        let _ = app.process_background_jobs();
        let _ = app.process_image_preview_timers();
        image_bytes = app
            .present_preview_overlay_behind_modal()
            .expect("resize-under-popup repaint should not fail");
        if !image_bytes.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !image_bytes.is_empty(),
        "resize-under-popup should emit fresh Sixel bytes before collision masking"
    );
    assert!(app.static_image_overlay_displayed());

    let (collisions, erase) = app.sixel_modal_collision_erase(&[popup]);
    assert_eq!(collisions, vec![popup]);
    assert!(
        !erase.is_empty(),
        "the freshly repainted image should still be punched out under the popup"
    );
    assert!(
        app.present_preview_overlay()
            .expect("normal popup redraw should not fail")
            .is_empty(),
        "normal Foot Sixel presentation remains blocked while the popup is open"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn open_with_overlay_clears_konsole_image_and_closing_it_redraws_it() {
    let (mut app, root, _image_path) =
        build_selected_static_image_app("konsole-open-with-clear", "demo.png");
    app.preview.terminal_images.protocol = ImageProtocol::KittyDirectGraphics;
    app.preview.image.selection_activation_delay = Duration::ZERO;
    app.sync_image_preview_selection_activation();

    let mut initial = Vec::new();
    app.present_static_image_overlay(ImageProtocol::KittyDirectGraphics, &[], false, &mut initial)
        .expect("initial Konsole image presentation should succeed");
    assert!(app.static_image_overlay_displayed());

    app.inject_open_with_for_test("Preview", "/usr/bin/true", vec![], false);
    app.input.screen_regions.open_with_panel = Some(Rect {
        x: 4,
        y: 5,
        width: 12,
        height: 4,
    });

    let cleared = String::from_utf8(
        app.present_preview_overlay()
            .expect("opening the open-with overlay should clear the Konsole image"),
    )
    .expect("Konsole clear output should be valid utf8");
    assert!(
        cleared.contains("\u{1b}_Ga=d,d=I,"),
        "opening the popup should send a Konsole delete command"
    );
    assert!(
        !app.static_image_overlay_displayed(),
        "opening the popup should clear the tracked Konsole image"
    );

    app.overlays.open_with = None;
    app.input.screen_regions.open_with_panel = None;

    let restored = String::from_utf8(
        app.present_preview_overlay()
            .expect("closing the open-with overlay should redraw the Konsole image"),
    )
    .expect("Konsole redraw output should be valid utf8");
    assert!(
        restored.contains("\u{1b}_Ga=T,"),
        "closing the popup should redraw the Konsole image"
    );
    assert!(
        app.static_image_overlay_displayed(),
        "closing the popup should restore the tracked Konsole image"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
