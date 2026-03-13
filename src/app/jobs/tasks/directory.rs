use super::*;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::Instant,
};

pub(in crate::app::jobs) struct DirectoryPool {
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
    pub(in crate::app::jobs) fn new(
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
                    let result = crate::fs::load_directory_snapshot(
                        &request.cwd,
                        request.show_hidden,
                        request.sort_mode,
                    )
                    .map_err(|error| {
                        error
                            .downcast_ref::<std::io::Error>()
                            .map(crate::fs::describe_io_error)
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

    pub(in crate::app::jobs) fn submit(&self, request: DirectoryRequest) -> bool {
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

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
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
