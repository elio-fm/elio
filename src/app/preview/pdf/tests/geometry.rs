use super::super::*;
use super::helpers::*;

#[test]
fn parse_pdfinfo_page_count_reads_page_field() {
    assert_eq!(
        parse_pdfinfo_page_count("Title: demo\nPages: 18\nProducer: test\n"),
        Some(18)
    );
}

#[test]
fn parse_pdfinfo_page_dimensions_reads_global_and_per_page_sizes() {
    assert_eq!(
        parse_pdfinfo_page_dimensions("Page size: 595.276 x 841.89 pts (A4)\n"),
        Some(PdfPageDimensions {
            width_pts: 595.276,
            height_pts: 841.89,
        })
    );
    assert_eq!(
        parse_pdfinfo_page_dimensions("Page    2 size: 300 x 144 pts\n"),
        Some(PdfPageDimensions {
            width_pts: 300.0,
            height_pts: 144.0,
        })
    );
}

#[test]
fn read_png_dimensions_reads_ihdr_size() {
    let root = temp_root("png-dimensions");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("page.png");
    write_test_png(&path, 600, 300);

    assert_eq!(
        read_png_dimensions(&path),
        Some(RenderedImageDimensions {
            width_px: 600,
            height_px: 300,
        })
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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

#[test]
fn fit_image_area_preserves_actual_rendered_png_aspect_ratio() {
    let area = fit_image_area(
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
        0.25,
    );

    assert_eq!(area.width, 10);
    assert_eq!(area.height, 20);
    assert_eq!(area.x, 20);
    assert_eq!(area.y, 4);
}
