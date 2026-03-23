mod pool;
mod results;
mod tasks;

#[cfg(test)]
use self::{
    pool::{preview::PreviewJobKey, search::SearchJobKey},
    tasks::{image::ImagePrepareJobKey, pdf_probe::PdfProbeJobKey, pdf_render::PdfRenderJobKey},
};
use self::{
    pool::{preview::PreviewPool, search::SearchPool},
    tasks::{
        directory::DirectoryPool, directory_fingerprint::DirectoryFingerprintPool,
        image::ImagePreparePool, item_count::DirectoryItemCountPool,
        line_count::PreviewLineCountPool, paste::PastePool, pdf_probe::PdfProbePool,
        pdf_render::PdfRenderPool,
    },
};
use super::overlays::images::PreparedStaticImageAsset;
use super::overlays::pdf::PdfProbeResult;
use super::*;
use crate::fs::search::SearchCandidate;
use crate::preview::PreviewWorkClass;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, MutexGuard, mpsc},
    time::{Duration, SystemTime},
};

const PREVIEW_WORKER_COUNT: usize = 2;
const SEARCH_WORKER_COUNT: usize = 1;
const DIRECTORY_ITEM_COUNT_WORKER_COUNT: usize = 1;
const DIRECTORY_FINGERPRINT_WORKER_COUNT: usize = 1;
const PREVIEW_LINE_COUNT_WORKER_COUNT: usize = 1;
const IMAGE_PREPARE_WORKER_COUNT: usize = 2;
const PDF_PROBE_WORKER_COUNT: usize = 2;
const PDF_RENDER_WORKER_COUNT: usize = 2;
const PREVIEW_QUEUE_LIMIT: usize = 8;
const DIRECTORY_ITEM_COUNT_QUEUE_LIMIT: usize = 48;
const PREVIEW_LINE_COUNT_QUEUE_LIMIT: usize = 16;
const IMAGE_PREPARE_QUEUE_LIMIT: usize = 6;
const PDF_PROBE_QUEUE_LIMIT: usize = 16;
const PDF_RENDER_QUEUE_LIMIT: usize = 8;

#[derive(Clone, Copy, Debug)]
struct SchedulerConfig {
    search_worker_count: usize,
    preview_worker_count: usize,
    preview_queue_limit: usize,
    directory_item_count_worker_count: usize,
    directory_item_count_queue_limit: usize,
    directory_fingerprint_worker_count: usize,
    preview_line_count_worker_count: usize,
    preview_line_count_queue_limit: usize,
    image_prepare_worker_count: usize,
    image_prepare_queue_limit: usize,
    pdf_probe_worker_count: usize,
    pdf_probe_queue_limit: usize,
    pdf_render_worker_count: usize,
    pdf_render_queue_limit: usize,
}

impl SchedulerConfig {
    fn production() -> Self {
        Self {
            search_worker_count: SEARCH_WORKER_COUNT,
            preview_worker_count: PREVIEW_WORKER_COUNT,
            preview_queue_limit: PREVIEW_QUEUE_LIMIT,
            directory_item_count_worker_count: DIRECTORY_ITEM_COUNT_WORKER_COUNT,
            directory_item_count_queue_limit: DIRECTORY_ITEM_COUNT_QUEUE_LIMIT,
            directory_fingerprint_worker_count: DIRECTORY_FINGERPRINT_WORKER_COUNT,
            preview_line_count_worker_count: PREVIEW_LINE_COUNT_WORKER_COUNT,
            preview_line_count_queue_limit: PREVIEW_LINE_COUNT_QUEUE_LIMIT,
            image_prepare_worker_count: IMAGE_PREPARE_WORKER_COUNT,
            image_prepare_queue_limit: IMAGE_PREPARE_QUEUE_LIMIT,
            pdf_probe_worker_count: PDF_PROBE_WORKER_COUNT,
            pdf_probe_queue_limit: PDF_PROBE_QUEUE_LIMIT,
            pdf_render_worker_count: PDF_RENDER_WORKER_COUNT,
            pdf_render_queue_limit: PDF_RENDER_QUEUE_LIMIT,
        }
    }

