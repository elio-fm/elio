use super::*;
use crate::filesystem::{directory_watch_debounce, start_git_head_watcher};
use std::{
    path::Path,
    process::Command,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::Instant,
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
    latest: Option<GitStatusRequest>,
    head_changed: Option<Instant>,
    active: bool,
    closed: bool,
}

impl GitStatusPool {
    pub(in crate::background_jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(GitStatusShared {
            state: Mutex::new(GitStatusState {
                pending: None,
                latest: None,
                head_changed: None,
                active: false,
                closed: false,
            }),
            available: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::spawn(move || {
            let mut watched_cwd = None;
            let mut watcher = None;
            while let Some(request) = GitStatusShared::pop(&worker_shared) {
                let inside_worktree = is_inside_worktree(&request.cwd);
                if !inside_worktree {
                    watcher = None;
                    watched_cwd = None;
                } else if watched_cwd.as_ref() != Some(&request.cwd) || watcher.is_none() {
                    watcher = None;
                    watched_cwd = Some(request.cwd.clone());
                    // Resolve through Git for nested directories and linked worktrees.
                    // Arm before reading status so switches during the read are not lost.
                    if let Some(git_dir) =
                        git_command(&request.cwd, ["rev-parse", "--absolute-git-dir"])
                            .and_then(non_empty_trimmed)
                    {
                        let shared = Arc::clone(&worker_shared);
                        let cwd = request.cwd.clone();
                        // Watch failure retains on-demand refresh only. Retry on the
                        // next request; never fall back to recurring Git polling.
                        watcher = start_git_head_watcher(Path::new(&git_dir), move || {
                            let mut state = lock_unpoison(&shared.state);
                            if !state.closed
                                && state
                                    .latest
                                    .as_ref()
                                    .is_some_and(|request| request.cwd == cwd)
                            {
                                state.head_changed = Some(Instant::now());
                                shared.available.notify_one();
                            }
                        })
                        .ok();
                    }
                }
                let (branch, dirty) = if inside_worktree {
                    current_worktree_status(&request.cwd)
                } else {
                    (None, false)
                };
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
        state.latest = Some(request.clone());
        state.head_changed = None;
        state.pending = Some(request);
        self.shared.available.notify_one();
        true
    }

    pub(in crate::background_jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active || state.head_changed.is_some()
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
            if let Some(changed) = state.head_changed {
                let remaining = directory_watch_debounce().saturating_sub(changed.elapsed());
                if remaining.is_zero() {
                    state.head_changed = None;
                    if let Some(request) = state.latest.clone() {
                        state.active = true;
                        return Some(request);
                    }
                } else {
                    state = shared
                        .available
                        .wait_timeout(state, remaining)
                        .unwrap_or_else(|error| error.into_inner())
                        .0;
                    continue;
                }
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

fn is_inside_worktree(cwd: &Path) -> bool {
    git_command(cwd, ["rev-parse", "--is-inside-work-tree"])
        .is_some_and(|output| output.trim() == "true")
}

#[cfg(test)]
fn current_status(cwd: &Path) -> (Option<String>, bool) {
    if is_inside_worktree(cwd) {
        current_worktree_status(cwd)
    } else {
        (None, false)
    }
}

fn current_worktree_status(cwd: &Path) -> (Option<String>, bool) {
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
