use super::*;
use std::{
    fs,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

/// Minimum time between intermediate progress results sent to the UI.
/// Only applies to permanent delete, which processes files one at a time.
/// Non-permanent trash is a single batched OS call with no intermediate
/// progress.
const PROGRESS_SEND_INTERVAL: Duration = Duration::from_millis(80);

pub(in crate::app::jobs) struct TrashPool {
    shared: Arc<TrashShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct TrashShared {
    state: Mutex<TrashState>,
    available: Condvar,
    cancelled: AtomicBool,
    cancel_token: AtomicU64,
}

struct TrashState {
    pending: Option<TrashRequest>,
    active: bool,
    closed: bool,
}

impl TrashPool {
    pub(in crate::app::jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(TrashShared {
            state: Mutex::new(TrashState {
                pending: None,
                active: false,
                closed: false,
            }),
            available: Condvar::new(),
            cancelled: AtomicBool::new(false),
            cancel_token: AtomicU64::new(0),
        });
        let shared_worker = Arc::clone(&shared);
        let worker = thread::spawn(move || {
            while let Some(request) = TrashShared::pop(&shared_worker) {
                TrashShared::set_active(&shared_worker, true);
                let (completed, errors, stopped_early) = run_trash(
                    &request,
                    &result_tx,
                    &shared_worker.cancelled,
                    &shared_worker.cancel_token,
                );
                TrashShared::set_active(&shared_worker, false);

                let verb = if request.permanent {
                    "Permanently deleted"
                } else {
                    "Trashed"
                };
                let total = request.targets.len();
                let single_name = (total == 1).then(|| request.targets[0].name.as_str());
                let status = if stopped_early {
                    match completed {
                        0 => "Trash cancelled".to_string(),
                        1 => format!("Trash cancelled — {verb} 1 item"),
                        n => format!("Trash cancelled — {verb} {n} items"),
                    }
                } else if errors.is_empty() {
                    match (completed, single_name) {
                        (0, _) => "Nothing was deleted".to_string(),
                        (1, Some(name)) => format!("{verb} \"{name}\""),
                        (n, _) => format!("{verb} {n} items"),
                    }
                } else if completed == 0 {
                    if errors.len() == 1 {
                        errors[0].clone()
                    } else {
                        format!("{} errors — first: {}", errors.len(), errors[0])
                    }
                } else {
                    format!(
                        "{verb} {completed} item(s); {} error(s) — first: {}",
                        errors.len(),
                        errors[0]
                    )
                };

                if result_tx
                    .send(JobResult::Trash(TrashBuild {
                        token: request.token,
                        completed,
                        done: true,
                        status: Some(status),
                    }))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            shared,
            workers: vec![worker],
        }
    }

    pub(in crate::app::jobs) fn submit(&self, request: TrashRequest) -> bool {
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        state.pending = Some(request);
        self.shared.available.notify_one();
        true
    }

    /// Signal the worker to stop after the current item if it is processing
    /// the trash request with the given token.  A concurrent or future request
    /// with a different token is unaffected.
    pub(in crate::app::jobs) fn cancel_trash(&self, token: u64) {
        self.shared.cancel_token.store(token, Ordering::Relaxed);
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active
    }
}

impl Drop for TrashPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            // Do NOT clear `pending` and do NOT set `cancelled`: the worker must
            // finish any in-flight and queued requests completely before exiting.
            // Setting `cancelled` here (as PastePool does) would abandon targets
            // mid-batch and leave them neither deleted nor untouched, which is
            // worse than a momentary delay on exit.
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl TrashShared {
    fn pop(shared: &Arc<Self>) -> Option<TrashRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            // Drain any queued request before honoring the close signal so
            // that a pending job submitted just before shutdown is not lost.
            if let Some(request) = state.pending.take() {
                return Some(request);
            }
            if state.closed {
                return None;
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn set_active(shared: &Arc<Self>, active: bool) {
        lock_unpoison(&shared.state).active = active;
    }
}

fn run_trash(
    request: &TrashRequest,
    result_tx: &mpsc::Sender<JobResult>,
    cancelled: &AtomicBool,
    cancel_token: &AtomicU64,
) -> (usize, Vec<String>, bool) {
    if request.permanent {
        run_permanent_delete(request, result_tx, cancelled, cancel_token)
    } else {
        run_trash_batch(request, cancelled, cancel_token)
    }
}

/// Delete each target permanently using per-item `fs` calls.
///
/// Sends throttled intermediate progress results so the UI chip updates
/// during long operations, and supports mid-batch cancellation between
/// items.
fn run_permanent_delete(
    request: &TrashRequest,
    result_tx: &mpsc::Sender<JobResult>,
    cancelled: &AtomicBool,
    cancel_token: &AtomicU64,
) -> (usize, Vec<String>, bool) {
    let mut completed = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut stopped_early = false;
    let mut last_progress_at: Option<Instant> = None;

    for target in &request.targets {
        if cancelled.load(Ordering::Relaxed)
            || cancel_token.load(Ordering::Relaxed) == request.token
        {
            stopped_early = true;
            break;
        }

        let result = if target.is_dir {
            fs::remove_dir_all(&target.path)
                .map_err(|e| anyhow::anyhow!("Could not delete \"{}\": {e}", target.name))
        } else {
            fs::remove_file(&target.path)
                .map_err(|e| anyhow::anyhow!("Could not delete \"{}\": {e}", target.name))
        };

        match result {
            Ok(()) => completed += 1,
            Err(e) => errors.push(e.to_string()),
        }

        if !send_trash_progress(result_tx, request.token, completed, &mut last_progress_at) {
            break;
        }
    }

    (completed, errors, stopped_early)
}

/// Move all targets to the OS trash in a single batched call to
/// `trash::delete_all`.
///
/// This is significantly faster than per-item trashing because the `trash`
/// crate amortizes expensive setup work — reading `/proc/mounts`, resolving
/// the trash directory, checking for name collisions — across the whole
/// batch instead of repeating it for every file.
///
/// Cancellation is checked once before the batch starts.  The batch itself
/// is treated as atomic from Elio's perspective: once the OS call is in
/// flight it cannot be interrupted mid-way.  No intermediate progress
/// results are sent; the UI chip stays at 0/N until the call returns.
fn run_trash_batch(
    request: &TrashRequest,
    cancelled: &AtomicBool,
    cancel_token: &AtomicU64,
) -> (usize, Vec<String>, bool) {
    if cancelled.load(Ordering::Relaxed) || cancel_token.load(Ordering::Relaxed) == request.token {
        return (0, Vec::new(), true);
    }

    let paths: Vec<_> = request.targets.iter().map(|t| &t.path).collect();
    let total = paths.len();

    match ::trash::delete_all(paths) {
        Ok(()) => (total, Vec::new(), false),
        Err(e) => (0, vec![e.to_string()], false),
    }
}

/// Send a throttled intermediate progress result for the permanent-delete
/// worker.  Returns `false` if the receiver has been dropped (loop should
/// break).
fn send_trash_progress(
    result_tx: &mpsc::Sender<JobResult>,
    token: u64,
    completed: usize,
    last_progress_at: &mut Option<Instant>,
) -> bool {
    let now = Instant::now();
    let due = last_progress_at.is_none_or(|t| now.duration_since(t) >= PROGRESS_SEND_INTERVAL);
    if due {
        *last_progress_at = Some(now);
        return result_tx
            .send(JobResult::Trash(TrashBuild {
                token,
                completed,
                done: false,
                status: None,
            }))
            .is_ok();
    }
    true
}
