#[cfg(test)]
use super::scheduler_metrics::SchedulerMetricsSnapshot;
#[cfg(test)]
use super::workers::{
    fuzzy_finder::FuzzyFinderJobKey, pdf_page_inspection::PdfProbeJobKey,
    pdf_page_rendering::PdfRenderJobKey, preview_building::PreviewJobKey,
    static_image_preparation::ImagePrepareJobKey,
};
use super::{
    job_requests::*,
    job_results::JobResult,
    scheduler_config::SchedulerConfig,
    scheduler_metrics::SchedulerMetrics,
    workers::{
        archive_creation::ArchiveCreatePool,
        archive_extraction::ArchiveExtractPool,
        copy_move::PastePool,
        directory_fingerprint::DirectoryFingerprintPool,
        directory_item_counts::DirectoryItemCountPool,
        directory_loading::DirectoryPool,
        directory_statistics::DirectoryStatsPool,
        duplicate_finder::DuplicateFinderPool,
        fuzzy_finder::FuzzyFinderPool,
        git_status::GitStatusPool,
        pdf_page_inspection::PdfProbePool,
        pdf_page_rendering::PdfRenderPool,
        preview_building::PreviewPool,
        preview_line_counts::PreviewLineCountPool,
        restore::RestorePool,
        static_image_preparation::ImagePreparePool,
        trash_delete::{self, TrashPool},
    },
};
use crate::file_operations::{
    ArchiveCreateRequest, ArchiveExtractRequest, PasteRequest, RestoreRequest, TrashRequest,
};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Condvar, Mutex, MutexGuard, mpsc},
    time::SystemTime,
};

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

pub(crate) struct JobScheduler {
    pub(crate) folder_sizes: super::folder_sizes::FolderSizeWorker,
    directory: DirectoryPool,
    directory_fingerprint: DirectoryFingerprintPool,
    archive_create: ArchiveCreatePool,
    archive_extract: ArchiveExtractPool,
    paste: PastePool,
    trash: TrashPool,
    restore: RestorePool,
    directory_item_count: DirectoryItemCountPool,
    directory_stats: DirectoryStatsPool,
    git_status: GitStatusPool,
    preview_line_count: PreviewLineCountPool,
    image_prepare: ImagePreparePool,
    pdf_probe: PdfProbePool,
    pdf_render: PdfRenderPool,
    search: FuzzyFinderPool,
    duplicates: DuplicateFinderPool,
    preview: PreviewPool,
    result_rx: mpsc::Receiver<JobResult>,
    buffered_results: Mutex<VecDeque<JobResult>>,
    #[cfg(test)]
    metrics: Arc<Mutex<SchedulerMetrics>>,
}

impl JobScheduler {
    pub(crate) fn new() -> Self {
        Self::with_config(SchedulerConfig::production())
    }

