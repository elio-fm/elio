use super::*;
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::SystemTime,
};

pub(in crate::background_jobs) struct PreviewLineCountPool {
    shared: Arc<PreviewLineCountShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct PreviewLineCountShared {
    state: Mutex<PreviewLineCountState>,
    available: Condvar,
}

struct PreviewLineCountState {
    pending: VecDeque<PreviewLineCountRequest>,
    queued_keys: HashSet<PreviewLineCountJobKey>,
    active_keys: HashSet<PreviewLineCountJobKey>,
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PreviewLineCountJobKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

impl PreviewLineCountPool {
    pub(in crate::background_jobs) fn new(
        worker_count: usize,
        capacity: usize,
        result_tx: mpsc::Sender<JobResult>,
    ) -> Self {
        let shared = Arc::new(PreviewLineCountShared {
            state: Mutex::new(PreviewLineCountState {
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
                while let Some(request) = PreviewLineCountShared::pop(&shared) {
                    let key = PreviewLineCountJobKey::from_request(&request);
                    let total_lines = crate::preview::count_total_text_lines(&request.path).ok();
                    PreviewLineCountShared::finish(&shared, &key);
                    if result_tx
                        .send(JobResult::PreviewLineCount(PreviewLineCountBuild {
                            path: request.path,
                            size: request.size,
                            modified: request.modified,
                            total_lines,
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

    pub(in crate::background_jobs) fn submit(&self, request: PreviewLineCountRequest) -> bool {
        let key = PreviewLineCountJobKey::from_request(&request);
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
                .remove(&PreviewLineCountJobKey::from_request(&stale));
        }
        state.queued_keys.insert(key);
        state.pending.push_back(request);
        self.shared.available.notify_one();
        true
    }

    pub(in crate::background_jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        !state.pending.is_empty() || !state.active_keys.is_empty()
    }
}

impl Drop for PreviewLineCountPool {
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

impl PreviewLineCountShared {
    fn pop(shared: &Arc<Self>) -> Option<PreviewLineCountRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending.pop_front() {
                let key = PreviewLineCountJobKey::from_request(&request);
                state.queued_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &PreviewLineCountJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active_keys.remove(key);
    }
}

impl PreviewLineCountJobKey {
    fn from_request(request: &PreviewLineCountRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
        }
    }
}
