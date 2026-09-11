use crate::{
    preview::images::{SixelDcsKey, StaticImageKey},
    terminal_runtime::terminal_images::{
        ImageProtocol, RenderedImageDimensions, TerminalWindowSize,
    },
};
use ratatui::layout::Rect;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct ImagePreviewState {
    pub(in crate::app::preview) dimensions: HashMap<StaticImageKey, RenderedImageDimensions>,
    pub(super) rendered_images: HashMap<StaticImageKey, PathBuf>,
    pub(super) render_order: VecDeque<StaticImageKey>,
    pub(super) inline_payloads: HashMap<StaticImageKey, Arc<str>>,
    pub(super) payload_order: VecDeque<StaticImageKey>,
    /// Cached Sixel DCS byte streams keyed by display path + placement area +
    /// window dimensions.  Entries are evicted LRU-style once the cache exceeds
    /// `SIXEL_DCS_CACHE_LIMIT`.
    pub(in crate::app::preview) sixel_dcs_payloads: HashMap<SixelDcsKey, Arc<[u8]>>,
    pub(in crate::app::preview) sixel_dcs_order: VecDeque<SixelDcsKey>,
    pub(in crate::app::preview) failed_images: HashSet<StaticImageKey>,
    pub(in crate::app::preview) pending_prepares: HashSet<StaticImageKey>,
    pub(super) displayed: Option<DisplayedStaticImagePreview>,
    pub(super) displayed_excluded: Vec<Rect>,
    pub(super) activation_ready_at: Option<Instant>,
    pub(in crate::app) selection_activation_delay: Duration,
    pub(super) ffmpeg_available: Option<bool>,
    pub(super) resvg_available: Option<bool>,
    pub(super) magick_available: Option<bool>,
    pub(super) preload_viewport: Option<StaticImagePreloadViewport>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum StaticImageOverlayMode {
    FullPane,
    Inline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct StaticImageOverlayRequest {
    pub(in crate::app::preview) path: PathBuf,
    pub(in crate::app::preview) size: u64,
    pub(in crate::app::preview) modified: Option<SystemTime>,
    pub(in crate::app::preview) area: Rect,
    pub(in crate::app::preview) target_width_px: u32,
    pub(in crate::app::preview) target_height_px: u32,
    pub(in crate::app::preview) mode: StaticImageOverlayMode,
    pub(in crate::app::preview) force_render_to_cache: bool,
    pub(in crate::app::preview) prepare_inline_payload: bool,
}

pub(in crate::app) struct PreparedStaticImage {
    pub(in crate::app::preview) display_path: PathBuf,
    pub(in crate::app::preview) dimensions: RenderedImageDimensions,
    pub(in crate::app::preview) inline_payload: Option<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DisplayedStaticImagePreview {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    area: Rect,
    pub(super) clear_area: Rect,
    pub(super) mode: StaticImageOverlayMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StaticImagePreloadViewport {
    pub(super) selected: usize,
    pub(super) scroll_row: usize,
    pub(super) cols: usize,
    pub(super) rows_visible: usize,
    pub(super) preview_content_area: Option<Rect>,
    pub(super) preview_media_area: Option<Rect>,
    pub(super) protocol: ImageProtocol,
    pub(super) window: Option<TerminalWindowSize>,
}

pub(in crate::app) enum StaticImageOverlayPreparation {
    Ready(PreparedStaticImage),
    Pending,
    Failed,
}

impl StaticImageKey {
    pub(in crate::app) fn from_request(request: &StaticImageOverlayRequest) -> Self {
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
    pub(super) fn from_request(
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
