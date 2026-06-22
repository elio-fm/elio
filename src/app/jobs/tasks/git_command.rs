use super::*;
use std::{
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
};

/// Runs ad-hoc, read-only git commands (status / log / diff) off the UI thread
/// and returns their captured output for display in the preview pane.
///
/// Only the most recent request is kept pending: a fresh menu selection
/// supersedes an in-flight one, mirroring [`GitStatusPool`](super::git_status).
pub(in crate::app::jobs) struct GitCommandPool {
    shared: Arc<GitCommandShared>,
    worker: Option<thread::JoinHandle<()>>,
}

struct GitCommandShared {
    state: Mutex<GitCommandState>,
    available: Condvar,
}

struct GitCommandState {
    pending: Option<GitCommandRequest>,
    active: bool,
    closed: bool,
}

impl GitCommandPool {
    pub(in crate::app::jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(GitCommandShared {
            state: Mutex::new(GitCommandState {
                pending: None,
                active: false,
                closed: false,
            }),
            available: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::spawn(move || {
            while let Some(request) = GitCommandShared::pop(&worker_shared) {
                let (output, success) = crate::app::git::run_command(&request.cwd, request.command);
                GitCommandShared::finish(&worker_shared);
                if result_tx
                    .send(JobResult::GitCommand(GitCommandBuild {
                        token: request.token,
                        cwd: request.cwd,
                        command: request.command,
                        output,
                        success,
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

    pub(in crate::app::jobs) fn submit(&self, request: GitCommandRequest) -> bool {
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

impl Drop for GitCommandPool {
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

impl GitCommandShared {
    fn pop(shared: &Arc<Self>) -> Option<GitCommandRequest> {
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
