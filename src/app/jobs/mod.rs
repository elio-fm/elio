mod config;
mod metrics;
mod pool;
mod results;
mod scheduler;
mod sync;
mod tasks;
mod types;

#[cfg(test)]
pub(super) use self::metrics::SchedulerMetricsSnapshot;
pub(crate) use self::scheduler::JobScheduler;
use self::sync::{lock_unpoison, wait_unpoison};
#[cfg(unix)]
pub(crate) use self::tasks::trash::run_user_trash_helper;
pub(super) use self::types::{
    ArchiveCreateBuild, ArchiveExtractBuild, ArchivePasswordPrompt, DirectoryBuild,
    DirectoryFingerprintBuild, DirectoryFingerprintRequest, DirectoryItemCountBuild,
    DirectoryItemCountRequest, DirectoryRequest, DirectoryStatsBuild, DirectoryStatsRequest,
    DuplicateScanBatchBuild, DuplicateScanBuild, DuplicateScanRequest, GitStatusBuild,
    GitStatusRequest, ImageJobPriority, ImagePrepareBuild, ImagePrepareRequest, JobResult,
    PasteBuild, PdfJobPriority, PdfProbeBuild, PdfProbeRequest, PdfRenderBuild, PdfRenderRequest,
    PreviewBuild, PreviewLineCountBuild, PreviewLineCountRequest, PreviewPriority, PreviewRequest,
    RestoreBuild, SearchBatchBuild, SearchBuild, SearchRequest, SixelPrepareConfig, TrashBuild,
};
pub(crate) use self::types::{
    ArchiveCreateRequest, ArchiveExtractBatchState, ArchiveExtractRequest, PasteRequest,
    RestoreRequest, TrashRequest,
};
use self::{config::SchedulerConfig, metrics::SchedulerMetrics};
#[cfg(test)]
use self::{
    pool::{fuzzy_finder::FuzzyFinderJobKey, preview::PreviewJobKey},
    tasks::{image::ImagePrepareJobKey, pdf_probe::PdfProbeJobKey, pdf_render::PdfRenderJobKey},
};
use super::*;

#[cfg(test)]
mod tests;
