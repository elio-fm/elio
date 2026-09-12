use super::*;
use std::{
    path::Path,
    process::Command,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
};

pub(in crate::background_jobs) struct GitStatusPool {
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
    pub(in crate::background_jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
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
                let (branch, dirty) = current_status(&request.cwd);
                GitStatusShared::finish(&worker_shared);
                if result_tx
                    .send(JobResult::GitStatus(GitStatusBuild {
                        token: request.token,
                        cwd: request.cwd,
                        branch,
                        dirty,
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

    pub(in crate::background_jobs) fn submit(&self, request: GitStatusRequest) -> bool {
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        state.pending = Some(request);
        self.shared.available.notify_one();
        true
    }

    pub(in crate::background_jobs) fn has_pending_work(&self) -> bool {
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

fn current_status(cwd: &Path) -> (Option<String>, bool) {
    if git_command(cwd, ["rev-parse", "--is-inside-work-tree"])
        .is_none_or(|output| output.trim() != "true")
    {
        return (None, false);
    }

    let branch = git_command(cwd, ["branch", "--show-current"])
        .and_then(non_empty_trimmed)
        .or_else(|| git_command(cwd, ["rev-parse", "--short", "HEAD"]).and_then(non_empty_trimmed));
    let dirty = git_command(
        cwd,
        ["status", "--porcelain=v1", "--untracked-files=normal"],
    )
    .is_some_and(|output| !output.trim().is_empty());

    (branch, dirty)
}

fn git_command<const N: usize>(cwd: &Path, args: [&str; N]) -> Option<String> {
    let output = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn non_empty_trimmed(output: String) -> Option<String> {
    let branch = output.trim();
    (!branch.is_empty()).then(|| branch.to_string())
}

#[cfg(test)]
#[path = "tests/git_status.rs"]
mod tests;
