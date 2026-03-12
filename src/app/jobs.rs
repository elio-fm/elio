use super::pdf_preview::PdfProbeResult;
use super::*;
use crate::search::SearchCandidate;
use std::{
    collections::{HashSet, VecDeque},
    hash::Hash,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, MutexGuard, mpsc},
    thread,
    time::{Duration, Instant, SystemTime},
};

const PREVIEW_WORKER_COUNT: usize = 2;
const SEARCH_WORKER_COUNT: usize = 1;
const DIRECTORY_ITEM_COUNT_WORKER_COUNT: usize = 1;
const PDF_PROBE_WORKER_COUNT: usize = 1;
const PDF_RENDER_WORKER_COUNT: usize = 2;
const PREVIEW_QUEUE_LIMIT: usize = 8;
const DIRECTORY_ITEM_COUNT_QUEUE_LIMIT: usize = 48;
const PDF_PROBE_QUEUE_LIMIT: usize = 16;
const PDF_RENDER_QUEUE_LIMIT: usize = 8;

#[derive(Clone, Copy, Debug)]
struct SchedulerConfig {
    search_worker_count: usize,
    preview_worker_count: usize,
    preview_queue_limit: usize,
    directory_item_count_worker_count: usize,
    directory_item_count_queue_limit: usize,
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

#[derive(Debug)]
pub(super) struct SearchBuild {
    pub token: u64,
    pub cwd: PathBuf,
    pub scope: SearchScope,
    pub result: Result<Arc<Vec<SearchCandidate>>, String>,
}

#[derive(Clone, Debug)]
pub(super) struct SearchRequest {
    pub token: u64,
    pub cwd: PathBuf,
    pub scope: SearchScope,
}

#[derive(Debug)]
pub(super) struct DirectoryBuild {
    pub token: u64,
    pub cwd: PathBuf,
    pub result: Result<support::DirectorySnapshot, String>,
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

#[derive(Clone, Debug)]
pub(super) struct DirectoryItemCountRequest {
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
    pub show_hidden: bool,
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
    pub result: preview::PreviewContent,
}

#[derive(Clone, Debug)]
pub(super) struct PreviewRequest {
    pub token: u64,
    pub entry: Entry,
    pub priority: PreviewPriority,
}

#[derive(Debug)]
pub(super) enum JobResult {
    Directory(DirectoryBuild),
    DirectoryItemCount(DirectoryItemCountBuild),
    PdfProbe(PdfProbeBuild),
    PdfRender(PdfRenderBuild),
    Search(SearchBuild),
    Preview(Box<PreviewBuild>),
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
    directory_item_count: DirectoryItemCountPool,
    pdf_probe: PdfProbePool,
    pdf_render: PdfRenderPool,
    search: SearchPool,
    preview: PreviewPool,
    result_rx: mpsc::Receiver<JobResult>,
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
            directory_item_count: DirectoryItemCountPool::new(
                config.directory_item_count_worker_count,
                config.directory_item_count_queue_limit,
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
            #[cfg(test)]
            metrics,
        }
    }

    pub(super) fn submit_directory(&self, request: DirectoryRequest) -> bool {
        self.directory.submit(request)
    }

    pub(super) fn submit_directory_item_count(&self, request: DirectoryItemCountRequest) -> bool {
        self.directory_item_count.submit(request)
    }

    pub(super) fn submit_pdf_probe(&self, request: PdfProbeRequest) -> bool {
        self.pdf_probe.submit(request)
    }

    pub(super) fn submit_pdf_render(&self, request: PdfRenderRequest) -> bool {
        self.pdf_render.submit(request)
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

    pub(super) fn submit_search(&self, request: SearchRequest) -> bool {
        self.search.submit(request)
    }

    pub(super) fn submit_preview(&self, request: PreviewRequest) -> bool {
        self.preview.submit(request)
    }

    pub(super) fn try_recv(&self) -> Result<JobResult, mpsc::TryRecvError> {
        self.result_rx.try_recv()
    }

    pub(super) fn has_pending_work(&self) -> bool {
        self.directory.has_pending_work()
            || self.directory_item_count.has_pending_work()
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
            pdf_probe_pending: self.pdf_probe.pending_keys(),
            pdf_render_pending: self.pdf_render.pending_keys(),
            preview_pending_high: self.preview.pending_keys(PreviewPriority::High),
            preview_pending_low: self.preview.pending_keys(PreviewPriority::Low),
            preview_active: self.preview.active_keys(),
        }
    }
}

#[derive(Default)]
struct SchedulerMetrics {
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
    fn record_directory_completed(&mut self, _elapsed: Duration) {
        self.directory_jobs_completed += 1;
    }

