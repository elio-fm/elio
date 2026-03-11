use super::*;
use crate::search::SearchCandidate;
use std::{
    collections::{HashSet, VecDeque},
    hash::Hash,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, MutexGuard, mpsc},
    thread,
    time::{Duration, Instant, SystemTime},
};

const PREVIEW_WORKER_COUNT: usize = 2;
const SEARCH_WORKER_COUNT: usize = 1;
const PREVIEW_QUEUE_LIMIT: usize = 8;

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
    search: SearchPool,
    preview: PreviewPool,
    result_rx: mpsc::Receiver<JobResult>,
    #[cfg(test)]
    metrics: Arc<Mutex<SchedulerMetrics>>,
}

impl JobScheduler {
    pub(super) fn new() -> Self {
        Self::with_config(
            SEARCH_WORKER_COUNT,
            PREVIEW_WORKER_COUNT,
            PREVIEW_QUEUE_LIMIT,
        )
    }

    fn with_config(
        search_worker_count: usize,
        preview_worker_count: usize,
        preview_queue_limit: usize,
    ) -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        let metrics = Arc::new(Mutex::new(SchedulerMetrics::default()));
        Self {
            directory: DirectoryPool::new(1, result_tx.clone(), Arc::clone(&metrics)),
            search: SearchPool::new(search_worker_count, result_tx.clone(), Arc::clone(&metrics)),
            preview: PreviewPool::new(
                preview_worker_count,
                preview_queue_limit,
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
        Self::with_config(
            search_worker_count,
            preview_worker_count,
            preview_queue_limit,
        )
    }

    #[cfg(test)]
    fn snapshot(&self) -> SchedulerSnapshot {
        SchedulerSnapshot {
            search_pending: self.search.pending_key(),
            search_active: self.search.active_key(),
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
    preview_pending_high: Vec<PreviewJobKey>,
    preview_pending_low: Vec<PreviewJobKey>,
    preview_active: Vec<PreviewJobKey>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_pool_deduplicates_identical_active_or_queued_requests() {
        let scheduler = JobScheduler::new_for_tests(0, 0, 8);
        let entry = Entry {
            path: PathBuf::from("archive.zip"),
            name: "archive.zip".to_string(),
            name_key: "archive.zip".to_string(),
            kind: EntryKind::File,
            size: 42,
            modified: None,
            readonly: false,
        };

        assert!(scheduler.submit_preview(PreviewRequest {
            token: 1,
            entry: entry.clone(),
            priority: PreviewPriority::Low,
        }));
        assert!(scheduler.submit_preview(PreviewRequest {
            token: 2,
            entry,
            priority: PreviewPriority::Low,
        }));
        let snapshot = scheduler.snapshot();
        assert!(snapshot.preview_pending_high.is_empty());
        assert_eq!(
            snapshot.preview_pending_low,
            vec![PreviewJobKey {
                path: PathBuf::from("archive.zip"),
                size: 42,
                modified: None,
            }]
        );
        assert!(snapshot.preview_active.is_empty());
    }

    #[test]
    fn search_pool_replaces_pending_request_with_latest_distinct_job() {
        let scheduler = JobScheduler::new_for_tests(0, 0, 8);

        assert!(scheduler.submit_search(SearchRequest {
            token: 1,
            cwd: PathBuf::from("/tmp/a"),
            scope: SearchScope::Files,
        }));
        assert!(scheduler.submit_search(SearchRequest {
            token: 2,
            cwd: PathBuf::from("/tmp/b"),
            scope: SearchScope::Files,
        }));
        assert_eq!(
            scheduler.snapshot().search_pending,
            Some(SearchJobKey {
                cwd: PathBuf::from("/tmp/b"),
                scope: SearchScope::Files,
            })
        );
    }

    #[test]
    fn preview_pool_discards_oldest_queued_request_when_full() {
        let scheduler = JobScheduler::new_for_tests(0, 0, 2);

        for name in ["a.zip", "b.zip", "c.zip"] {
            assert!(scheduler.submit_preview(PreviewRequest {
                token: 1,
                entry: Entry {
                    path: PathBuf::from(name),
                    name: name.to_string(),
                    name_key: name.to_string(),
                    kind: EntryKind::File,
                    size: 1,
                    modified: None,
                    readonly: false,
                },
                priority: PreviewPriority::Low,
            }));
        }

        assert!(scheduler.snapshot().preview_pending_high.is_empty());
        assert_eq!(
            scheduler.snapshot().preview_pending_low,
            vec![
                PreviewJobKey {
                    path: PathBuf::from("b.zip"),
                    size: 1,
                    modified: None,
                },
                PreviewJobKey {
                    path: PathBuf::from("c.zip"),
                    size: 1,
                    modified: None,
                },
            ]
        );
    }

    #[test]
    fn high_priority_preview_promotes_over_low_priority_duplicate() {
        let scheduler = JobScheduler::new_for_tests(0, 0, 4);
        let entry = Entry {
            path: PathBuf::from("archive.zip"),
            name: "archive.zip".to_string(),
            name_key: "archive.zip".to_string(),
            kind: EntryKind::File,
            size: 42,
            modified: None,
            readonly: false,
        };

        assert!(scheduler.submit_preview(PreviewRequest {
            token: 1,
            entry: entry.clone(),
            priority: PreviewPriority::Low,
        }));
        assert!(scheduler.submit_preview(PreviewRequest {
            token: 2,
            entry,
            priority: PreviewPriority::High,
        }));

        let snapshot = scheduler.snapshot();
        assert!(snapshot.preview_pending_low.is_empty());
        assert_eq!(
            snapshot.preview_pending_high,
            vec![PreviewJobKey {
                path: PathBuf::from("archive.zip"),
                size: 42,
                modified: None,
            }]
        );
        assert_eq!(scheduler.metrics_snapshot().preview_promotions, 1);
    }

    #[test]
    fn low_priority_preview_does_not_displace_full_high_priority_queue() {
        let scheduler = JobScheduler::new_for_tests(0, 0, 1);

        assert!(scheduler.submit_preview(PreviewRequest {
            token: 1,
            entry: Entry {
                path: PathBuf::from("a.zip"),
                name: "a.zip".to_string(),
                name_key: "a.zip".to_string(),
                kind: EntryKind::File,
                size: 1,
                modified: None,
                readonly: false,
            },
            priority: PreviewPriority::High,
        }));
        assert!(scheduler.submit_preview(PreviewRequest {
            token: 2,
            entry: Entry {
                path: PathBuf::from("b.zip"),
                name: "b.zip".to_string(),
                name_key: "b.zip".to_string(),
                kind: EntryKind::File,
                size: 1,
                modified: None,
                readonly: false,
            },
            priority: PreviewPriority::Low,
        }));

        let snapshot = scheduler.snapshot();
        assert_eq!(
            snapshot.preview_pending_high,
            vec![PreviewJobKey {
                path: PathBuf::from("a.zip"),
                size: 1,
                modified: None,
            }]
        );
        assert!(snapshot.preview_pending_low.is_empty());
        assert_eq!(
            scheduler.metrics_snapshot().preview_low_priority_evictions,
            0
        );
    }

    #[test]
    fn low_priority_preview_eviction_updates_metrics() {
        let scheduler = JobScheduler::new_for_tests(0, 0, 1);

        for name in ["a.zip", "b.zip"] {
            assert!(scheduler.submit_preview(PreviewRequest {
                token: 1,
                entry: Entry {
                    path: PathBuf::from(name),
                    name: name.to_string(),
                    name_key: name.to_string(),
                    kind: EntryKind::File,
                    size: 1,
                    modified: None,
                    readonly: false,
                },
                priority: PreviewPriority::Low,
            }));
        }

        let metrics = scheduler.metrics_snapshot();
        assert_eq!(metrics.preview_jobs_submitted_low, 2);
        assert_eq!(metrics.preview_low_priority_evictions, 1);
    }

    #[test]
    fn scheduler_reports_pending_work_when_jobs_are_queued() {
        let scheduler = JobScheduler::new_for_tests(0, 0, 2);
        assert!(!scheduler.has_pending_work());

        assert!(scheduler.submit_search(SearchRequest {
            token: 1,
            cwd: PathBuf::from("/tmp/a"),
            scope: SearchScope::Files,
        }));
        assert!(scheduler.has_pending_work());
    }
}
