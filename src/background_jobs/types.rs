use crate::app::preview::pdf::PdfProbeResult;
use crate::app::preview::static_images::{PreparedStaticImageAsset, SixelDcsKey};
use crate::app::preview::terminal_images::TerminalWindowSize;
use crate::duplicate_finder::{DuplicateScanBatch, DuplicateScanResult};
use crate::file_operations::ClipOp;
use crate::fs::{Entry, SortMode};
use crate::fuzzy_finder::{SearchIndex, SearchIndexBatch, SearchScope};
use crate::{preview, preview::PreviewWorkClass};
use std::{path::PathBuf, sync::Arc, time::SystemTime};

/// Parameters needed by the background image-prepare job to pre-encode a
/// Sixel DCS stream alongside the rendered PNG.  Bundled as an `Option` so
/// non-Sixel sessions pay no extra memory cost.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SixelPrepareConfig {
    /// Width of the target area in terminal cells.
    pub(crate) area_width: u16,
    /// Height of the target area in terminal cells.
    pub(crate) area_height: u16,
    /// Terminal window dimensions at the time the job was submitted.
    /// Required to reproduce the exact aspect-ratio fitting and pixel-size
    /// computation that will be used at render time.
    pub(crate) window_size: TerminalWindowSize,
}

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

#[derive(Clone, Debug)]
pub(crate) struct SearchRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) scope: SearchScope,
    pub(crate) show_hidden: bool,
    pub(crate) fingerprint: crate::fs::DirectoryFingerprint,
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

#[derive(Clone, Debug)]
pub(crate) struct DuplicateScanRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) show_hidden: bool,
}

#[derive(Debug)]
pub(crate) struct DirectoryBuild {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) result: Result<crate::fs::DirectorySnapshot, String>,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectoryRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) show_hidden: bool,
    pub(crate) sort_mode: SortMode,
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

#[derive(Debug)]
pub(crate) struct GitStatusBuild {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) dirty: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct GitStatusRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
}

#[derive(Debug)]
pub(crate) struct PreviewLineCountBuild {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) total_lines: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreviewLineCountRequest {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
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

#[derive(Clone, Debug)]
pub(crate) struct ImagePrepareRequest {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) target_width_px: u32,
    pub(crate) target_height_px: u32,
    pub(crate) ffmpeg_available: bool,
    pub(crate) resvg_available: bool,
    pub(crate) magick_available: bool,
    pub(crate) force_render_to_cache: bool,
    pub(crate) prepare_inline_payload: bool,
    /// When `Some`, the prepare job also encodes a Sixel DCS stream for the
    /// rendered image using the area and window dimensions supplied here.
    pub(crate) sixel_prepare: Option<SixelPrepareConfig>,
}

#[derive(Debug)]
pub(crate) struct PdfProbeBuild {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
    pub(crate) result: Result<PdfProbeResult, String>,
}

#[derive(Clone, Debug)]
pub(crate) struct PdfProbeRequest {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) page: usize,
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

#[derive(Clone, Debug)]
pub(crate) struct ArchiveCreateRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) sources: Vec<PathBuf>,
    pub(crate) output_name: String,
    pub(crate) options: crate::archive::CreateArchiveOptions,
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

#[derive(Clone, Debug)]
pub(crate) struct ArchiveExtractBatchState {
    pub(crate) total_archives: usize,
    pub(crate) completed_archives: usize,
    pub(crate) failed_archives: usize,
    pub(crate) skipped_archives: usize,
    pub(crate) skipped_non_archives: usize,
    pub(crate) dest_dirs: Vec<PathBuf>,
}

impl ArchiveExtractBatchState {
    pub(crate) fn new(total_archives: usize, skipped_non_archives: usize) -> Self {
        Self {
            total_archives,
            completed_archives: 0,
            failed_archives: 0,
            skipped_archives: 0,
            skipped_non_archives,
            dest_dirs: Vec::new(),
        }
    }

    pub(crate) fn finished_archives(&self) -> usize {
        self.completed_archives + self.failed_archives + self.skipped_archives
    }

    pub(crate) fn is_single_archive(&self) -> bool {
        self.total_archives == 1 && self.skipped_non_archives == 0
    }

    pub(crate) fn reselect_path(&self) -> Option<PathBuf> {
        self.is_single_archive()
            .then(|| self.dest_dirs.first().cloned())
            .flatten()
    }

    pub(crate) fn status(&self) -> String {
        if self.is_single_archive()
            && self.completed_archives == 1
            && let Some(dest_dir) = self.dest_dirs.first()
        {
            let name = dest_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("folder");
            return format!("Extracted 1 archive to \"{name}\"");
        }

        let mut parts = Vec::new();
        parts.push(format!(
            "Extracted {} {}",
            self.completed_archives,
            if self.completed_archives == 1 {
                "archive"
            } else {
                "archives"
            }
        ));
        if self.failed_archives > 0 {
            parts.push(format!("{} failed", self.failed_archives));
        }
        if self.skipped_archives > 0 {
            parts.push(format!("{} skipped", self.skipped_archives));
        }
        if self.skipped_non_archives > 0 {
            parts.push(format!(
                "skipped {} {}",
                self.skipped_non_archives,
                if self.skipped_non_archives == 1 {
                    "non-archive"
                } else {
                    "non-archives"
                }
            ));
        }
        parts.join(", ")
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ArchiveExtractRequest {
    pub(crate) token: u64,
    pub(crate) archives: Vec<PathBuf>,
    pub(crate) password: Option<crate::archive::ArchivePassword>,
    pub(crate) batch: ArchiveExtractBatchState,
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

#[derive(Clone, Debug)]
pub(crate) struct PasteRequest {
    pub(crate) token: u64,
    pub(crate) dest_dir: PathBuf,
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) op: ClipOp,
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

#[derive(Clone, Debug)]
pub(crate) struct TrashRequest {
    pub(crate) token: u64,
    pub(crate) targets: Vec<crate::file_operations::TrashTarget>,
    pub(crate) permanent: bool,
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

#[derive(Clone, Debug)]
pub(crate) struct RestoreRequest {
    pub(crate) token: u64,
    pub(crate) targets: Vec<crate::file_operations::TrashTarget>,
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