    fn record_search_completed(&mut self, _elapsed: Duration) {
        self.search_jobs_completed += 1;
    }

    fn record_preview_completed(&mut self, elapsed: Duration) {
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

struct DirectoryPool {
    shared: Arc<DirectoryShared>,
    workers: Vec<thread::JoinHandle<()>>,
    metrics: Arc<Mutex<SchedulerMetrics>>,
}

struct DirectoryShared {
    state: Mutex<DirectoryState>,
    available: Condvar,
}

struct DirectoryState {
    pending: Option<DirectoryRequest>,
    pending_key: Option<DirectoryJobKey>,
    active_key: Option<DirectoryJobKey>,
    closed: bool,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DirectoryJobKey {
    cwd: PathBuf,
    show_hidden: bool,
    sort_mode: SortMode,
}

impl DirectoryPool {
    fn new(
        worker_count: usize,
        result_tx: mpsc::Sender<JobResult>,
        metrics: Arc<Mutex<SchedulerMetrics>>,
    ) -> Self {
        let shared = Arc::new(DirectoryShared {
            state: Mutex::new(DirectoryState {
                pending: None,
                pending_key: None,
                active_key: None,
                closed: false,
            }),
            available: Condvar::new(),
        });
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let shared = Arc::clone(&shared);
            let result_tx = result_tx.clone();
            let metrics = Arc::clone(&metrics);
            workers.push(thread::spawn(move || {
                while let Some(request) = DirectoryShared::pop(&shared) {
                    let key = DirectoryJobKey::from_request(&request);
                    let started_at = Instant::now();
                    let result = support::load_directory_snapshot(
                        &request.cwd,
                        request.show_hidden,
                        request.sort_mode,
                    )
                    .map_err(|error| {
                        error
                            .downcast_ref::<std::io::Error>()
                            .map(support::describe_io_error)
                            .unwrap_or("Read error")
                            .to_string()
                    });
                    DirectoryShared::finish(&shared, &key);
                    lock_unpoison(&metrics).record_directory_completed(started_at.elapsed());
                    if result_tx
                        .send(JobResult::Directory(DirectoryBuild {
                            token: request.token,
                            cwd: request.cwd,
                            result,
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        Self {
            shared,
            workers,
            metrics,
        }
    }

    fn submit(&self, request: DirectoryRequest) -> bool {
        let key = DirectoryJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        if state.pending_key.as_ref() == Some(&key) {
            state.pending = Some(request);
            self.shared.available.notify_one();
            return true;
        }
        state.pending = Some(request);
        state.pending_key = Some(key);
        lock_unpoison(&self.metrics).directory_jobs_submitted += 1;
        self.shared.available.notify_one();
        true
    }

    fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active_key.is_some()
    }
}

impl Drop for DirectoryPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending = None;
            state.pending_key = None;
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl DirectoryShared {
    fn pop(shared: &Arc<Self>) -> Option<DirectoryRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending.take() {
                state.active_key = state.pending_key.take();
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &DirectoryJobKey) {
        let mut state = lock_unpoison(&shared.state);
        if state.active_key.as_ref() == Some(key) {
            state.active_key = None;
        }
    }
}

impl DirectoryJobKey {
    fn from_request(request: &DirectoryRequest) -> Self {
        Self {
            cwd: request.cwd.clone(),
            show_hidden: request.show_hidden,
            sort_mode: request.sort_mode,
        }
    }
}

struct DirectoryItemCountPool {
    shared: Arc<DirectoryItemCountShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct DirectoryItemCountShared {
    state: Mutex<DirectoryItemCountState>,
    available: Condvar,
}

struct DirectoryItemCountState {
    pending: VecDeque<DirectoryItemCountRequest>,
    queued_keys: HashSet<DirectoryItemCountJobKey>,
    active_keys: HashSet<DirectoryItemCountJobKey>,
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DirectoryItemCountJobKey {
    path: PathBuf,
    modified: Option<SystemTime>,
    show_hidden: bool,
}

impl DirectoryItemCountPool {
    fn new(worker_count: usize, capacity: usize, result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(DirectoryItemCountShared {
            state: Mutex::new(DirectoryItemCountState {
                pending: VecDeque::new(),
                queued_keys: HashSet::new(),
                active_keys: HashSet::new(),
                closed: false,
                capacity,
            }),
            available: Condvar::new(),
        });
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let shared = Arc::clone(&shared);
            let result_tx = result_tx.clone();
            workers.push(thread::spawn(move || {
                while let Some(request) = DirectoryItemCountShared::pop(&shared) {
                    let key = DirectoryItemCountJobKey::from_request(&request);
                    let item_count =
                        support::count_directory_items(&request.path, request.show_hidden).ok();
                    DirectoryItemCountShared::finish(&shared, &key);
                    if result_tx
                        .send(JobResult::DirectoryItemCount(DirectoryItemCountBuild {
                            path: request.path,
                            modified: request.modified,
                            show_hidden: request.show_hidden,
                            item_count,
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        Self { shared, workers }
    }

    fn submit(&self, request: DirectoryItemCountRequest) -> bool {
        let key = DirectoryItemCountJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        if state.queued_keys.contains(&key) || state.active_keys.contains(&key) {
            return true;
        }
        while state.pending.len() >= state.capacity {
            let Some(stale) = state.pending.pop_front() else {
                break;
            };
            state
                .queued_keys
                .remove(&DirectoryItemCountJobKey::from_request(&stale));
        }
        state.queued_keys.insert(key);
        state.pending.push_back(request);
        self.shared.available.notify_one();
        true
    }

    fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        !state.pending.is_empty() || !state.active_keys.is_empty()
    }
}

impl Drop for DirectoryItemCountPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending.clear();
            state.queued_keys.clear();
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl DirectoryItemCountShared {
    fn pop(shared: &Arc<Self>) -> Option<DirectoryItemCountRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending.pop_front() {
                let key = DirectoryItemCountJobKey::from_request(&request);
                state.queued_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &DirectoryItemCountJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active_keys.remove(key);
    }
}

impl DirectoryItemCountJobKey {
    fn from_request(request: &DirectoryItemCountRequest) -> Self {
        Self {
            path: request.path.clone(),
            modified: request.modified,
            show_hidden: request.show_hidden,
        }
    }
}

struct PdfProbePool {
    shared: Arc<PdfProbeShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct PdfProbeShared {
    state: Mutex<PdfProbeState>,
    available: Condvar,
}

struct PdfProbeState {
    pending: VecDeque<PdfProbeRequest>,
    queued_keys: HashSet<PdfProbeJobKey>,
    active_keys: HashSet<PdfProbeJobKey>,
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PdfProbeJobKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    page: usize,
}

impl PdfProbePool {
    fn new(worker_count: usize, capacity: usize, result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(PdfProbeShared {
            state: Mutex::new(PdfProbeState {
                pending: VecDeque::new(),
                queued_keys: HashSet::new(),
                active_keys: HashSet::new(),
                closed: false,
                capacity,
            }),
            available: Condvar::new(),
        });
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let shared = Arc::clone(&shared);
            let result_tx = result_tx.clone();
            workers.push(thread::spawn(move || {
                while let Some(request) = PdfProbeShared::pop(&shared) {
                    let key = PdfProbeJobKey::from_request(&request);
                    let result = pdf_preview::probe_pdf_page(&request.path, request.page)
                        .map_err(|error| error.to_string());
                    PdfProbeShared::finish(&shared, &key);
                    if result_tx
                        .send(JobResult::PdfProbe(PdfProbeBuild {
                            path: request.path,
                            size: request.size,
                            modified: request.modified,
                            page: request.page,
                            result,
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        Self { shared, workers }
    }

    fn submit(&self, request: PdfProbeRequest) -> bool {
        let key = PdfProbeJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        if state.queued_keys.contains(&key) || state.active_keys.contains(&key) {
            return true;
        }
        while state.pending.len() >= state.capacity {
            let Some(stale) = state.pending.pop_front() else {
                break;
            };
            state
                .queued_keys
                .remove(&PdfProbeJobKey::from_request(&stale));
        }
        state.queued_keys.insert(key);
        state.pending.push_back(request);
        self.shared.available.notify_one();
        true
    }

    fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        !state.pending.is_empty() || !state.active_keys.is_empty()
    }

    #[cfg(test)]
    fn pending_keys(&self) -> Vec<PdfProbeJobKey> {
        let state = lock_unpoison(&self.shared.state);
        state
            .pending
            .iter()
            .map(PdfProbeJobKey::from_request)
            .collect()
    }

    fn clear_pending(&self) {
        let mut state = lock_unpoison(&self.shared.state);
        state.pending.clear();
        state.queued_keys.clear();
    }

    fn retain_pending(
        &self,
        path: &Path,
        size: u64,
        modified: Option<SystemTime>,
        keep_pages: &[usize],
    ) {
        let mut state = lock_unpoison(&self.shared.state);
        let mut retained = VecDeque::with_capacity(state.pending.len());
        state.queued_keys.clear();
        while let Some(request) = state.pending.pop_front() {
            let keep = request.path == path
                && request.size == size
                && request.modified == modified
                && keep_pages.contains(&request.page);
            if keep {
                state
                    .queued_keys
                    .insert(PdfProbeJobKey::from_request(&request));
                retained.push_back(request);
            }
        }
        state.pending = retained;
    }
}

impl Drop for PdfProbePool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending.clear();
            state.queued_keys.clear();
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl PdfProbeShared {
    fn pop(shared: &Arc<Self>) -> Option<PdfProbeRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending.pop_front() {
                let key = PdfProbeJobKey::from_request(&request);
                state.queued_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &PdfProbeJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active_keys.remove(key);
    }
}

impl PdfProbeJobKey {
    fn from_request(request: &PdfProbeRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
        }
    }
}

struct PdfRenderPool {
    shared: Arc<PdfRenderShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct PdfRenderShared {
    state: Mutex<PdfRenderState>,
    available: Condvar,
}

struct PdfRenderState {
    pending: VecDeque<PdfRenderRequest>,
    queued_keys: HashSet<PdfRenderJobKey>,
    active_keys: HashSet<PdfRenderJobKey>,
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PdfRenderJobKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    page: usize,
    width_px: u32,
    height_px: u32,
}

impl PdfRenderPool {
    fn new(worker_count: usize, capacity: usize, result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(PdfRenderShared {
            state: Mutex::new(PdfRenderState {
                pending: VecDeque::new(),
                queued_keys: HashSet::new(),
                active_keys: HashSet::new(),
                closed: false,
                capacity,
            }),
            available: Condvar::new(),
        });
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let shared = Arc::clone(&shared);
            let result_tx = result_tx.clone();
            workers.push(thread::spawn(move || {
                while let Some(request) = PdfRenderShared::pop(&shared) {
                    let key = PdfRenderJobKey::from_request(&request);
                    let result = pdf_preview::render_pdf_page_to_cache(
                        &request.path,
                        request.size,
                        request.modified,
                        request.page,
                        request.width_px,
                        request.height_px,
                    )
                    .map_err(|error| error.to_string());
                    PdfRenderShared::finish(&shared, &key);
                    if result_tx
                        .send(JobResult::PdfRender(PdfRenderBuild {
                            path: request.path,
                            size: request.size,
                            modified: request.modified,
                            page: request.page,
                            width_px: request.width_px,
                            height_px: request.height_px,
                            result,
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        Self { shared, workers }
    }

    fn submit(&self, request: PdfRenderRequest) -> bool {
        let key = PdfRenderJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        if state.queued_keys.contains(&key) || state.active_keys.contains(&key) {
            return true;
        }
        while state.pending.len() >= state.capacity {
            let Some(stale) = state.pending.pop_front() else {
                break;
            };
            state
                .queued_keys
                .remove(&PdfRenderJobKey::from_request(&stale));
        }
        state.queued_keys.insert(key);
        state.pending.push_back(request);
        self.shared.available.notify_one();
        true
    }

    fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        !state.pending.is_empty() || !state.active_keys.is_empty()
    }

    #[cfg(test)]
    fn pending_keys(&self) -> Vec<PdfRenderJobKey> {
        let state = lock_unpoison(&self.shared.state);
        state
            .pending
            .iter()
            .map(PdfRenderJobKey::from_request)
            .collect()
    }

    fn clear_pending(&self) {
        let mut state = lock_unpoison(&self.shared.state);
        state.pending.clear();
        state.queued_keys.clear();
    }

    fn retain_pending(
        &self,
        path: &Path,
        size: u64,
        modified: Option<SystemTime>,
        keep_variants: &[(usize, u32, u32)],
    ) {
        let mut state = lock_unpoison(&self.shared.state);
        let mut retained = VecDeque::with_capacity(state.pending.len());
        state.queued_keys.clear();
        while let Some(request) = state.pending.pop_front() {
            let keep = request.path == path
                && request.size == size
                && request.modified == modified
                && keep_variants.contains(&(request.page, request.width_px, request.height_px));
            if keep {
                state
                    .queued_keys
                    .insert(PdfRenderJobKey::from_request(&request));
                retained.push_back(request);
            }
        }
        state.pending = retained;
    }
}

impl Drop for PdfRenderPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending.clear();
            state.queued_keys.clear();
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl PdfRenderShared {
    fn pop(shared: &Arc<Self>) -> Option<PdfRenderRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending.pop_front() {
                let key = PdfRenderJobKey::from_request(&request);
                state.queued_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &PdfRenderJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active_keys.remove(key);
    }
}

impl PdfRenderJobKey {
    fn from_request(request: &PdfRenderRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
            width_px: request.width_px,
            height_px: request.height_px,
        }
    }
}

struct SearchPool {
    shared: Arc<SearchShared>,
    workers: Vec<thread::JoinHandle<()>>,
    metrics: Arc<Mutex<SchedulerMetrics>>,
}

struct SearchShared {
    state: Mutex<SearchState>,
    available: Condvar,
}

struct SearchState {
    pending: Option<SearchRequest>,
    pending_key: Option<SearchJobKey>,
    active_key: Option<SearchJobKey>,
    closed: bool,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct SearchJobKey {
    cwd: PathBuf,
    scope: SearchScope,
}

impl SearchPool {
    fn new(
        worker_count: usize,
        result_tx: mpsc::Sender<JobResult>,
        metrics: Arc<Mutex<SchedulerMetrics>>,
    ) -> Self {
        let shared = Arc::new(SearchShared {
            state: Mutex::new(SearchState {
                pending: None,
                pending_key: None,
                active_key: None,
                closed: false,
            }),
            available: Condvar::new(),
        });
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let shared = Arc::clone(&shared);
            let result_tx = result_tx.clone();
            let metrics = Arc::clone(&metrics);
            workers.push(thread::spawn(move || {
                while let Some(request) = SearchShared::pop(&shared) {
                    let key = SearchJobKey::from_request(&request);
                    let started_at = Instant::now();
                    let result = crate::search::collect_candidates(
                        &request.cwd,
                        true,
                        request.scope.candidate_scope(),
                    )
                    .map(Arc::new)
                    .map_err(|error| error.to_string());
                    SearchShared::finish(&shared, &key);
                    lock_unpoison(&metrics).record_search_completed(started_at.elapsed());
                    if result_tx
                        .send(JobResult::Search(SearchBuild {
                            token: request.token,
                            cwd: request.cwd,
                            scope: request.scope,
                            result,
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        Self {
            shared,
            workers,
            metrics,
        }
    }

    fn submit(&self, request: SearchRequest) -> bool {
        let key = SearchJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        if state.pending_key.as_ref() == Some(&key) {
            state.pending = Some(request);
            self.shared.available.notify_one();
            return true;
        }
        state.pending = Some(request);
        state.pending_key = Some(key);
        lock_unpoison(&self.metrics).search_jobs_submitted += 1;
        self.shared.available.notify_one();
        true
    }

    fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active_key.is_some()
    }

    #[cfg(test)]
    fn pending_key(&self) -> Option<SearchJobKey> {
        lock_unpoison(&self.shared.state).pending_key.clone()
    }

    #[cfg(test)]
    fn active_key(&self) -> Option<SearchJobKey> {
        lock_unpoison(&self.shared.state).active_key.clone()
    }
}

impl Drop for SearchPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending = None;
            state.pending_key = None;
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl SearchShared {
    fn pop(shared: &Arc<Self>) -> Option<SearchRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending.take() {
                state.active_key = state.pending_key.take();
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &SearchJobKey) {
        let mut state = lock_unpoison(&shared.state);
        if state.active_key.as_ref() == Some(key) {
            state.active_key = None;
        }
    }
}

impl SearchJobKey {
    fn from_request(request: &SearchRequest) -> Self {
        Self {
            cwd: request.cwd.clone(),
            scope: request.scope,
        }
    }
}

struct PreviewPool {
    shared: Arc<PreviewShared>,
    workers: Vec<thread::JoinHandle<()>>,
    metrics: Arc<Mutex<SchedulerMetrics>>,
}

struct PreviewShared {
    state: Mutex<PreviewState>,
    available: Condvar,
}

struct PreviewState {
    pending_high: VecDeque<PreviewRequest>,
    pending_low: VecDeque<PreviewRequest>,
    queued_high_keys: HashSet<PreviewJobKey>,
    queued_low_keys: HashSet<PreviewJobKey>,
    active_keys: HashSet<PreviewJobKey>,
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PreviewJobKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

impl PreviewPool {
    fn new(
        worker_count: usize,
        capacity: usize,
        result_tx: mpsc::Sender<JobResult>,
        metrics: Arc<Mutex<SchedulerMetrics>>,
    ) -> Self {
        let shared = Arc::new(PreviewShared {
            state: Mutex::new(PreviewState {
                pending_high: VecDeque::new(),
                pending_low: VecDeque::new(),
                queued_high_keys: HashSet::new(),
                queued_low_keys: HashSet::new(),
                active_keys: HashSet::new(),
                closed: false,
                capacity,
            }),
            available: Condvar::new(),
        });
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let shared = Arc::clone(&shared);
            let result_tx = result_tx.clone();
            let metrics = Arc::clone(&metrics);
            workers.push(thread::spawn(move || {
                while let Some(request) = PreviewShared::pop(&shared) {
                    let key = PreviewJobKey::from_entry(&request.entry);
                    let started_at = Instant::now();
                    let result = preview::build_preview(&request.entry);
                    PreviewShared::finish(&shared, &key);
                    lock_unpoison(&metrics).record_preview_completed(started_at.elapsed());
                    if result_tx
                        .send(JobResult::Preview(Box::new(PreviewBuild {
                            token: request.token,
                            entry: request.entry,
                            result,
                        })))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        Self {
            shared,
            workers,
            metrics,
        }
    }

    fn submit(&self, request: PreviewRequest) -> bool {
        let key = PreviewJobKey::from_entry(&request.entry);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        match request.priority {
            PreviewPriority::High => {
                if state.queued_high_keys.contains(&key) {
                    replace_preview_request(&mut state.pending_high, request, &key);
                    return true;
                }
                if state.queued_low_keys.remove(&key) {
                    remove_preview_request(&mut state.pending_low, &key);
                    lock_unpoison(&self.metrics).preview_promotions += 1;
                }
                let evicted = trim_preview_queue_for_high(&mut state);
                state.queued_high_keys.insert(key);
                state.pending_high.push_back(request);
                let mut metrics = lock_unpoison(&self.metrics);
                metrics.preview_jobs_submitted_high += 1;
                metrics.preview_low_priority_evictions += evicted;
            }
            PreviewPriority::Low => {
                if state.queued_high_keys.contains(&key) {
                    replace_preview_request_with_priority(
                        &mut state.pending_high,
                        request,
                        &key,
                        PreviewPriority::High,
                    );
                    return true;
                }
                if state.queued_low_keys.contains(&key) {
                    replace_preview_request(&mut state.pending_low, request, &key);
                    return true;
                }
                let evicted = if preview_pending_len(&state) >= state.capacity {
                    u64::from(evict_oldest_low_priority_preview(&mut state))
                } else {
                    0
                };
                if preview_pending_len(&state) >= state.capacity && evicted == 0 {
                    return true;
                }
                state.queued_low_keys.insert(key);
                state.pending_low.push_back(request);
                let mut metrics = lock_unpoison(&self.metrics);
                metrics.preview_jobs_submitted_low += 1;
                metrics.preview_low_priority_evictions += evicted;
            }
        }
        self.shared.available.notify_one();
        true
    }

    fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        !state.pending_high.is_empty()
            || !state.pending_low.is_empty()
            || !state.active_keys.is_empty()
    }

    #[cfg(test)]
    fn pending_keys(&self, priority: PreviewPriority) -> Vec<PreviewJobKey> {
        let state = lock_unpoison(&self.shared.state);
        let queue = match priority {
            PreviewPriority::High => &state.pending_high,
            PreviewPriority::Low => &state.pending_low,
        };
        queue
            .iter()
            .map(|request| PreviewJobKey::from_entry(&request.entry))
            .collect()
    }

    #[cfg(test)]
    fn active_keys(&self) -> Vec<PreviewJobKey> {
        let mut keys = lock_unpoison(&self.shared.state)
            .active_keys
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.path.cmp(&right.path));
        keys
    }

    #[cfg(test)]
    fn pending_len(&self, priority: PreviewPriority) -> usize {
        let state = lock_unpoison(&self.shared.state);
        match priority {
            PreviewPriority::High => state.pending_high.len(),
            PreviewPriority::Low => state.pending_low.len(),
        }
    }

    #[cfg(test)]
    fn active_len(&self) -> usize {
        lock_unpoison(&self.shared.state).active_keys.len()
    }
}

impl Drop for PreviewPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending_high.clear();
            state.pending_low.clear();
            state.queued_high_keys.clear();
            state.queued_low_keys.clear();
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl PreviewShared {
    fn pop(shared: &Arc<Self>) -> Option<PreviewRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending_high.pop_front() {
                let key = PreviewJobKey::from_entry(&request.entry);
                state.queued_high_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            if let Some(request) = state.pending_low.pop_front() {
                let key = PreviewJobKey::from_entry(&request.entry);
                state.queued_low_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &PreviewJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active_keys.remove(key);
    }
}

impl PreviewJobKey {
    fn from_entry(entry: &Entry) -> Self {
        Self {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
        }
    }
}

fn remove_preview_request(queue: &mut VecDeque<PreviewRequest>, key: &PreviewJobKey) {
    if let Some(index) = queue
        .iter()
        .position(|request| PreviewJobKey::from_entry(&request.entry) == *key)
    {
        queue.remove(index);
    }
}

fn replace_preview_request(
    queue: &mut VecDeque<PreviewRequest>,
    request: PreviewRequest,
    key: &PreviewJobKey,
) {
    let priority = request.priority;
    replace_preview_request_with_priority(queue, request, key, priority);
}

fn replace_preview_request_with_priority(
    queue: &mut VecDeque<PreviewRequest>,
    mut request: PreviewRequest,
    key: &PreviewJobKey,
    priority: PreviewPriority,
) {
    request.priority = priority;
    if let Some(index) = queue
        .iter()
        .position(|queued| PreviewJobKey::from_entry(&queued.entry) == *key)
    {
        queue[index] = request;
    }
}

fn preview_pending_len(state: &PreviewState) -> usize {
    state.pending_high.len() + state.pending_low.len()
}

fn evict_oldest_low_priority_preview(state: &mut PreviewState) -> bool {
    let Some(stale) = state.pending_low.pop_front() else {
        return false;
    };
    state
        .queued_low_keys
        .remove(&PreviewJobKey::from_entry(&stale.entry));
    true
}

fn trim_preview_queue_for_high(state: &mut PreviewState) -> u64 {
    let mut evicted = 0;
    while preview_pending_len(state) >= state.capacity {
        if evict_oldest_low_priority_preview(state) {
            evicted += 1;
            continue;
        }

        let Some(stale) = state.pending_high.pop_front() else {
            break;
        };
        state
            .queued_high_keys
            .remove(&PreviewJobKey::from_entry(&stale.entry));
    }
    evicted
}

fn lock_unpoison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn wait_unpoison<'a, T>(condvar: &Condvar, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
    condvar
        .wait(guard)
        .unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
struct SchedulerSnapshot {
    search_pending: Option<SearchJobKey>,
    search_active: Option<SearchJobKey>,
    pdf_probe_pending: Vec<PdfProbeJobKey>,
    pdf_render_pending: Vec<PdfRenderJobKey>,
    preview_pending_high: Vec<PreviewJobKey>,
    preview_pending_low: Vec<PreviewJobKey>,
    preview_active: Vec<PreviewJobKey>,
}

#[cfg(test)]
mod tests;
