use super::{
    PreviewContent, PreviewRequestOptions,
    documents::pdf::{FittedPdfPlacement, PdfPageDimensions, PdfRenderKey},
    images::{SixelDcsKey, StaticImageKey},
};
use crate::{
    fs::Entry,
    terminal_runtime::terminal_images::{
        ImageProtocol, RenderedImageDimensions, TerminalIdentity, TerminalWindowSize,
    },
};
use ratatui::layout::Rect;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, Debug)]
pub(crate) struct CachedPreview {
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) preview: PreviewContent,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreviewCacheKey {
    pub(crate) path: PathBuf,
    pub(crate) variant: PreviewRequestOptions,
    pub(crate) ffmpeg_available: bool,
    pub(crate) code_line_limit: usize,
    /// The render limit used for this cache entry. Partial (incremental)
    /// renders have `code_render_limit < code_line_limit`; complete renders
    /// have `code_render_limit == code_line_limit`.
    pub(crate) code_render_limit: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreviewLineCountKey {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PreviewMetricsSnapshot {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub applied_results: u64,
    pub stale_results_dropped: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PreviewMetrics {
    pub(crate) cache_hits: u64,
    pub(crate) cache_misses: u64,
    pub(crate) applied_results: u64,
    pub(crate) stale_results_dropped: u64,
}

impl PreviewMetrics {
    #[cfg(test)]
    pub(crate) fn snapshot(self) -> PreviewMetricsSnapshot {
        PreviewMetricsSnapshot {
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            applied_results: self.applied_results,
            stale_results_dropped: self.stale_results_dropped,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MediaPreviewState {
    pub(crate) ffprobe_available: Option<bool>,
    pub(crate) ffmpeg_available: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreviewLoadState {
    Placeholder(PathBuf),
    Refreshing(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreviewDirectoryStatsState {
    Loading {
        token: u64,
        path: PathBuf,
    },
    Complete {
        token: u64,
        path: PathBuf,
        stats: crate::fs::DirectoryStats,
    },
    Incomplete {
        token: u64,
        path: PathBuf,
        partial: crate::fs::DirectoryStats,
        error: String,
    },
}

impl PreviewDirectoryStatsState {
    pub(crate) fn token(&self) -> u64 {
        match self {
            Self::Loading { token, .. }
            | Self::Complete { token, .. }
            | Self::Incomplete { token, .. } => *token,
        }
    }

    pub(crate) fn path(&self) -> &PathBuf {
        match self {
            Self::Loading { path, .. }
            | Self::Complete { path, .. }
            | Self::Incomplete { path, .. } => path,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewRefreshMode {
    Immediate,
    Deferred,
}

pub(crate) struct PreviewState {
    pub(crate) scroll: usize,
    pub(crate) horizontal_scroll: usize,
    pub(crate) content: PreviewContent,
    pub(crate) token: u64,
    pub(crate) metrics: PreviewMetrics,
    pub(crate) load_state: Option<PreviewLoadState>,
    pub(crate) directory_stats: Option<PreviewDirectoryStatsState>,
    pub(crate) directory_stats_ready_at: Option<Instant>,
    pub(crate) deferred_refresh_at: Option<Instant>,
    pub(crate) prefetch_ready_at: Option<Instant>,
    pub(crate) result_cache: HashMap<PreviewCacheKey, CachedPreview>,
    pub(crate) result_order: VecDeque<PreviewCacheKey>,
    pub(crate) line_count_cache: HashMap<PreviewLineCountKey, usize>,
    pub(crate) line_count_order: VecDeque<PreviewLineCountKey>,
    pub(crate) pending_line_counts: HashSet<PreviewLineCountKey>,
    /// True while an incremental extension job is outstanding for the current
    /// selection. Prevents duplicate extension submissions.
    pub(crate) incremental_render_in_flight: bool,
    /// The path of the entry that triggered the in-flight extension job.
    /// Used to clear `incremental_render_in_flight` when a stale result drops.
    pub(crate) incremental_render_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ComicPreviewState {
    pub(crate) session: Option<ComicSession>,
    /// Path of the comic file whose page image is currently displayed in the
    /// inline overlay. Set when a page image is rendered so we can decide
    /// whether to keep or clear the stale overlay when the selection changes.
    pub(crate) displayed_page_source: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ComicSession {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) current_page: usize,
    pub(crate) total_pages: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EpubPreviewState {
    pub(crate) session: Option<EpubSession>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EpubSession {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) current_section: usize,
    pub(crate) total_sections: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PdfPreviewState {
    pub(crate) pdf_tools_available: bool,
    pub(crate) session: Option<PdfSession>,
    pub(crate) document_page_counts: HashMap<PdfDocumentKey, usize>,
    pub(crate) page_dimensions: HashMap<PdfPageKey, PdfPageDimensions>,
    pub(crate) pending_page_probes: HashSet<PdfPageKey>,
    pub(crate) failed_page_probes: HashSet<PdfPageKey>,
    pub(crate) rendered_pages: HashMap<PdfRenderKey, PathBuf>,
    pub(crate) rendered_page_dimensions: HashMap<PdfRenderKey, RenderedImageDimensions>,
    pub(crate) render_order: VecDeque<PdfRenderKey>,
    pub(crate) pending_renders: HashSet<PdfRenderKey>,
    pub(crate) failed_renders: HashSet<PdfRenderKey>,
    pub(crate) displayed: Option<DisplayedPdfPreview>,
    pub(crate) displayed_excluded: Vec<Rect>,
    pub(crate) activation_ready_at: Option<Instant>,
    pub(crate) last_navigation_direction: isize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PdfSession {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) current_page: usize,
    pub(crate) total_pages: Option<usize>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PdfDocumentKey {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PdfPageKey {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DisplayedPdfPreview {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
    pub(crate) area: Rect,
    pub(crate) render_width_px: u32,
    pub(crate) render_height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PdfOverlayRequest {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
    pub(crate) area: Rect,
}

impl PdfOverlayRequest {
    pub(crate) fn render_key(&self, placement: FittedPdfPlacement) -> PdfRenderKey {
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
    pub(crate) fn from_request(request: &PdfOverlayRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
        }
    }
}

impl DisplayedPdfPreview {
    pub(crate) fn from_request(request: &PdfOverlayRequest, placement: FittedPdfPlacement) -> Self {
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
    pub(crate) fn from_entry(entry: &Entry) -> Self {
        Self {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
        }
    }

    pub(crate) fn from_page_key(key: &PdfPageKey) -> Self {
        Self {
            path: key.path.clone(),
            size: key.size,
            modified: key.modified,
        }
    }

    pub(crate) fn from_session(session: &PdfSession) -> Self {
        Self {
            path: session.path.clone(),
            size: session.size,
            modified: session.modified,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ImagePreviewState {
    pub(crate) dimensions: HashMap<StaticImageKey, RenderedImageDimensions>,
    pub(crate) rendered_images: HashMap<StaticImageKey, PathBuf>,
    pub(crate) render_order: VecDeque<StaticImageKey>,
    pub(crate) inline_payloads: HashMap<StaticImageKey, Arc<str>>,
    pub(crate) payload_order: VecDeque<StaticImageKey>,
    /// Cached Sixel DCS byte streams keyed by display path + placement area +
    /// window dimensions. Entries are evicted LRU-style once the cache exceeds
    /// `SIXEL_DCS_CACHE_LIMIT`.
    pub(crate) sixel_dcs_payloads: HashMap<SixelDcsKey, Arc<[u8]>>,
    pub(crate) sixel_dcs_order: VecDeque<SixelDcsKey>,
    pub(crate) failed_images: HashSet<StaticImageKey>,
    pub(crate) pending_prepares: HashSet<StaticImageKey>,
    pub(crate) displayed: Option<DisplayedStaticImagePreview>,
    pub(crate) displayed_excluded: Vec<Rect>,
    pub(crate) activation_ready_at: Option<Instant>,
    pub(crate) selection_activation_delay: Duration,
    pub(crate) ffmpeg_available: Option<bool>,
    pub(crate) resvg_available: Option<bool>,
    pub(crate) magick_available: Option<bool>,
    pub(crate) preload_viewport: Option<StaticImagePreloadViewport>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StaticImageOverlayMode {
    FullPane,
    Inline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticImageOverlayRequest {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) area: Rect,
    pub(crate) target_width_px: u32,
    pub(crate) target_height_px: u32,
    pub(crate) mode: StaticImageOverlayMode,
    pub(crate) force_render_to_cache: bool,
    pub(crate) prepare_inline_payload: bool,
}

pub(crate) struct PreparedStaticImage {
    pub(crate) display_path: PathBuf,
    pub(crate) dimensions: RenderedImageDimensions,
    pub(crate) inline_payload: Option<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DisplayedStaticImagePreview {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) area: Rect,
    pub(crate) clear_area: Rect,
    pub(crate) mode: StaticImageOverlayMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaticImagePreloadViewport {
    pub(crate) selected: usize,
    pub(crate) scroll_row: usize,
    pub(crate) cols: usize,
    pub(crate) rows_visible: usize,
    pub(crate) preview_content_area: Option<Rect>,
    pub(crate) preview_media_area: Option<Rect>,
    pub(crate) protocol: ImageProtocol,
    pub(crate) window: Option<TerminalWindowSize>,
}

pub(crate) enum StaticImageOverlayPreparation {
    Ready(PreparedStaticImage),
    Pending,
    Failed,
}

impl StaticImageKey {
    pub(crate) fn from_request(request: &StaticImageOverlayRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            target_width_px: request.target_width_px,
            target_height_px: request.target_height_px,
            force_render_to_cache: request.force_render_to_cache,
            prepare_inline_payload: request.prepare_inline_payload,
        }
    }
}

impl DisplayedStaticImagePreview {
    pub(crate) fn from_request(
        request: &StaticImageOverlayRequest,
        area: Rect,
        clear_area: Rect,
    ) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            area,
            clear_area,
            mode: request.mode,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TerminalImageState {
    pub(crate) protocol: ImageProtocol,
    pub(crate) identity: TerminalIdentity,
    pub(crate) window: Option<TerminalWindowSize>,
    pub(crate) pending_iterm_erase: Vec<Rect>,
    pub(crate) pending_resize_clear: bool,
    pub(crate) pending_iterm_popup_restore: bool,
    pub(crate) pending_sixel_repaint: bool,
    pub(crate) resize_settled_at: Option<Instant>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OverlayPresentState {
    NotRequested,
    Waiting,
    Displayed,
}

pub(crate) struct PreviewRuntime {
    pub(crate) visible: bool,
    pub(crate) fullscreen: bool,
    pub(crate) exit_fullscreen_after_directory_load: bool,
    pub(crate) state: PreviewState,
    pub(crate) comic: ComicPreviewState,
    pub(crate) epub: EpubPreviewState,
    pub(crate) image: ImagePreviewState,
    pub(crate) media: MediaPreviewState,
    pub(crate) pdf: PdfPreviewState,
    pub(crate) terminal_images: TerminalImageState,
}

impl PreviewRuntime {
    pub(crate) fn new() -> Self {
        Self {
            visible: true,
            fullscreen: false,
            exit_fullscreen_after_directory_load: false,
            state: PreviewState {
                scroll: 0,
                horizontal_scroll: 0,
                content: PreviewContent::placeholder("No selection"),
                token: 0,
                metrics: PreviewMetrics::default(),
                load_state: None,
                directory_stats: None,
                directory_stats_ready_at: None,
                deferred_refresh_at: None,
                prefetch_ready_at: None,
                result_cache: HashMap::new(),
                result_order: VecDeque::new(),
                line_count_cache: HashMap::new(),
                line_count_order: VecDeque::new(),
                pending_line_counts: HashSet::new(),
                incremental_render_in_flight: false,
                incremental_render_path: None,
            },
            comic: ComicPreviewState::default(),
            epub: EpubPreviewState::default(),
            image: ImagePreviewState::default(),
            media: MediaPreviewState::default(),
            pdf: PdfPreviewState::default(),
            terminal_images: TerminalImageState::default(),
        }
    }
}
