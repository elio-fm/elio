use super::*;
use std::{
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
};

pub(in crate::app::jobs) struct GitStatusPool {
    shared: Arc<GitStatusShared>,
    worker: Option<thread::JoinHandle<()>>,
}

struct GitStatusShared {
    state: Mutex<GitStatusState>,
    available: Condvar,
}

struct GitStatusState {
    pending: Option<GitStatusRequest>,
    active: bool,
    closed: bool,
}

impl GitStatusPool {
    pub(in crate::app::jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(GitStatusShared {
            state: Mutex::new(GitStatusState {
                pending: None,
                active: false,
                closed: false,
            }),
            available: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::spawn(move || {
            while let Some(request) = GitStatusShared::pop(&worker_shared) {
                let (branch, statuses) = crate::app::git::current_status(&request.cwd);
                let dirty = !statuses.is_empty();
                GitStatusShared::finish(&worker_shared);
                if result_tx
                    .send(JobResult::GitStatus(GitStatusBuild {
                        token: request.token,
                        cwd: request.cwd,
                        branch,
                        dirty,
                        statuses,
                    }))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            shared,
            worker: Some(worker),
        }
    }

    pub(in crate::app::jobs) fn submit(&self, request: GitStatusRequest) -> bool {
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        state.pending = Some(request);
        self.shared.available.notify_one();
        true
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active
    }
}

impl Drop for GitStatusPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending = None;
        }
        self.shared.available.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl GitStatusShared {
    fn pop(shared: &Arc<Self>) -> Option<GitStatusRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending.take() {
                state.active = true;
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>) {
        let mut state = lock_unpoison(&shared.state);
        state.active = false;
        shared.available.notify_all();
    }
}
