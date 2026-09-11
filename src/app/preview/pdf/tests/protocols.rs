use super::helpers::*;
use ratatui::layout::Rect;
use std::fs;

#[test]
fn iterm_inline_protocol_uses_preencoded_payload_without_reading_source() {
    let output = String::from_utf8(
        crate::terminal_runtime::terminal_images::place_terminal_image(
            crate::terminal_runtime::terminal_images::ImageProtocol::ItermInline,
            std::path::Path::new("/definitely/missing.png"),
            Rect {
                x: 2,
                y: 3,
                width: 10,
                height: 4,
            },
            &[],
            Some("YWJj"),
            None,
        )
        .expect("preencoded iterm payload should not require source file"),
    )
    .expect("iterm payload should be utf8");

    assert!(output.contains("]1337;File=inline=1;"));
    assert!(output.contains("YWJj"));
}

#[test]
fn konsole_protocol_uses_kitty_graphics_sequence_for_pngs() {
    let root = temp_root("konsole-direct-placement");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("demo.png");
    write_test_raster_image(&path, ImageFormat::Png, 600, 300);

    let output = String::from_utf8(
        crate::terminal_runtime::terminal_images::place_terminal_image(
            crate::terminal_runtime::terminal_images::ImageProtocol::KittyDirectGraphics,
            &path,
            Rect {
                x: 2,
                y: 3,
                width: 10,
                height: 4,
            },
            &[],
            None,
            None,
        )
        .expect("Konsole direct placement should build"),
    )
    .expect("Konsole placement should be utf8");

    assert!(output.starts_with("\x1b[4;3H\x1b_G"));
    assert!(output.contains("a=T"));
    assert!(output.contains("q=2"));
    assert!(output.contains("c=10"));
    assert!(output.contains("r=4"));
    assert!(output.contains("C=1"));
    assert!(!output.contains("]1337;File=inline=1;"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iterm_static_image_requests_prepare_inline_payloads() {
    let (mut app, root) = build_selected_static_image_app("iterm-request", "demo.png");
    configure_iterm_image_support(&mut app);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("iterm static image request should exist");
    assert!(request.prepare_inline_payload);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iterm_full_pane_static_image_clear_area_excludes_preview_header_and_border() {
    let (mut app, root) = build_selected_static_image_app("iterm-clear-area", "demo.png");
    configure_iterm_image_support(&mut app);
    app.input.frame_state.preview_panel = Some(Rect {
        x: 1,
        y: 1,
        width: 50,
        height: 24,
    });
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.refresh_preview();

    wait_for_displayed_static_image_overlay(&mut app);

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
fn iterm_full_pane_static_image_erase_expands_to_body_bottom_edge() {
    let (mut app, root) = build_selected_static_image_app("iterm-erase-bottom-edge", "demo.png");
    configure_iterm_image_support(&mut app);
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
        height: 21,
    });
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.refresh_preview();

    wait_for_displayed_static_image_overlay(&mut app);
    app.queue_forced_iterm_preview_erase();

    let erase = String::from_utf8(app.iterm_pre_draw_erase())
        .expect("iTerm erase output should be valid utf8");
    assert!(erase.contains("\x1b[24;3H"));
    assert!(!erase.contains("\x1b[3;3H"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
