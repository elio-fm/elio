use super::{RenderedImageDimensions, TerminalWindowSize};
use ratatui::layout::Rect;
use std::{fs::File, io::Read, path::Path};

pub(in crate::app) fn read_png_dimensions(path: &Path) -> Option<RenderedImageDimensions> {
    let mut file = File::open(path).ok()?;
    let mut header = [0_u8; 24];
    file.read_exact(&mut header).ok()?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        return None;
    }

    let width_px = u32::from_be_bytes(header[16..20].try_into().ok()?);
    let height_px = u32::from_be_bytes(header[20..24].try_into().ok()?);
    (width_px > 0 && height_px > 0).then_some(RenderedImageDimensions {
        width_px,
        height_px,
    })
}

pub(in crate::app) fn fit_image_pixels(
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

pub(in crate::app) fn fit_image_area(
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

pub(super) fn push_unique_rect(rects: &mut Vec<Rect>, area: Rect) {
    if area.width == 0 || area.height == 0 || rects.contains(&area) {
        return;
    }
    rects.push(area);
}

pub(super) fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = a.x.saturating_add(a.width).min(b.x.saturating_add(b.width));
    let bottom =
        a.y.saturating_add(a.height)
            .min(b.y.saturating_add(b.height));
    (right > left && bottom > top).then_some(Rect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    })
}
