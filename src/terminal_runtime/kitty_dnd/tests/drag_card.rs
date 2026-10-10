#[test]
#[cfg(not(target_os = "macos"))]
fn fontconfig_charset_query_uses_lowercase_hex_codepoint() {
    assert_eq!(
        super::platform_fonts::fontconfig_charset_query('󰉋'),
        ":charset=f024b"
    );
    assert_eq!(
        super::platform_fonts::fontconfig_charset_query('A'),
        ":charset=41"
    );
}

#[test]
#[cfg(not(target_os = "macos"))]
fn fontconfig_bold_family_query_requests_a_bold_face() {
    assert_eq!(
        super::platform_fonts::bold_family_query("Noto Sans Mono"),
        "Noto Sans Mono:style=Bold"
    );
}

#[test]
fn kitty_setting_value_reads_bold_font_without_matching_prefixes() {
    assert_eq!(
        super::kitty_setting_value("bold_font Noto Sans Mono", "bold_font"),
        Some("Noto Sans Mono")
    );
    assert_eq!(
        super::kitty_setting_value("bold_font Noto Sans Mono", "font"),
        None
    );
}

#[test]
fn drag_card_metrics_follow_terminal_cell_height() {
    assert_eq!(super::drag_card_metrics(Some(32.0)).height, 52);
    assert_eq!(super::drag_card_metrics(Some(32.0)).font_size, 30.0);

    assert_eq!(super::drag_card_metrics(Some(16.0)).height, 26);
    assert_eq!(super::drag_card_metrics(Some(16.0)).font_size, 15.0);

    assert_eq!(super::drag_card_metrics(Some(48.0)).height, 78);
    assert_eq!(super::drag_card_metrics(Some(48.0)).font_size, 45.0);

    assert_eq!(super::drag_card_metrics(Some(96.0)).height, 156);
    assert_eq!(super::drag_card_metrics(Some(96.0)).font_size, 90.0);
}

#[test]
fn drag_card_metrics_reject_invalid_cell_heights() {
    for cell_height in [
        None,
        Some(0.0),
        Some(-1.0),
        Some(f32::NAN),
        Some(f32::INFINITY),
        Some(f32::NEG_INFINITY),
        Some(7.0),
        Some(257.0),
    ] {
        let metrics = super::drag_card_metrics(cell_height);
        assert_eq!(metrics.height, 52);
        assert_eq!(metrics.font_size, 30.0);
        assert_eq!(metrics.icon_size, 33.0);
        assert_eq!(metrics.padding_x, 20);
    }
}

#[test]
fn drag_image_pixel_len_rejects_oversized_or_overflowing_buffers() {
    assert_eq!(super::drag_image_pixel_len(512, 512), Some(1_048_576));
    assert_eq!(super::drag_image_pixel_len(1025, 1024), None);
    assert_eq!(super::drag_image_pixel_len(u32::MAX, u32::MAX), None);
}
