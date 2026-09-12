use crate::file_operations::ClipOp;
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

#[derive(Clone, Debug)]
pub(crate) struct ArchiveCreateRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) sources: Vec<PathBuf>,
    pub(crate) output_name: String,
    pub(crate) options: crate::archive::CreateArchiveOptions,
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

#[derive(Clone, Debug)]
pub(crate) struct PasteRequest {
    pub(crate) token: u64,
    pub(crate) dest_dir: PathBuf,
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) op: ClipOp,
}

#[derive(Clone, Debug)]
pub(crate) struct TrashRequest {
    pub(crate) token: u64,
    pub(crate) targets: Vec<crate::file_operations::TrashTarget>,
    pub(crate) permanent: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct RestoreRequest {
    pub(crate) token: u64,
    pub(crate) targets: Vec<crate::file_operations::TrashTarget>,
}
