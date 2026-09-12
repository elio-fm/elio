use crate::app::Entry;
use crate::preview::documents::pdf::{FittedPdfPlacement, PdfPageDimensions, PdfRenderKey};
use crate::terminal_runtime::terminal_images::RenderedImageDimensions;
use ratatui::layout::Rect;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    time::{Instant, SystemTime},
};

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct PdfPreviewState {
    pub(in crate::app::preview) pdf_tools_available: bool,
    pub(super) session: Option<PdfSession>,
    pub(super) document_page_counts: HashMap<PdfDocumentKey, usize>,
    pub(super) page_dimensions: HashMap<PdfPageKey, PdfPageDimensions>,
    pub(super) pending_page_probes: HashSet<PdfPageKey>,
    pub(super) failed_page_probes: HashSet<PdfPageKey>,
    pub(super) rendered_pages: HashMap<PdfRenderKey, PathBuf>,
    pub(super) rendered_page_dimensions: HashMap<PdfRenderKey, RenderedImageDimensions>,
    pub(super) render_order: VecDeque<PdfRenderKey>,
    pub(super) pending_renders: HashSet<PdfRenderKey>,
    pub(super) failed_renders: HashSet<PdfRenderKey>,
    pub(super) displayed: Option<DisplayedPdfPreview>,
    pub(super) displayed_excluded: Vec<Rect>,
    pub(super) activation_ready_at: Option<Instant>,
    pub(super) last_navigation_direction: isize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PdfSession {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) current_page: usize,
    pub(super) total_pages: Option<usize>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct PdfDocumentKey {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct PdfPageKey {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) page: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DisplayedPdfPreview {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) page: usize,
    pub(super) area: Rect,
    pub(super) render_width_px: u32,
    pub(super) render_height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PdfOverlayRequest {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) page: usize,
    pub(super) area: Rect,
}

impl PdfOverlayRequest {
    pub(super) fn render_key(&self, placement: FittedPdfPlacement) -> PdfRenderKey {
        PdfRenderKey {
            path: self.path.clone(),
            size: self.size,
            modified: self.modified,
            page: self.page,
            width_px: placement.render_width_px,
            height_px: placement.render_height_px,
        }
    }
}

impl PdfPageKey {
    pub(super) fn from_request(request: &PdfOverlayRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
        }
    }
}

impl DisplayedPdfPreview {
    pub(super) fn from_request(request: &PdfOverlayRequest, placement: FittedPdfPlacement) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
            area: request.area,
            render_width_px: placement.render_width_px,
            render_height_px: placement.render_height_px,
        }
    }
}

impl PdfDocumentKey {
    pub(super) fn from_entry(entry: &Entry) -> Self {
        Self {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
        }
    }

    pub(super) fn from_page_key(key: &PdfPageKey) -> Self {
        Self {
            path: key.path.clone(),
            size: key.size,
            modified: key.modified,
        }
    }

    pub(super) fn from_session(session: &PdfSession) -> Self {
        Self {
            path: session.path.clone(),
            size: session.size,
            modified: session.modified,
        }
    }
}
