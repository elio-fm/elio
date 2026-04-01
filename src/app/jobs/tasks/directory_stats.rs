use super::*;
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

pub(in crate::app::jobs) struct DirectoryStatsPool {
    shared: Arc<DirectoryStatsShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct DirectoryStatsShared {
    state: Mutex<DirectoryStatsState>,
    available: Condvar,
}

struct DirectoryStatsState {
    pending: Option<DirectoryStatsRequest>,
    active: Option<ActiveDirectoryStatsJob>,
    closed: bool,
}

#[derive(Clone, Debug)]
struct ActiveDirectoryStatsJob {
    key: DirectoryStatsJobKey,
    canceled: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DirectoryStatsJobKey {
    token: u64,
    path: PathBuf,
}

impl DirectoryStatsPool {
    pub(in crate::app::jobs) fn new(
        worker_count: usize,
        result_tx: mpsc::Sender<JobResult>,
    ) -> Self {
        let shared = Arc::new(DirectoryStatsShared {
            state: Mutex::new(DirectoryStatsState {
                pending: None,
                active: None,
                closed: false,
            }),
            available: Condvar::new(),
        });
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let shared = Arc::clone(&shared);
            let result_tx = result_tx.clone();
            workers.push(thread::spawn(move || {
                while let Some((request, canceled)) = DirectoryStatsShared::pop(&shared) {
                    let key = DirectoryStatsJobKey::from_request(&request);
                    let result = crate::fs::scan_directory_stats(&request.path, &|| {
                        canceled.load(Ordering::Relaxed)
                    });
                    DirectoryStatsShared::finish(&shared, &key);
                    if canceled.load(Ordering::Relaxed) {
                        continue;
                    }
                    if result_tx
                        .send(JobResult::DirectoryStats(DirectoryStatsBuild {
                            token: request.token,
                            path: request.path,
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

    pub(in crate::app::jobs) fn submit(&self, request: DirectoryStatsRequest) -> bool {
        let key = DirectoryStatsJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        if state
            .pending
            .as_ref()
            .is_some_and(|pending| DirectoryStatsJobKey::from_request(pending) == key)
        {
            state.pending = Some(request);
            self.shared.available.notify_one();
            return true;
        }
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.key == key)
        {
            return true;
        }
        if let Some(active) = &state.active {
            active.canceled.store(true, Ordering::Relaxed);
        }
        state.pending = Some(request);
        self.shared.available.notify_one();
        true
    }

    pub(in crate::app::jobs) fn cancel_all(&self) {
        let mut state = lock_unpoison(&self.shared.state);
        state.pending = None;
        if let Some(active) = &state.active {
            active.canceled.store(true, Ordering::Relaxed);
        }
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active.is_some()
    }
}

impl Drop for DirectoryStatsPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending = None;
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

impl DirectoryStatsShared {
    fn pop(shared: &Arc<Self>) -> Option<(DirectoryStatsRequest, Arc<AtomicBool>)> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if state.active.is_none()
                && let Some(request) = state.pending.take()
            {
                let key = DirectoryStatsJobKey::from_request(&request);
                let canceled = Arc::new(AtomicBool::new(false));
                state.active = Some(ActiveDirectoryStatsJob {
                    key,
                    canceled: Arc::clone(&canceled),
                });
                return Some((request, canceled));
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &DirectoryStatsJobKey) {
        let mut state = lock_unpoison(&shared.state);
        if state
            .active
            .as_ref()
            .is_some_and(|active| &active.key == key)
        {
            state.active = None;
        }
        shared.available.notify_one();
    }
}

impl DirectoryStatsJobKey {
    fn from_request(request: &DirectoryStatsRequest) -> Self {
        Self {
            token: request.token,
            path: request.path.clone(),
        }
    }
}