    #[cfg(test)]
    fn for_tests(
        search_worker_count: usize,
        preview_worker_count: usize,
        preview_queue_limit: usize,
    ) -> Self {
        Self {
            search_worker_count,
            preview_worker_count,
            preview_queue_limit,
            directory_item_count_worker_count: DIRECTORY_ITEM_COUNT_WORKER_COUNT,
            directory_item_count_queue_limit: DIRECTORY_ITEM_COUNT_QUEUE_LIMIT,
            directory_fingerprint_worker_count: 0,
            preview_line_count_worker_count: 0,
            preview_line_count_queue_limit: PREVIEW_LINE_COUNT_QUEUE_LIMIT,
            image_prepare_worker_count: 0,
            image_prepare_queue_limit: IMAGE_PREPARE_QUEUE_LIMIT,
            pdf_probe_worker_count: 0,
            pdf_probe_queue_limit: PDF_PROBE_QUEUE_LIMIT,
            pdf_render_worker_count: 0,
            pdf_render_queue_limit: PDF_RENDER_QUEUE_LIMIT,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PreviewPriority {
    High,
    Low,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PdfJobPriority {
    Current,
    Prefetch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ImageJobPriority {
    Current,
    Nearby,
}

#[derive(Debug)]
pub(super) struct SearchBuild {
    pub token: u64,
    pub cwd: PathBuf,
    pub scope: SearchScope,
    pub show_hidden: bool,
    pub result: Result<Arc<Vec<SearchCandidate>>, String>,
}

#[derive(Clone, Debug)]
pub(super) struct SearchRequest {
    pub token: u64,
    pub cwd: PathBuf,
    pub scope: SearchScope,
    pub show_hidden: bool,
}

#[derive(Debug)]
pub(super) struct DirectoryBuild {
    pub token: u64,
    pub cwd: PathBuf,
    pub result: Result<crate::fs::DirectorySnapshot, String>,
}

#[derive(Clone, Debug)]
pub(super) struct DirectoryRequest {
    pub token: u64,
    pub cwd: PathBuf,
    pub show_hidden: bool,
    pub sort_mode: SortMode,
}

#[derive(Debug)]
pub(super) struct DirectoryItemCountBuild {
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
    pub show_hidden: bool,
    pub item_count: Option<usize>,
}

#[derive(Debug)]
pub(super) struct DirectoryFingerprintBuild {
    pub token: u64,
    pub cwd: PathBuf,
    pub show_hidden: bool,
    pub result: Result<crate::fs::DirectoryFingerprint, String>,
}

#[derive(Clone, Debug)]
pub(super) struct DirectoryFingerprintRequest {
    pub token: u64,
    pub cwd: PathBuf,
    pub show_hidden: bool,
}

#[derive(Clone, Debug)]
pub(super) struct DirectoryItemCountRequest {
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
    pub show_hidden: bool,
}

#[derive(Debug)]
pub(super) struct PreviewLineCountBuild {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub total_lines: Option<usize>,
}

#[derive(Clone, Debug)]
pub(super) struct PreviewLineCountRequest {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug)]
pub(super) struct ImagePrepareBuild {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub target_width_px: u32,
    pub target_height_px: u32,
    pub force_render_to_cache: bool,
    pub prepare_inline_payload: bool,
    pub canceled: bool,
    pub result: Option<PreparedStaticImageAsset>,
}

#[derive(Clone, Debug)]
pub(super) struct ImagePrepareRequest {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub target_width_px: u32,
    pub target_height_px: u32,
    pub ffmpeg_available: bool,
    pub resvg_available: bool,
    pub magick_available: bool,
    pub force_render_to_cache: bool,
    pub prepare_inline_payload: bool,
}

#[derive(Debug)]
pub(super) struct PdfProbeBuild {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub page: usize,
    pub result: Result<PdfProbeResult, String>,
}

#[derive(Clone, Debug)]
pub(super) struct PdfProbeRequest {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub page: usize,
}

#[derive(Debug)]
pub(super) struct PdfRenderBuild {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub page: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub result: Result<Option<PathBuf>, String>,
}

#[derive(Clone, Debug)]
pub(super) struct PdfRenderRequest {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub page: usize,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Debug)]
pub(super) struct PreviewBuild {
    pub token: u64,
    pub entry: Entry,
    pub variant: preview::PreviewRequestOptions,
    pub code_line_limit: usize,
    pub result: preview::PreviewContent,
}

#[derive(Clone, Debug)]
pub(super) struct PreviewRequest {
    pub token: u64,
    pub entry: Entry,
    pub variant: preview::PreviewRequestOptions,
    pub code_line_limit: usize,
    pub priority: PreviewPriority,
    pub work_class: PreviewWorkClass,
    pub ffprobe_available: bool,
    pub ffmpeg_available: bool,
}

#[derive(Clone, Debug)]
pub(super) struct PasteRequest {
    pub token: u64,
    pub dest_dir: std::path::PathBuf,
    pub paths: Vec<std::path::PathBuf>,
    pub op: ClipOp,
}

#[derive(Debug)]
pub(super) struct PasteBuild {
    pub token: u64,
    pub completed: usize,
    /// `true` on the final result; `false` on intermediate progress updates.
    pub done: bool,
    /// Populated only when `done = true`.
    pub status: Option<String>,
}

#[derive(Debug)]
pub(super) enum JobResult {
    Directory(DirectoryBuild),
    DirectoryFingerprint(DirectoryFingerprintBuild),
    DirectoryItemCount(DirectoryItemCountBuild),
    PreviewLineCount(PreviewLineCountBuild),
    ImagePrepare(ImagePrepareBuild),
    PdfProbe(PdfProbeBuild),
    PdfRender(PdfRenderBuild),
    Search(SearchBuild),
    Preview(Box<PreviewBuild>),
    Paste(PasteBuild),
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SchedulerMetricsSnapshot {
    pub directory_jobs_submitted: u64,
    pub directory_jobs_completed: u64,
    pub search_jobs_submitted: u64,
    pub search_jobs_completed: u64,
    pub preview_jobs_submitted_high: u64,
    pub preview_jobs_submitted_low: u64,
    pub preview_jobs_completed: u64,
    pub preview_low_priority_evictions: u64,
    pub preview_promotions: u64,
    pub preview_avg_build_ms: u64,
    pub preview_max_build_ms: u64,
    pub preview_pending_high: usize,
    pub preview_pending_low: usize,
    pub preview_active: usize,
}

pub(super) struct JobScheduler {
    directory: DirectoryPool,
    directory_fingerprint: DirectoryFingerprintPool,
    paste: PastePool,
    directory_item_count: DirectoryItemCountPool,
    preview_line_count: PreviewLineCountPool,
    image_prepare: ImagePreparePool,
    pdf_probe: PdfProbePool,
    pdf_render: PdfRenderPool,
    search: SearchPool,
    preview: PreviewPool,
    result_rx: mpsc::Receiver<JobResult>,
    buffered_results: Mutex<VecDeque<JobResult>>,
    #[cfg(test)]
    metrics: Arc<Mutex<SchedulerMetrics>>,
}

impl JobScheduler {
    pub(super) fn new() -> Self {
        Self::with_config(SchedulerConfig::production())
    }

    fn with_config(config: SchedulerConfig) -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        let metrics = Arc::new(Mutex::new(SchedulerMetrics::default()));
        Self {
            directory: DirectoryPool::new(1, result_tx.clone(), Arc::clone(&metrics)),
            paste: PastePool::new(result_tx.clone()),
            directory_fingerprint: DirectoryFingerprintPool::new(
                config.directory_fingerprint_worker_count,
                result_tx.clone(),
            ),
            directory_item_count: DirectoryItemCountPool::new(
                config.directory_item_count_worker_count,
                config.directory_item_count_queue_limit,
                result_tx.clone(),
            ),
            preview_line_count: PreviewLineCountPool::new(
                config.preview_line_count_worker_count,
                config.preview_line_count_queue_limit,
                result_tx.clone(),
            ),
            image_prepare: ImagePreparePool::new(
                config.image_prepare_worker_count,
                config.image_prepare_queue_limit,
                result_tx.clone(),
            ),
            pdf_probe: PdfProbePool::new(
                config.pdf_probe_worker_count,
                config.pdf_probe_queue_limit,
                result_tx.clone(),
            ),
            pdf_render: PdfRenderPool::new(
                config.pdf_render_worker_count,
                config.pdf_render_queue_limit,
                result_tx.clone(),
            ),
            search: SearchPool::new(
                config.search_worker_count,
                result_tx.clone(),
                Arc::clone(&metrics),
            ),
            preview: PreviewPool::new(
                config.preview_worker_count,
                config.preview_queue_limit,
                result_tx,
                Arc::clone(&metrics),
            ),
            result_rx,
            buffered_results: Mutex::new(VecDeque::new()),
            #[cfg(test)]
            metrics,
        }
    }

    pub(super) fn submit_directory(&self, request: DirectoryRequest) -> bool {
        self.directory.submit(request)
    }

    pub(super) fn submit_directory_fingerprint(
        &self,
        request: DirectoryFingerprintRequest,
    ) -> bool {
        self.directory_fingerprint.submit(request)
    }

    pub(super) fn submit_directory_item_count(&self, request: DirectoryItemCountRequest) -> bool {
        self.directory_item_count.submit(request)
    }

    pub(super) fn submit_preview_line_count(&self, request: PreviewLineCountRequest) -> bool {
        self.preview_line_count.submit(request)
    }

    pub(super) fn submit_image_prepare(&self, request: ImagePrepareRequest) -> bool {
        self.image_prepare
            .submit(request, ImageJobPriority::Current)
    }

    pub(super) fn submit_nearby_image_prepare(&self, request: ImagePrepareRequest) -> bool {
        self.image_prepare.submit(request, ImageJobPriority::Nearby)
    }

    pub(super) fn retain_image_prepares(
        &self,
        current: Option<&ImagePrepareRequest>,
        nearby: &[ImagePrepareRequest],
    ) {
        self.image_prepare.retain_pending(current, nearby);
    }

    pub(super) fn submit_pdf_probe(
        &self,
        request: PdfProbeRequest,
        priority: PdfJobPriority,
    ) -> bool {
        self.pdf_probe.submit(request, priority)
    }

    pub(super) fn submit_pdf_render(
        &self,
        request: PdfRenderRequest,
        priority: PdfJobPriority,
    ) -> bool {
        self.pdf_render.submit(request, priority)
    }

    pub(super) fn clear_pending_pdf_jobs(&self) {
        self.pdf_probe.clear_pending();
        self.pdf_render.clear_pending();
    }

    pub(super) fn retain_pdf_probe_pages(
        &self,
        path: &Path,
        size: u64,
        modified: Option<SystemTime>,
        keep_pages: &[usize],
    ) {
        self.pdf_probe
            .retain_pending(path, size, modified, keep_pages);
    }

    pub(super) fn retain_pdf_render_variants(
        &self,
        path: &Path,
        size: u64,
        modified: Option<SystemTime>,
        keep_variants: &[(usize, u32, u32)],
    ) {
        self.pdf_render
            .retain_pending(path, size, modified, keep_variants);
    }

    pub(super) fn submit_paste(&self, request: PasteRequest) -> bool {
        self.paste.submit(request)
    }

    pub(super) fn cancel_paste(&self, token: u64) {
        self.paste.cancel_paste(token);
    }

    pub(super) fn submit_search(&self, request: SearchRequest) -> bool {
        self.search.submit(request)
    }

    pub(super) fn submit_preview(&self, request: PreviewRequest) -> bool {
        self.preview.submit(request)
    }

    pub(super) fn try_recv(&self) -> Result<JobResult, mpsc::TryRecvError> {
        if let Some(job) = lock_unpoison(&self.buffered_results).pop_front() {
            return Ok(job);
        }
        self.result_rx.try_recv()
    }

    pub(super) fn defer_result(&self, job: JobResult) {
        lock_unpoison(&self.buffered_results).push_front(job);
    }

    pub(super) fn has_pending_work(&self) -> bool {
        !lock_unpoison(&self.buffered_results).is_empty()
            || self.directory.has_pending_work()
            || self.directory_fingerprint.has_pending_work()
            || self.paste.has_pending_work()
            || self.directory_item_count.has_pending_work()
            || self.preview_line_count.has_pending_work()
            || self.image_prepare.has_pending_work()
            || self.pdf_probe.has_pending_work()
            || self.pdf_render.has_pending_work()
            || self.search.has_pending_work()
            || self.preview.has_pending_work()
    }

    #[cfg(test)]
    pub(super) fn metrics_snapshot(&self) -> SchedulerMetricsSnapshot {
        let mut snapshot = lock_unpoison(&self.metrics).snapshot();
        snapshot.preview_pending_high = self.preview.pending_len(PreviewPriority::High);
        snapshot.preview_pending_low = self.preview.pending_len(PreviewPriority::Low);
        snapshot.preview_active = self.preview.active_len();
        snapshot
    }

    #[cfg(test)]
    fn new_for_tests(
        search_worker_count: usize,
        preview_worker_count: usize,
        preview_queue_limit: usize,
    ) -> Self {
        Self::with_config(SchedulerConfig::for_tests(
            search_worker_count,
            preview_worker_count,
            preview_queue_limit,
        ))
    }

    #[cfg(test)]
    fn snapshot(&self) -> SchedulerSnapshot {
        SchedulerSnapshot {
            search_pending: self.search.pending_key(),
            search_active: self.search.active_key(),
            image_prepare_pending: self.image_prepare.pending_keys(),
            pdf_probe_pending: self.pdf_probe.pending_keys(),
            pdf_render_pending: self.pdf_render.pending_keys(),
            preview_pending_high: self.preview.pending_keys(PreviewPriority::High),
            preview_pending_low: self.preview.pending_keys(PreviewPriority::Low),
            preview_active: self.preview.active_keys(),
        }
    }
}

#[derive(Default)]
pub(super) struct SchedulerMetrics {
    directory_jobs_submitted: u64,
    directory_jobs_completed: u64,
    search_jobs_submitted: u64,
    search_jobs_completed: u64,
    preview_jobs_submitted_high: u64,
    preview_jobs_submitted_low: u64,
    preview_jobs_completed: u64,
    preview_low_priority_evictions: u64,
    preview_promotions: u64,
    preview_total_build_time: Duration,
    preview_max_build_time: Duration,
}

impl SchedulerMetrics {
    pub(super) fn record_directory_completed(&mut self, _elapsed: Duration) {
        self.directory_jobs_completed += 1;
    }

    pub(super) fn record_search_completed(&mut self, _elapsed: Duration) {
        self.search_jobs_completed += 1;
    }

    pub(super) fn record_preview_completed(&mut self, elapsed: Duration) {
        self.preview_jobs_completed += 1;
        self.preview_total_build_time += elapsed;
        self.preview_max_build_time = self.preview_max_build_time.max(elapsed);
    }

    #[cfg(test)]
    fn snapshot(&self) -> SchedulerMetricsSnapshot {
        let preview_avg_build_ms = if self.preview_jobs_completed == 0 {
            0
        } else {
            (self.preview_total_build_time.as_millis() as u64) / self.preview_jobs_completed
        };
        SchedulerMetricsSnapshot {
            directory_jobs_submitted: self.directory_jobs_submitted,
            directory_jobs_completed: self.directory_jobs_completed,
            search_jobs_submitted: self.search_jobs_submitted,
            search_jobs_completed: self.search_jobs_completed,
            preview_jobs_submitted_high: self.preview_jobs_submitted_high,
            preview_jobs_submitted_low: self.preview_jobs_submitted_low,
            preview_jobs_completed: self.preview_jobs_completed,
            preview_low_priority_evictions: self.preview_low_priority_evictions,
            preview_promotions: self.preview_promotions,
            preview_avg_build_ms,
            preview_max_build_ms: self.preview_max_build_time.as_millis() as u64,
            preview_pending_high: 0,
            preview_pending_low: 0,
            preview_active: 0,
        }
    }
}

pub(super) fn lock_unpoison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub(super) fn wait_unpoison<'a, T>(
    condvar: &Condvar,
    guard: MutexGuard<'a, T>,
) -> MutexGuard<'a, T> {
    condvar
        .wait(guard)
        .unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
struct SchedulerSnapshot {
    search_pending: Option<SearchJobKey>,
    search_active: Option<SearchJobKey>,
    image_prepare_pending: Vec<ImagePrepareJobKey>,
    pdf_probe_pending: Vec<PdfProbeJobKey>,
    pdf_render_pending: Vec<PdfRenderJobKey>,
    preview_pending_high: Vec<PreviewJobKey>,
    preview_pending_low: Vec<PreviewJobKey>,
    preview_active: Vec<PreviewJobKey>,
}

#[cfg(test)]
mod tests;
