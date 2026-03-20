use super::*;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
};

pub(in crate::app::jobs) struct DirectoryFingerprintPool {
    shared: Arc<DirectoryFingerprintShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct DirectoryFingerprintShared {
    state: Mutex<DirectoryFingerprintState>,
    available: Condvar,
}

struct DirectoryFingerprintState {
    pending: Option<DirectoryFingerprintRequest>,
    pending_key: Option<DirectoryFingerprintJobKey>,
    active_key: Option<DirectoryFingerprintJobKey>,
    closed: bool,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DirectoryFingerprintJobKey {
    cwd: PathBuf,
    show_hidden: bool,
}

impl DirectoryFingerprintPool {
    pub(in crate::app::jobs) fn new(
        worker_count: usize,
        result_tx: mpsc::Sender<JobResult>,
    ) -> Self {
        let shared = Arc::new(DirectoryFingerprintShared {
            state: Mutex::new(DirectoryFingerprintState {
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
            workers.push(thread::spawn(move || {
                while let Some(request) = DirectoryFingerprintShared::pop(&shared) {
                    let key = DirectoryFingerprintJobKey::from_request(&request);
                    let result =
                        crate::fs::scan_directory_fingerprint(&request.cwd, request.show_hidden)
                            .map_err(|error| {
                                error
                                    .downcast_ref::<std::io::Error>()
                                    .map(crate::fs::describe_io_error)
                                    .unwrap_or("Read error")
                                    .to_string()
                            });
                    DirectoryFingerprintShared::finish(&shared, &key);
                    if result_tx
                        .send(JobResult::DirectoryFingerprint(DirectoryFingerprintBuild {
                            token: request.token,
                            cwd: request.cwd,
                            show_hidden: request.show_hidden,
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

    pub(in crate::app::jobs) fn submit(&self, request: DirectoryFingerprintRequest) -> bool {
        let key = DirectoryFingerprintJobKey::from_request(&request);
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
        self.shared.available.notify_one();
        true
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active_key.is_some()
    }
}

impl Drop for DirectoryFingerprintPool {
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

impl DirectoryFingerprintShared {
    fn pop(shared: &Arc<Self>) -> Option<DirectoryFingerprintRequest> {
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

    fn finish(shared: &Arc<Self>, key: &DirectoryFingerprintJobKey) {
        let mut state = lock_unpoison(&shared.state);
        if state.active_key.as_ref() == Some(key) {
            state.active_key = None;
        }
    }
}

impl DirectoryFingerprintJobKey {
    fn from_request(request: &DirectoryFingerprintRequest) -> Self {
        Self {
            cwd: request.cwd.clone(),
            show_hidden: request.show_hidden,
        }
    }
}
