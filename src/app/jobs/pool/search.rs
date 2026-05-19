use super::*;
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};

pub(in crate::app::jobs) struct SearchPool {
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
    active: Option<ActiveSearchJob>,
    closed: bool,
}

#[derive(Clone, Debug)]
struct ActiveSearchJob {
    key: SearchJobKey,
    canceled: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app::jobs) struct SearchJobKey {
    pub(in crate::app::jobs) cwd: PathBuf,
    pub(in crate::app::jobs) scope: SearchScope,
    pub(in crate::app::jobs) show_hidden: bool,
    pub(in crate::app::jobs) fingerprint: crate::fs::DirectoryFingerprint,
}

impl SearchPool {
    pub(in crate::app::jobs) fn new(
        worker_count: usize,
        result_tx: mpsc::Sender<JobResult>,
        metrics: Arc<Mutex<SchedulerMetrics>>,
    ) -> Self {
        let shared = Arc::new(SearchShared {
            state: Mutex::new(SearchState {
                pending: None,
                pending_key: None,
                active: None,
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
                while let Some((request, canceled)) = SearchShared::pop(&shared) {
                    let key = SearchJobKey::from_request(&request);
                    let started_at = Instant::now();
                    let progress_cwd = request.cwd.clone();
                    let progress_scope = request.scope;
                    let progress_show_hidden = request.show_hidden;
                    let progress_fingerprint = request.fingerprint;
                    let progress_token = request.token;
                    let mut progress_send_failed = false;
                    let result = crate::fs::search::collect_candidates_streaming(
                        &request.cwd,
                        request.show_hidden,
                        request.scope.candidate_scope(),
                        || canceled.load(Ordering::Relaxed),
                        |batch| {
                            if result_tx
                                .send(JobResult::SearchBatch(SearchBatchBuild {
                                    token: progress_token,
                                    cwd: progress_cwd.clone(),
                                    scope: progress_scope,
                                    show_hidden: progress_show_hidden,
                                    fingerprint: progress_fingerprint,
                                    batch,
                                }))
                                .is_err()
                            {
                                progress_send_failed = true;
                                return false;
                            }
                            true
                        },
                    )
                    .map_err(|error| error.to_string());
                    if progress_send_failed {
                        SearchShared::finish(&shared, &key);
                        break;
                    }
                    SearchShared::finish(&shared, &key);
                    lock_unpoison(&metrics).record_search_completed(started_at.elapsed());
                    if canceled.load(Ordering::Relaxed) {
                        continue;
                    }
                    if result_tx
                        .send(JobResult::Search(SearchBuild {
                            token: request.token,
                            cwd: request.cwd,
                            scope: request.scope,
                            show_hidden: request.show_hidden,
                            fingerprint: request.fingerprint,
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

    pub(in crate::app::jobs) fn submit(&self, request: SearchRequest) -> bool {
        let key = SearchJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        if state.pending_key.as_ref() == Some(&key) {
            state.pending = Some(request);
            if let Some(active) = &state.active {
                active.canceled.store(true, Ordering::Relaxed);
            }
            self.shared.available.notify_one();
            return true;
        }
        state.pending = Some(request);
        state.pending_key = Some(key);
        if let Some(active) = &state.active {
            active.canceled.store(true, Ordering::Relaxed);
        }
        lock_unpoison(&self.metrics).search_jobs_submitted += 1;
        self.shared.available.notify_one();
        true
    }

    pub(in crate::app::jobs) fn cancel_all(&self) {
        let mut state = lock_unpoison(&self.shared.state);
        state.pending = None;
        state.pending_key = None;
        if let Some(active) = &state.active {
            active.canceled.store(true, Ordering::Relaxed);
        }
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active.is_some()
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn pending_key(&self) -> Option<SearchJobKey> {
        lock_unpoison(&self.shared.state).pending_key.clone()
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn active_key(&self) -> Option<SearchJobKey> {
        lock_unpoison(&self.shared.state)
            .active
            .as_ref()
            .map(|active| active.key.clone())
    }
}

impl Drop for SearchPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending = None;
            state.pending_key = None;
            if let Some(active) = &state.active {
                active.canceled.store(true, Ordering::Relaxed);
            }
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl SearchShared {
    fn pop(shared: &Arc<Self>) -> Option<(SearchRequest, Arc<AtomicBool>)> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if state.active.is_none()
                && let Some(request) = state.pending.take()
            {
                let key = state
                    .pending_key
                    .take()
                    .expect("pending key should exist for search job");
                let canceled = Arc::new(AtomicBool::new(false));
                state.active = Some(ActiveSearchJob {
                    key,
                    canceled: Arc::clone(&canceled),
                });
                return Some((request, canceled));
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &SearchJobKey) {
        let mut state = lock_unpoison(&shared.state);
        if state
            .active
            .as_ref()
            .is_some_and(|active| &active.key == key)
        {
            state.active = None;
            shared.available.notify_one();
        }
    }
}

impl SearchJobKey {
    fn from_request(request: &SearchRequest) -> Self {
        Self {
            cwd: request.cwd.clone(),
            scope: request.scope,
            show_hidden: request.show_hidden,
            fingerprint: request.fingerprint,
        }
    }
}
