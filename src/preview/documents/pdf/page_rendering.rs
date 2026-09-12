use super::PdfPageDimensions;
use crate::terminal_runtime::terminal_images::{
    TerminalWindowSize, fit_image_area, fit_image_pixels,
};
use anyhow::{Context, Result};
use ratatui::layout::Rect;
use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

const PDF_RENDER_BUCKET_PX: u32 = 64;
const PDF_RENDER_MIN_DIMENSION_PX: u32 = 96;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PdfRenderKey {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FittedPdfPlacement {
    pub(crate) image_area: Rect,
    pub(crate) render_width_px: u32,
    pub(crate) render_height_px: u32,
}

pub(crate) fn fit_pdf_page(
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

pub(crate) fn render_pdf_page_to_cache(
    path: &Path,
    size: u64,
    modified: Option<SystemTime>,
    page: usize,
    width_px: u32,
    height_px: u32,
) -> Result<Option<PathBuf>> {
    let key = PdfRenderKey {
        path: path.to_path_buf(),
        size,
        modified,
        page,
        width_px,
        height_px,
    };
    let cache_dir = pdf_render_cache_dir()?;
    let stem = cache_dir.join(pdf_render_cache_stem(&key));
    let png_path = stem.with_extension("png");
    if png_path.exists() {
        return Ok(Some(png_path));
    }

    let status = Command::new("pdftocairo")
        .arg("-png")
        .arg("-singlefile")
        .arg("-f")
        .arg(page.to_string())
        .arg("-l")
        .arg(page.to_string())
        .arg("-scale-to-x")
        .arg(width_px.to_string())
        .arg("-scale-to-y")
        .arg("-1")
        .arg(path)
        .arg(&stem)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to start pdftocairo")?;

    if !status.success() || !png_path.exists() {
        return Ok(None);
    }
    Ok(Some(png_path))
}

fn bucket_render_dimensions(dimensions: (u32, u32)) -> (u32, u32) {
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

fn pdf_render_cache_dir() -> Result<PathBuf> {
    let cache_dir = env::temp_dir().join("elio-pdf-preview");
    fs::create_dir_all(&cache_dir).context("failed to create PDF preview cache")?;
    Ok(cache_dir)
}

fn pdf_render_cache_stem(key: &PdfRenderKey) -> String {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("page-{:016x}", hasher.finish())
}

#[cfg(test)]
#[path = "tests/page_rendering.rs"]
mod tests;
