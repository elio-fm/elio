use super::super::*;
use super::helpers::*;

#[test]
fn select_image_protocol_kitty_always_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::Kitty, false),
        ImageProtocol::KittyGraphics
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Kitty, true),
        ImageProtocol::KittyGraphics
    );
}

#[test]
fn select_image_protocol_ghostty_always_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::Ghostty, false),
        ImageProtocol::KittyGraphics
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Ghostty, true),
        ImageProtocol::KittyGraphics
    );
}

#[test]
fn select_image_protocol_wezterm_always_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::WezTerm, false),
        ImageProtocol::ItermInline
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::WezTerm, true),
        ImageProtocol::ItermInline
    );
}

#[test]
fn select_image_protocol_warp_always_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::Warp, false),
        ImageProtocol::KittyGraphics
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Warp, true),
        ImageProtocol::KittyGraphics
    );
}

#[test]
fn select_image_protocol_alacritty_disabled_and_other_override_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::Alacritty, true),
        ImageProtocol::None
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Other, false),
        ImageProtocol::None
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Other, true),
        ImageProtocol::KittyGraphics
    );
}

#[test]
fn fallback_window_size_pixels_uses_reasonable_cell_defaults() {
    assert_eq!(fallback_window_size_pixels(100, 40), (800, 640));
    assert_eq!(fallback_window_size_pixels(0, 0), (8, 16));
}

#[cfg(unix)]
#[test]
fn command_exists_checks_direct_executable_paths_without_shelling_out() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("command-exists-direct-path");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let executable = root.join("demo-tool");
    fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("failed to write test executable");

    let mut permissions = fs::metadata(&executable)
        .expect("test executable metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("failed to mark test executable");

    assert!(command_exists(
        executable.to_str().expect("path should be valid utf-8")
    ));

    let not_executable = root.join("demo-data");
    fs::write(&not_executable, b"plain data").expect("failed to write plain file");
    assert!(!command_exists(
        not_executable.to_str().expect("path should be valid utf-8")
    ));
}

#[test]
fn build_kitty_upload_sequence_uses_unicode_placeholder_mode() {
    let root = temp_root("kitty-upload-sequence");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("demo.pdf-preview.png");
    write_test_raster_image(&path, ImageFormat::Png, 24, 16);
    let payload = fs::read(&path).expect("png payload should exist");
    let id = 42_u32;
    let area = Rect {
        x: 10,
        y: 4,
        width: 30,
        height: 20,
    };

    let sequence = String::from_utf8(
        build_kitty_upload_sequence(&path, id, area).expect("kitty upload sequence should build"),
    )
    .expect("kitty upload sequence should be utf8");

    assert!(sequence.starts_with("\u{1b}_G"));
    assert!(sequence.contains("a=T"));
    assert!(sequence.contains("q=2"));
    assert!(sequence.contains("U=1"));
    assert!(sequence.contains(&format!("i={id}")));
    assert!(sequence.contains("p=1"));
    assert!(sequence.contains("c=30"));
    assert!(sequence.contains("r=20"));
    assert!(sequence.contains("C=1"));
    assert!(sequence.contains("m=0"));
    assert!(!sequence.contains("t=f"));
    assert!(sequence.contains(&BASE64_STANDARD.encode(payload)));
    assert!(sequence.ends_with("\u{1b}\\"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn kitty_placeholder_sequence_sets_panel_background_for_transparency() {
    let sequence = String::from_utf8(build_kitty_placeholder_sequence(
        42,
        Rect {
            x: 1,
            y: 2,
            width: 2,
            height: 2,
        },
        &[],
    ))
    .expect("placeholder sequence should be utf8");

    assert!(sequence.contains("[38;2;"));
    assert!(sequence.contains(";48;2;"));
    assert!(sequence.contains(";58;2;0;0;1m"));
}

#[test]
fn iterm_png_and_jpeg_static_images_use_direct_source_payloads() {
    for (file_name, format) in [
        ("direct.png", ImageFormat::Png),
        ("direct.jpg", ImageFormat::Jpeg),
    ] {
        let root = temp_root("iterm-direct-static-image");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join(file_name);
        write_test_raster_image(&path, format, 600, 300);
        let metadata = fs::metadata(&path).expect("image metadata should exist");

        let prepared = crate::app::overlays::images::prepare_static_image_asset(
            &jobs::ImagePrepareRequest {
                path: path.clone(),
                size: metadata.len(),
                modified: None,
                target_width_px: 768,
                target_height_px: 540,
                ffmpeg_available: true,
                resvg_available: false,
                magick_available: true,
                force_render_to_cache: false,
                prepare_inline_payload: true,
            },
            || false,
        )
        .expect("iterm direct static image should prepare successfully");

        assert_eq!(prepared.display_path, path);
        assert_eq!(
            prepared.dimensions,
            RenderedImageDimensions {
                width_px: 600,
                height_px: 300,
            }
        );
        assert!(prepared.inline_payload.is_some());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn iterm_inline_protocol_uses_preencoded_payload_without_reading_source() {
    let output = String::from_utf8(
        crate::app::overlays::inline_image::place_terminal_image(
            ImageProtocol::ItermInline,
            Path::new("/definitely/missing.png"),
            Rect {
                x: 2,
                y: 3,
                width: 10,
                height: 4,
            },
            &[],
            Some("YWJj"),
        )
        .expect("preencoded iterm payload should not require source file"),
    )
    .expect("iterm payload should be utf8");

    assert!(output.contains("]1337;File=inline=1;"));
    assert!(output.contains("YWJj"));
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
fn build_kitty_clear_sequence_deletes_visible_images() {
    assert_eq!(build_kitty_clear_sequence(), "\u{1b}_Ga=d,d=A,q=2\u{1b}\\");
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
