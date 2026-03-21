use crate::app::overlays::inline_image::{
    ImageProtocol, RenderedImageDimensions, TerminalWindowSize,
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
    pub(in crate::app::overlays) dimensions: HashMap<StaticImageKey, RenderedImageDimensions>,
    pub(super) rendered_images: HashMap<StaticImageKey, PathBuf>,
    pub(super) render_order: VecDeque<StaticImageKey>,
    pub(super) inline_payloads: HashMap<StaticImageKey, Arc<str>>,
    pub(super) payload_order: VecDeque<StaticImageKey>,
    pub(in crate::app::overlays) failed_images: HashSet<StaticImageKey>,
    pub(in crate::app::overlays) pending_prepares: HashSet<StaticImageKey>,
    pub(super) displayed: Option<DisplayedStaticImagePreview>,
    pub(super) displayed_excluded: Vec<Rect>,
    pub(super) activation_ready_at: Option<Instant>,
    pub(in crate::app) selection_activation_delay: Duration,
    pub(super) ffmpeg_available: Option<bool>,
    pub(super) resvg_available: Option<bool>,
    pub(super) magick_available: Option<bool>,
    pub(super) preload_viewport: Option<StaticImagePreloadViewport>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app) struct StaticImageKey {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) target_width_px: u32,
    pub(super) target_height_px: u32,
    pub(super) force_render_to_cache: bool,
    pub(super) prepare_inline_payload: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum StaticImageOverlayMode {
    FullPane,
    Inline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct StaticImageOverlayRequest {
    pub(in crate::app::overlays) path: PathBuf,
    pub(in crate::app::overlays) size: u64,
    pub(in crate::app::overlays) modified: Option<SystemTime>,
    pub(in crate::app::overlays) area: Rect,
    pub(in crate::app::overlays) target_width_px: u32,
    pub(in crate::app::overlays) target_height_px: u32,
    pub(in crate::app::overlays) mode: StaticImageOverlayMode,
    pub(in crate::app::overlays) force_render_to_cache: bool,
    pub(in crate::app::overlays) prepare_inline_payload: bool,
}

pub(in crate::app) struct PreparedStaticImage {
    pub(in crate::app::overlays) display_path: PathBuf,
    pub(in crate::app::overlays) dimensions: RenderedImageDimensions,
    pub(in crate::app::overlays) inline_payload: Option<Arc<str>>,
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

#[derive(Debug)]
pub(in crate::app) struct PreparedStaticImageAsset {
    pub(in crate::app::overlays) display_path: PathBuf,
    pub(in crate::app::overlays) dimensions: RenderedImageDimensions,
    pub(in crate::app::overlays) inline_payload: Option<Arc<str>>,
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

    pub(in crate::app) fn from_parts(
        path: PathBuf,
        size: u64,
        modified: Option<SystemTime>,
        target_width_px: u32,
        target_height_px: u32,
        force_render_to_cache: bool,
        prepare_inline_payload: bool,
    ) -> Self {
        Self {
            path,
            size,
            modified,
            target_width_px,
            target_height_px,
            force_render_to_cache,
            prepare_inline_payload,
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
