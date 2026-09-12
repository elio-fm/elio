mod cache;
mod prefetch;
mod present;
mod session;
mod types;

pub(in crate::app) use self::types::PdfPreviewState;
pub(in crate::app::preview::pdf) use self::types::{
    DisplayedPdfPreview, PdfDocumentKey, PdfOverlayRequest, PdfPageKey, PdfSession,
};
#[cfg(test)]
use super::super::*;
#[cfg(test)]
use crate::preview::documents::pdf::PdfProbeResult;
pub(in crate::app::preview::pdf) use crate::preview::documents::pdf::{
    FittedPdfPlacement, PdfPageDimensions, PdfRenderKey,
};
#[cfg(test)]
use crate::terminal_runtime::terminal_images::RenderedImageDimensions;
#[cfg(test)]
use ratatui::layout::Rect;
use std::time::Duration;
#[cfg(test)]
use std::{fs, path::PathBuf, time::Instant};

const PDF_RENDER_CACHE_LIMIT: usize = 12;
const PDF_PAGE_MIN: usize = 1;
const PDF_PAGE_STATUS_PREFIX: &str = "PDF page ";
const PDF_PROBE_PREFETCH_AHEAD_DISTANCE: usize = 2;
const PDF_PROBE_PREFETCH_BEHIND_DISTANCE: usize = 1;
const PDF_RENDER_PREFETCH_AHEAD_DISTANCE: usize = 2;
const PDF_RENDER_PREFETCH_BEHIND_DISTANCE: usize = 1;
const PDF_SELECTION_ACTIVATION_DELAY: Duration = Duration::from_millis(16);

#[cfg(test)]
mod tests;
