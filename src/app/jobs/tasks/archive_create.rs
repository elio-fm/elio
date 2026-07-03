use super::*;
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const PROGRESS_SEND_INTERVAL: Duration = Duration::from_millis(80);

pub(in crate::app::jobs) struct ArchiveCreatePool {
    shared: Arc<ArchiveCreateShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct ArchiveCreateShared {
    state: Mutex<ArchiveCreateState>,
    available: Condvar,
    cancelled: AtomicBool,
    cancel_token: AtomicU64,
}

struct ArchiveCreateState {
    pending: Option<ArchiveCreateRequest>,
    active: bool,
    closed: bool,
}

impl ArchiveCreatePool {
    pub(in crate::app::jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(ArchiveCreateShared {
            state: Mutex::new(ArchiveCreateState {
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
            while let Some(request) = ArchiveCreateShared::pop(&shared_worker) {
                ArchiveCreateShared::set_active(&shared_worker, true);
                run_create(
                    request,
                    &result_tx,
                    &shared_worker.cancelled,
                    &shared_worker.cancel_token,
                );
                ArchiveCreateShared::set_active(&shared_worker, false);
            }
        });
        Self {
            shared,
            workers: vec![worker],
        }
    }

    pub(in crate::app::jobs) fn submit(&self, request: ArchiveCreateRequest) -> bool {
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed || state.pending.is_some() || state.active {
            return false;
        }
        state.pending = Some(request);
        self.shared.available.notify_one();
        true
    }

    pub(in crate::app::jobs) fn cancel_create(&self, token: u64) {
        self.shared.cancel_token.store(token, Ordering::Relaxed);
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active
    }
}

impl Drop for ArchiveCreatePool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending = None;
        }
        self.shared.cancelled.store(true, Ordering::Relaxed);
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl ArchiveCreateShared {
    fn pop(shared: &Arc<Self>) -> Option<ArchiveCreateRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending.take() {
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn set_active(shared: &Arc<Self>, active: bool) {
        lock_unpoison(&shared.state).active = active;
    }
}

fn run_create(
    request: ArchiveCreateRequest,
    result_tx: &mpsc::Sender<JobResult>,
    cancelled: &AtomicBool,
    cancel_token: &AtomicU64,
) {
    let mut completed = 0usize;
    let mut total = 0usize;
    let mut last_progress_at: Option<Instant> = None;
    let stopped_early = AtomicBool::new(false);

    let result = crate::archive::plan_create_zip_archive(
        &request.cwd,
        request.sources.clone(),
        &request.output_name,
    )
    .and_then(|plan| {
        crate::archive::create_zip_archive(
            &plan,
            |progress| {
                completed = progress.completed;
                total = progress.total;
                let _ = send_create_progress(
                    result_tx,
                    request.token,
                    completed,
                    total,
                    &mut last_progress_at,
                );
            },
            || {
                let stop = cancelled.load(Ordering::Relaxed)
                    || cancel_token.load(Ordering::Relaxed) == request.token;
                if stop {
                    stopped_early.store(true, Ordering::Relaxed);
                }
                stop
            },
        )
    });

    let (output_path, status) = match result {
        Ok(summary) if stopped_early.load(Ordering::Relaxed) => (
            Some(summary.output_path),
            Some("Archive creation cancelled".to_string()),
        ),
        Ok(summary) => {
            let name = summary
                .output_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("archive.zip")
                .to_string();
            (
                Some(summary.output_path),
                Some(format!("Created \"{name}\"")),
            )
        }
        Err(error) if stopped_early.load(Ordering::Relaxed) => (None, Some(error.to_string())),
        Err(error) => {
            let name = request.output_name.trim();
            let name = if name.is_empty() { "archive.zip" } else { name };
            (None, Some(format!("Cannot create \"{name}\" — {error}")))
        }
    };

    let _ = result_tx.send(JobResult::ArchiveCreate(ArchiveCreateBuild {
        token: request.token,
        completed,
        total,
        done: true,
        output_path,
        status,
    }));
}

fn send_create_progress(
    result_tx: &mpsc::Sender<JobResult>,
    token: u64,
    completed: usize,
    total: usize,
    last_progress_at: &mut Option<Instant>,
) -> bool {
    let now = Instant::now();
    if last_progress_at.is_some_and(|last| now.duration_since(last) < PROGRESS_SEND_INTERVAL) {
        return true;
    }
    *last_progress_at = Some(now);
    result_tx
        .send(JobResult::ArchiveCreate(ArchiveCreateBuild {
            token,
            completed,
            total,
            done: false,
            output_path: None,
            status: None,
        }))
        .is_ok()
}
