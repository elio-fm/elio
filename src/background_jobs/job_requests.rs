use crate::filesystem::{Entry, SortMode};
use crate::fuzzy_finder::SearchScope;
use crate::{preview, preview::PreviewWorkClass};
use std::{path::PathBuf, time::SystemTime};

pub(crate) use crate::preview::images::{ImagePrepareRequest, SixelPrepareConfig};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewPriority {
    High,
    Low,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfJobPriority {
    Current,
    Prefetch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImageJobPriority {
    Current,
    Nearby,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) scope: SearchScope,
    pub(crate) show_hidden: bool,
    pub(crate) fingerprint: crate::filesystem::DirectoryFingerprint,
}

#[derive(Clone, Debug)]
pub(crate) struct DuplicateScanRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) show_hidden: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectoryRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) show_hidden: bool,
    pub(crate) sort_mode: SortMode,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectoryFingerprintRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) show_hidden: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectoryItemCountRequest {
    pub(crate) path: PathBuf,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) show_hidden: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectoryStatsRequest {
    pub(crate) token: u64,
    pub(crate) path: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct GitStatusRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct PreviewLineCountRequest {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
}

#[derive(Clone, Debug)]
pub(crate) struct PdfProbeRequest {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct PdfRenderRequest {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) sixel_prepare: Option<SixelPrepareConfig>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreviewRequest {
    pub(crate) token: u64,
    pub(crate) entry: Entry,
    pub(crate) variant: preview::PreviewRequestOptions,
    pub(crate) code_line_limit: usize,
    /// The actual render line limit for this pass. For the initial incremental
    /// render this is smaller than `code_line_limit`; for extension/prefetch
    /// renders it equals `code_line_limit`.
    pub(crate) code_render_limit: usize,
    pub(crate) priority: PreviewPriority,
    pub(crate) work_class: PreviewWorkClass,
    pub(crate) ffprobe_available: bool,
    pub(crate) ffmpeg_available: bool,
}
