use super::super::inline_image::{TerminalWindowSize, fit_image_area, fit_image_pixels};
use super::{
    FittedPdfPlacement, PDF_RENDER_BUCKET_PX, PDF_RENDER_MIN_DIMENSION_PX, PdfPageDimensions,
};
use ratatui::layout::Rect;

pub(super) fn fit_pdf_page(
    area: Rect,
    window_size: TerminalWindowSize,
    page_dimensions: PdfPageDimensions,
) -> FittedPdfPlacement {
    let page_aspect = (page_dimensions.width_pts / page_dimensions.height_pts.max(f32::EPSILON))
        .max(f32::EPSILON);

    let (fit_width_px, fit_height_px) = fit_image_pixels(area, window_size, page_aspect);
    let (render_width_px, render_height_px) = bucket_render_dimensions(ensure_render_floor(
        fit_width_px.max(1.0),
        fit_height_px.max(1.0),
    ));

    FittedPdfPlacement {
        image_area: fit_image_area(area, window_size, page_aspect),
        render_width_px,
        render_height_px,
    }
}

pub(super) fn bucket_render_dimensions(dimensions: (u32, u32)) -> (u32, u32) {
    let (width_px, height_px) = dimensions;
    let longest = width_px.max(height_px).max(1);
    let bucketed_longest = longest.next_multiple_of(PDF_RENDER_BUCKET_PX);
    if bucketed_longest == longest {
        return (width_px, height_px);
    }

    let scale = bucketed_longest as f32 / longest as f32;
    (
        (width_px as f32 * scale).round().max(1.0) as u32,
        (height_px as f32 * scale).round().max(1.0) as u32,
    )
}

fn ensure_render_floor(width_px: f32, height_px: f32) -> (u32, u32) {
    let longest = width_px.max(height_px).max(1.0);
    if longest >= PDF_RENDER_MIN_DIMENSION_PX as f32 {
        return (width_px.round() as u32, height_px.round() as u32);
    }

    let scale = PDF_RENDER_MIN_DIMENSION_PX as f32 / longest;
    (
        (width_px * scale).round().max(1.0) as u32,
        (height_px * scale).round().max(1.0) as u32,
    )
}
