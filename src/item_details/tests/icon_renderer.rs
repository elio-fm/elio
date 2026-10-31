use super::super::icon_renderer;
use ratatui::style::Color;

#[test]
fn renderer_rejects_invalid_canvas_dimensions() {
    for (width, height) in [(0, 32), (32, 0), (513, 32), (32, 513)] {
        assert!(
            icon_renderer::render_icon_png_for_cell_area("󰈔", Color::Blue, width, height).is_none(),
            "{width}×{height} canvas should be rejected"
        );
    }
}

#[test]
fn private_use_detection_covers_nerd_font_codepoints() {
    assert!(icon_renderer::is_private_use_character('\u{f014}'));
    assert!(icon_renderer::is_private_use_character('\u{f0001}'));
    assert!(!icon_renderer::is_private_use_character('A'));
    assert!(!icon_renderer::is_private_use_character('\u{10fffe}'));
}

#[test]
fn png_writer_preserves_icon_pixels() {
    let path = icon_renderer::write_png((
        2,
        1,
        vec![
            1, 2, 3, 4, // first pixel
            5, 6, 7, 8, // second pixel
        ],
    ))
    .expect("PNG writer should create an icon image");
    let image = image::ImageReader::open(&path)
        .expect("icon PNG should open")
        .decode()
        .expect("icon PNG should decode")
        .to_rgba8();

    assert_eq!(image.dimensions(), (2, 1));
    assert_eq!(image.get_pixel(0, 0).0, [1, 2, 3, 4]);
    assert_eq!(image.get_pixel(1, 0).0, [5, 6, 7, 8]);

    std::fs::remove_file(path).expect("test icon PNG should be removable");
}

#[test]
fn glyph_renderer_produces_a_decodable_png_when_a_font_is_available() {
    let Some(path) = icon_renderer::render_icon_png("󰈔", Color::Blue) else {
        return;
    };
    let image = image::ImageReader::open(&path)
        .expect("rendered icon PNG should open")
        .decode()
        .expect("rendered icon PNG should decode");

    assert!(image.width() > 0);
    assert!(image.height() > 0);

    std::fs::remove_file(path).expect("rendered icon PNG should be removable");
}
