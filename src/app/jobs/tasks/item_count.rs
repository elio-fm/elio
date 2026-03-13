use super::*;
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::SystemTime,
};

pub(in crate::app::jobs) struct DirectoryItemCountPool {
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
    pub(in crate::app::jobs) fn new(
        worker_count: usize,
        capacity: usize,
        result_tx: mpsc::Sender<JobResult>,
    ) -> Self {
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
                        crate::fs::count_directory_items(&request.path, request.show_hidden).ok();
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

    pub(in crate::app::jobs) fn submit(&self, request: DirectoryItemCountRequest) -> bool {
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

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
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
