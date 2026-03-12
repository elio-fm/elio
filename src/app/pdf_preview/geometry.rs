use super::{
    FittedPdfPlacement, PDF_RENDER_BUCKET_PX, PDF_RENDER_MIN_DIMENSION_PX, PdfPageDimensions,
    TerminalWindowSize,
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

pub(super) fn fit_image_pixels(
    area: Rect,
    window_size: TerminalWindowSize,
    aspect_ratio: f32,
) -> (f32, f32) {
    let aspect_ratio = aspect_ratio.max(f32::EPSILON);
    let cell_width_px = window_size.pixels_width as f32 / f32::from(window_size.cells_width.max(1));
    let cell_height_px =
        window_size.pixels_height as f32 / f32::from(window_size.cells_height.max(1));
    let area_width_px = f32::from(area.width.max(1)) * cell_width_px;
    let area_height_px = f32::from(area.height.max(1)) * cell_height_px;

    if area_width_px / area_height_px > aspect_ratio {
        let height = area_height_px;
        (height * aspect_ratio, height)
    } else {
        let width = area_width_px;
        (width, width / aspect_ratio)
    }
}

pub(super) fn fit_image_area(
    area: Rect,
    window_size: TerminalWindowSize,
    aspect_ratio: f32,
) -> Rect {
    let cell_width_px = window_size.pixels_width as f32 / f32::from(window_size.cells_width.max(1));
    let cell_height_px =
        window_size.pixels_height as f32 / f32::from(window_size.cells_height.max(1));
    let (fit_width_px, fit_height_px) = fit_image_pixels(area, window_size, aspect_ratio);
    let width_cells = ((fit_width_px / cell_width_px).round() as u16).clamp(1, area.width.max(1));
    let height_cells =
        ((fit_height_px / cell_height_px).round() as u16).clamp(1, area.height.max(1));

    Rect {
        x: area.x + (area.width.saturating_sub(width_cells)) / 2,
        y: area.y + (area.height.saturating_sub(height_cells)) / 2,
        width: width_cells,
        height: height_cells,
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
