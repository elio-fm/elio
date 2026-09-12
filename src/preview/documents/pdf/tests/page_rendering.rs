use super::*;

#[test]
fn bucket_render_dimensions_rounds_up_longest_edge_without_distortion() {
    assert_eq!(bucket_render_dimensions((512, 768)), (512, 768));
    assert_eq!(bucket_render_dimensions((530, 742)), (549, 768));
}

#[test]
fn fit_pdf_page_preserves_aspect_ratio_for_wide_pages() {
    let placement = fit_pdf_page(
        Rect {
            x: 10,
            y: 4,
            width: 30,
            height: 20,
        },
        TerminalWindowSize {
            cells_width: 100,
            cells_height: 50,
            pixels_width: 1000,
            pixels_height: 1000,
        },
        PdfPageDimensions {
            width_pts: 300.0,
            height_pts: 144.0,
        },
    );

    assert!(placement.image_area.width <= 30);
    assert!(placement.image_area.height <= 20);
    assert_eq!(placement.image_area.height, 7);
    assert_eq!(placement.image_area.y, 10);
    assert!(placement.render_width_px > placement.render_height_px);
}
