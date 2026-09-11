use super::job_requests::ArchiveExtractRequest;
use crate::app::preview::pdf::PdfProbeResult;
use crate::app::preview::static_images::{PreparedStaticImageAsset, SixelDcsKey};
use crate::duplicate_finder::{DuplicateScanBatch, DuplicateScanResult};
use crate::fs::Entry;
use crate::fuzzy_finder::{SearchIndex, SearchIndexBatch, SearchScope};
use crate::preview;
use std::{path::PathBuf, sync::Arc, time::SystemTime};

#[derive(Debug)]
pub(crate) struct SearchBuild {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) scope: SearchScope,
    pub(crate) show_hidden: bool,
    pub(crate) fingerprint: crate::fs::DirectoryFingerprint,
    pub(crate) result: Result<SearchIndex, String>,
}

#[derive(Debug)]
pub(crate) struct SearchBatchBuild {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) scope: SearchScope,
    pub(crate) show_hidden: bool,
    pub(crate) fingerprint: crate::fs::DirectoryFingerprint,
    pub(crate) batch: SearchIndexBatch,
}

#[derive(Debug)]
pub(crate) struct DuplicateScanBuild {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) show_hidden: bool,
    pub(crate) result: Result<DuplicateScanResult, String>,
}

#[derive(Debug)]
pub(crate) struct DuplicateScanBatchBuild {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) show_hidden: bool,
    pub(crate) batch: DuplicateScanBatch,
}

#[derive(Debug)]
pub(crate) struct DirectoryBuild {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) result: Result<crate::fs::DirectorySnapshot, String>,
}

#[derive(Debug)]
pub(crate) struct DirectoryItemCountBuild {
    pub(crate) path: PathBuf,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) show_hidden: bool,
    pub(crate) item_count: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct DirectoryStatsBuild {
    pub(crate) token: u64,
    pub(crate) path: PathBuf,
    pub(crate) result: crate::fs::DirectoryStatsScanResult,
}

#[derive(Debug)]
pub(crate) struct DirectoryFingerprintBuild {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) show_hidden: bool,
    pub(crate) result: Result<crate::fs::DirectoryFingerprint, String>,
}

#[derive(Debug)]
pub(crate) struct GitStatusBuild {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) dirty: bool,
}

#[derive(Debug)]
pub(crate) struct PreviewLineCountBuild {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) total_lines: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct ImagePrepareBuild {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) target_width_px: u32,
    pub(crate) target_height_px: u32,
    pub(crate) force_render_to_cache: bool,
    pub(crate) prepare_inline_payload: bool,
    pub(crate) canceled: bool,
    pub(crate) result: Option<PreparedStaticImageAsset>,
}

#[derive(Debug)]
pub(crate) struct PdfProbeBuild {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
    pub(crate) result: Result<PdfProbeResult, String>,
}

#[derive(Debug)]
pub(crate) struct PdfRenderBuild {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) sixel_dcs: Option<Arc<[u8]>>,
    pub(crate) sixel_dcs_key: Option<SixelDcsKey>,
    pub(crate) result: Result<Option<PathBuf>, String>,
}

#[derive(Debug)]
pub(crate) struct PreviewBuild {
    pub(crate) token: u64,
    pub(crate) entry: Entry,
    pub(crate) variant: preview::PreviewRequestOptions,
    pub(crate) code_line_limit: usize,
    /// The actual line limit used for this render pass. May be less than
    /// `code_line_limit` for initial incremental renders.
    pub(crate) code_render_limit: usize,
    pub(crate) ffmpeg_available: bool,
    pub(crate) result: preview::PreviewContent,
}

#[derive(Debug)]
pub(crate) struct ArchiveCreateBuild {
    pub(crate) token: u64,
    pub(crate) completed: usize,
    pub(crate) total: usize,
    /// `true` on the final result; `false` on intermediate progress updates.
    pub(crate) done: bool,
    /// Populated only when `done = true`.
    pub(crate) output_path: Option<PathBuf>,
    /// Populated only when `done = true`.
    pub(crate) status: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArchivePasswordPrompt {
    Required,
    BadPassword,
}

#[derive(Debug)]
pub(crate) struct ArchiveExtractBuild {
    pub(crate) token: u64,
    pub(crate) completed: usize,
    pub(crate) total: Option<usize>,
    /// `true` on the final result; `false` on intermediate progress updates.
    pub(crate) done: bool,
    /// Populated only when `done = true`.
    pub(crate) dest_dir: Option<PathBuf>,
    /// Populated only when `done = true`.
    pub(crate) status: Option<String>,
    /// Populated only when `done = true` and extraction needs password input.
    pub(crate) password_prompt: Option<ArchivePasswordPrompt>,
    /// Remaining batch to resume after password input.
    pub(crate) password_request: Option<ArchiveExtractRequest>,
}

#[derive(Debug)]
pub(crate) struct PasteBuild {
    pub(crate) token: u64,
    pub(crate) completed: usize,
    /// `true` on the final result; `false` on intermediate progress updates.
    pub(crate) done: bool,
    /// Populated only when `done = true`.
    pub(crate) status: Option<String>,
    /// Destination paths actually written by the completed paste/drop.
    pub(crate) destination_paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub(crate) struct TrashBuild {
    pub(crate) token: u64,
    pub(crate) completed: usize,
    /// `true` on the final result; `false` on intermediate progress updates.
    pub(crate) done: bool,
    /// Populated only when `done = true`.
    pub(crate) status: Option<String>,
}

#[derive(Debug)]
pub(crate) struct RestoreBuild {
    pub(crate) token: u64,
    pub(crate) completed: usize,
    /// `true` on the final result; `false` on intermediate progress updates.
    pub(crate) done: bool,
    /// Populated only when `done = true`.
    pub(crate) status: Option<String>,
}

#[derive(Debug)]
pub(crate) enum JobResult {
    Directory(DirectoryBuild),
    DirectoryFingerprint(DirectoryFingerprintBuild),
    DirectoryItemCount(DirectoryItemCountBuild),
    DirectoryStats(DirectoryStatsBuild),
    GitStatus(GitStatusBuild),
    PreviewLineCount(PreviewLineCountBuild),
    ImagePrepare(ImagePrepareBuild),
    PdfProbe(PdfProbeBuild),
    PdfRender(PdfRenderBuild),
    SearchBatch(SearchBatchBuild),
    Search(SearchBuild),
    DuplicateScanBatch(DuplicateScanBatchBuild),
    DuplicateScan(DuplicateScanBuild),
    Preview(Box<PreviewBuild>),
    ArchiveCreate(ArchiveCreateBuild),
    ArchiveExtract(ArchiveExtractBuild),
    Paste(PasteBuild),
    Trash(TrashBuild),
    Restore(RestoreBuild),
}
