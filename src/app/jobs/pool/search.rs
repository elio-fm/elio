use super::*;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
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
    active_key: Option<SearchJobKey>,
    closed: bool,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app::jobs) struct SearchJobKey {
    pub(in crate::app::jobs) cwd: PathBuf,
    pub(in crate::app::jobs) scope: SearchScope,
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
                    let result = crate::fs::search::collect_candidates(
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

    pub(in crate::app::jobs) fn submit(&self, request: SearchRequest) -> bool {
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

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active_key.is_some()
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn pending_key(&self) -> Option<SearchJobKey> {
        lock_unpoison(&self.shared.state).pending_key.clone()
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn active_key(&self) -> Option<SearchJobKey> {
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
