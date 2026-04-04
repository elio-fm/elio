mod cache;
mod geometry;
mod pipeline;
mod prefetch;
mod present;
mod session;
mod types;

pub(crate) use self::pipeline::{probe_pdf_page, render_pdf_page_to_cache};
pub(in crate::app::overlays::pdf) use self::types::{
    DisplayedPdfPreview, FittedPdfPlacement, PdfDocumentKey, PdfOverlayRequest, PdfPageDimensions,
    PdfPageKey, PdfRenderKey, PdfSession,
};
pub(in crate::app) use self::types::{PdfPreviewState, PdfProbeResult};
#[cfg(test)]
use self::{
    geometry::{bucket_render_dimensions, fit_pdf_page},
    pipeline::{parse_pdfinfo_page_count, parse_pdfinfo_page_dimensions},
};
#[cfg(test)]
use super::super::*;
#[cfg(test)]
use super::inline_image::{RenderedImageDimensions, fit_image_area, read_png_dimensions};
#[cfg(test)]
use ratatui::layout::Rect;
use std::time::Duration;
#[cfg(test)]
use std::{fs, path::PathBuf, time::Instant};

const PDF_RENDER_CACHE_LIMIT: usize = 12;
const PDF_RENDER_BUCKET_PX: u32 = 64;
const PDF_RENDER_MIN_DIMENSION_PX: u32 = 96;
const PDF_PAGE_MIN: usize = 1;
const PDF_PAGE_STATUS_PREFIX: &str = "PDF page ";
const PDF_PROBE_PREFETCH_AHEAD_DISTANCE: usize = 2;
const PDF_PROBE_PREFETCH_BEHIND_DISTANCE: usize = 1;
const PDF_RENDER_PREFETCH_AHEAD_DISTANCE: usize = 2;
const PDF_RENDER_PREFETCH_BEHIND_DISTANCE: usize = 1;
const PDF_SELECTION_ACTIVATION_DELAY: Duration = Duration::from_millis(16);

#[cfg(test)]
mod tests;