    fn with_config(config: SchedulerConfig) -> Self {
        // Reclaim any staging directories left behind by a previous session
        // that was killed before staged-directory cleanup could finish.
        trash_delete::sweep_staging_on_startup();

        let (result_tx, result_rx) = mpsc::channel();
        let metrics = Arc::new(Mutex::new(SchedulerMetrics::default()));
        Self {
            folder_sizes: super::folder_sizes::FolderSizeWorker::new(),
            directory: DirectoryPool::new(1, result_tx.clone(), Arc::clone(&metrics)),
            archive_create: ArchiveCreatePool::new(result_tx.clone()),
            archive_extract: ArchiveExtractPool::new(result_tx.clone()),
            paste: PastePool::new(result_tx.clone()),
            trash: TrashPool::new(result_tx.clone()),
            restore: RestorePool::new(result_tx.clone()),
            directory_fingerprint: DirectoryFingerprintPool::new(
                config.directory_fingerprint_worker_count,
                result_tx.clone(),
            ),
            directory_item_count: DirectoryItemCountPool::new(
                config.directory_item_count_worker_count,
                config.directory_item_count_queue_limit,
                result_tx.clone(),
            ),
            directory_stats: DirectoryStatsPool::new(
                config.directory_stats_worker_count(),
                result_tx.clone(),
            ),
            git_status: GitStatusPool::new(result_tx.clone()),
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
            search: FuzzyFinderPool::new(
                config.search_worker_count,
                result_tx.clone(),
                Arc::clone(&metrics),
            ),
            duplicates: DuplicateFinderPool::new(1, result_tx.clone()),
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

    pub(crate) fn submit_directory(&self, request: DirectoryRequest) -> bool {
        self.directory.submit(request)
    }

    pub(crate) fn submit_directory_fingerprint(
        &self,
        request: DirectoryFingerprintRequest,
    ) -> bool {
        self.directory_fingerprint.submit(request)
    }

    pub(crate) fn cancel_directory_fingerprints(&self) {
        self.directory_fingerprint.cancel_all();
    }

    pub(crate) fn submit_directory_item_count(&self, request: DirectoryItemCountRequest) -> bool {
        self.directory_item_count.submit(request)
    }

    pub(crate) fn submit_directory_stats(&self, request: DirectoryStatsRequest) -> bool {
        self.directory_stats.submit(request)
    }

    pub(crate) fn cancel_directory_stats(&self) {
        self.directory_stats.cancel_all();
    }

    pub(crate) fn submit_git_status(&self, request: GitStatusRequest) -> bool {
        self.git_status.submit(request)
    }

    pub(crate) fn submit_preview_line_count(&self, request: PreviewLineCountRequest) -> bool {
        self.preview_line_count.submit(request)
    }

    pub(crate) fn submit_image_prepare(&self, request: ImagePrepareRequest) -> bool {
        self.image_prepare
            .submit(request, ImageJobPriority::Current)
    }

    pub(crate) fn submit_nearby_image_prepare(&self, request: ImagePrepareRequest) -> bool {
        self.image_prepare.submit(request, ImageJobPriority::Nearby)
    }

    pub(crate) fn retain_image_prepares(
        &self,
        current: Option<&ImagePrepareRequest>,
        nearby: &[ImagePrepareRequest],
    ) {
        self.image_prepare.retain_pending(current, nearby);
    }

    pub(crate) fn submit_pdf_probe(
        &self,
        request: PdfProbeRequest,
        priority: PdfJobPriority,
    ) -> bool {
        self.pdf_probe.submit(request, priority)
    }

    pub(crate) fn submit_pdf_render(
        &self,
        request: PdfRenderRequest,
        priority: PdfJobPriority,
    ) -> bool {
        self.pdf_render.submit(request, priority)
    }

    pub(crate) fn clear_pending_pdf_jobs(&self) {
        self.pdf_probe.clear_pending();
        self.pdf_render.clear_pending();
    }

    pub(crate) fn retain_pdf_probe_pages(
        &self,
        path: &Path,
        size: u64,
        modified: Option<SystemTime>,
        keep_pages: &[usize],
    ) {
        self.pdf_probe
            .retain_pending(path, size, modified, keep_pages);
    }

    pub(crate) fn retain_pdf_render_variants(
        &self,
        path: &Path,
        size: u64,
        modified: Option<SystemTime>,
        keep_variants: &[(usize, u32, u32)],
    ) {
        self.pdf_render
            .retain_pending(path, size, modified, keep_variants);
    }

    pub(crate) fn submit_archive_create(&self, request: ArchiveCreateRequest) -> bool {
        self.archive_create.submit(request)
    }

    pub(crate) fn cancel_archive_create(&self, token: u64) {
        self.archive_create.cancel_create(token);
    }

    pub(crate) fn submit_archive_extract(&self, request: ArchiveExtractRequest) -> bool {
        self.archive_extract.submit(request)
    }

    pub(crate) fn cancel_archive_extract(&self, token: u64) {
        self.archive_extract.cancel_extract(token);
    }

    pub(crate) fn submit_paste(&self, request: PasteRequest) -> bool {
        self.paste.submit(request)
    }

    pub(crate) fn cancel_paste(&self, token: u64) {
        self.paste.cancel_paste(token);
    }

    pub(crate) fn submit_trash(&self, request: TrashRequest) -> bool {
        self.trash.submit(request)
    }

    pub(crate) fn cancel_trash(&self, token: u64) {
        self.trash.cancel_trash(token);
    }

    pub(crate) fn submit_restore(&self, request: RestoreRequest) -> bool {
        self.restore.submit(request)
    }

    pub(crate) fn cancel_restore(&self, token: u64) {
        self.restore.cancel_restore(token);
    }

    pub(crate) fn submit_search(&self, request: SearchRequest) -> bool {
        self.search.submit(request)
    }

    pub(crate) fn cancel_search(&self) {
        self.search.cancel_all();
    }

    pub(crate) fn submit_duplicate_scan(&self, request: DuplicateScanRequest) -> bool {
        self.duplicates.submit(request)
    }

    pub(crate) fn cancel_duplicate_scan(&self) {
        self.duplicates.cancel_all();
    }

    pub(crate) fn submit_preview(&self, request: PreviewRequest) -> bool {
        self.preview.submit(request)
    }

    pub(crate) fn try_recv(&self) -> Result<JobResult, mpsc::TryRecvError> {
        if let Some(job) = lock_unpoison(&self.buffered_results).pop_front() {
            return Ok(job);
        }
        self.result_rx.try_recv()
    }

    pub(crate) fn defer_result(&self, job: JobResult) {
        lock_unpoison(&self.buffered_results).push_front(job);
    }

    pub(crate) fn has_pending_work(&self) -> bool {
        !lock_unpoison(&self.buffered_results).is_empty()
            || self.directory.has_pending_work()
            || self.directory_fingerprint.has_pending_work()
            || self.archive_create.has_pending_work()
            || self.archive_extract.has_pending_work()
            || self.paste.has_pending_work()
            || self.trash.has_pending_work()
            || self.restore.has_pending_work()
            || self.directory_item_count.has_pending_work()
            || self.directory_stats.has_pending_work()
            || self.git_status.has_pending_work()
            || self.preview_line_count.has_pending_work()
            || self.image_prepare.has_pending_work()
            || self.pdf_probe.has_pending_work()
            || self.pdf_render.has_pending_work()
            || self.search.has_pending_work()
            || self.duplicates.has_pending_work()
            || self.preview.has_pending_work()
    }

    #[cfg(test)]
    pub(crate) fn metrics_snapshot(&self) -> SchedulerMetricsSnapshot {
        let mut snapshot = lock_unpoison(&self.metrics).snapshot();
        snapshot.preview_pending_high = self.preview.pending_len(PreviewPriority::High);
        snapshot.preview_pending_low = self.preview.pending_len(PreviewPriority::Low);
        snapshot.preview_active = self.preview.active_len();
        snapshot
    }

    #[cfg(test)]
    pub(in crate::background_jobs) fn new_for_tests(
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
    pub(in crate::background_jobs) fn snapshot(&self) -> SchedulerSnapshot {
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

    #[cfg(test)]
    pub(in crate::background_jobs) fn pop_next_pending_preview_for_tests(
        &self,
    ) -> Option<PreviewRequest> {
        self.preview.pop_next_pending_for_tests()
    }

    #[cfg(test)]
    pub(in crate::background_jobs) fn canceled_active_preview_keys_for_tests(
        &self,
    ) -> Vec<PreviewJobKey> {
        self.preview.canceled_active_keys()
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
pub(in crate::background_jobs) struct SchedulerSnapshot {
    pub(in crate::background_jobs) search_pending: Option<FuzzyFinderJobKey>,
    pub(in crate::background_jobs) search_active: Option<FuzzyFinderJobKey>,
    pub(in crate::background_jobs) image_prepare_pending: Vec<ImagePrepareJobKey>,
    pub(in crate::background_jobs) pdf_probe_pending: Vec<PdfProbeJobKey>,
    pub(in crate::background_jobs) pdf_render_pending: Vec<PdfRenderJobKey>,
    pub(in crate::background_jobs) preview_pending_high: Vec<PreviewJobKey>,
    pub(in crate::background_jobs) preview_pending_low: Vec<PreviewJobKey>,
    pub(in crate::background_jobs) preview_active: Vec<PreviewJobKey>,
}
