use super::*;
use crate::archive::ExtractError;
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

pub(in crate::app::jobs) struct ArchiveExtractPool {
    shared: Arc<ArchiveExtractShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct ArchiveExtractShared {
    state: Mutex<ArchiveExtractState>,
    available: Condvar,
    cancelled: AtomicBool,
    cancel_token: AtomicU64,
}

struct ArchiveExtractState {
    pending: Option<ArchiveExtractRequest>,
    active: bool,
    closed: bool,
}

impl ArchiveExtractPool {
    pub(in crate::app::jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let shared = Arc::new(ArchiveExtractShared {
            state: Mutex::new(ArchiveExtractState {
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
            while let Some(request) = ArchiveExtractShared::pop(&shared_worker) {
                ArchiveExtractShared::set_active(&shared_worker, true);
                run_extract(
                    request,
                    &result_tx,
                    &shared_worker.cancelled,
                    &shared_worker.cancel_token,
                );
                ArchiveExtractShared::set_active(&shared_worker, false);
            }
        });
        Self {
            shared,
            workers: vec![worker],
        }
    }

    pub(in crate::app::jobs) fn submit(&self, request: ArchiveExtractRequest) -> bool {
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed || state.pending.is_some() || state.active {
            return false;
        }
        state.pending = Some(request);
        self.shared.available.notify_one();
        true
    }

    pub(in crate::app::jobs) fn cancel_extract(&self, token: u64) {
        self.shared.cancel_token.store(token, Ordering::Relaxed);
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        state.pending.is_some() || state.active
    }
}

impl Drop for ArchiveExtractPool {
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

impl ArchiveExtractShared {
    fn pop(shared: &Arc<Self>) -> Option<ArchiveExtractRequest> {
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

fn run_extract(
    request: ArchiveExtractRequest,
    result_tx: &mpsc::Sender<JobResult>,
    cancelled: &AtomicBool,
    cancel_token: &AtomicU64,
) {
    let mut completed = 0usize;
    let mut total = None;
    let mut last_progress_at: Option<Instant> = None;
    let mut stopped_early = false;

    let result = crate::archive::plan_extract(&request.archive_path)
        .map_err(ExtractError::from)
        .and_then(|plan| {
            crate::archive::extract_archive_with_password(
                &plan,
                request.password.as_ref(),
                |progress| {
                    completed = progress.completed;
                    total = progress.total;
                    let _ = send_extract_progress(
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
                        stopped_early = true;
                    }
                    stop
                },
            )
        });

    let mut password_prompt = None;
    let (dest_dir, status) = match result {
        Ok(summary) if stopped_early => {
            let noun = if summary.completed == 1 {
                "item"
            } else {
                "items"
            };
            (
                Some(summary.dest_dir),
                format!(
                    "Extraction cancelled — extracted {} {noun}",
                    summary.completed
                ),
            )
        }
        Ok(summary) => {
            let name = summary
                .dest_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("folder")
                .to_string();
            let noun = if summary.completed == 1 {
                "item"
            } else {
                "items"
            };
            (
                Some(summary.dest_dir),
                format!("Extracted {} {noun} to \"{name}\"", summary.completed),
            )
        }
        Err(ExtractError::PasswordRequired) => {
            password_prompt = Some(ArchivePasswordPrompt::Required);
            (None, String::new())
        }
        Err(ExtractError::BadPassword) => {
            password_prompt = Some(ArchivePasswordPrompt::BadPassword);
            (None, String::new())
        }
        Err(error) => {
            let name = request
                .archive_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("archive");
            (None, format!("Cannot extract \"{name}\" — {error}"))
        }
    };

    let _ = result_tx.send(JobResult::ArchiveExtract(ArchiveExtractBuild {
        token: request.token,
        completed,
        total,
        done: true,
        dest_dir,
        status: (!status.is_empty()).then_some(status),
        password_prompt,
    }));
}

fn send_extract_progress(
    result_tx: &mpsc::Sender<JobResult>,
    token: u64,
    completed: usize,
    total: Option<usize>,
    last_progress_at: &mut Option<Instant>,
) -> bool {
    let now = Instant::now();
    if last_progress_at.is_some_and(|last| now.duration_since(last) < PROGRESS_SEND_INTERVAL) {
        return true;
    }
    *last_progress_at = Some(now);
    result_tx
        .send(JobResult::ArchiveExtract(ArchiveExtractBuild {
            token,
            completed,
            total,
            done: false,
            dest_dir: None,
            status: None,
            password_prompt: None,
        }))
        .is_ok()
}
