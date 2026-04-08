use super::*;
use crate::preview::{PreviewRequestOptions, PreviewWorkClass};
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Instant, SystemTime},
};

const MAX_CONCURRENT_LOW_PRIORITY_HEAVY_PREVIEWS: usize = 2;

pub(in crate::app::jobs) struct PreviewPool {
    shared: Arc<PreviewShared>,
    workers: Vec<thread::JoinHandle<()>>,
    metrics: Arc<Mutex<SchedulerMetrics>>,
}

struct PreviewShared {
    state: Mutex<PreviewState>,
    available: Condvar,
}

struct PreviewState {
    pending_high: VecDeque<PreviewRequest>,
    pending_low: VecDeque<PreviewRequest>,
    queued_high_keys: HashSet<PreviewJobKey>,
    queued_low_keys: HashSet<PreviewJobKey>,
    active_keys: HashSet<PreviewJobKey>,
    active_jobs: Vec<ActivePreviewJob>,
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug)]
struct ActivePreviewJob {
    key: PreviewJobKey,
    priority: PreviewPriority,
    work_class: PreviewWorkClass,
    canceled: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app::jobs) struct PreviewJobKey {
    pub(in crate::app::jobs) path: PathBuf,
    pub(in crate::app::jobs) size: u64,
    pub(in crate::app::jobs) modified: Option<SystemTime>,
    pub(in crate::app::jobs) variant: PreviewRequestOptions,
    pub(in crate::app::jobs) code_line_limit: usize,
    /// Included so that an initial partial render and its extension job are
    /// treated as distinct keys and not deduplicated against each other.
    pub(in crate::app::jobs) code_render_limit: usize,
}

impl PreviewPool {
    pub(in crate::app::jobs) fn new(
        worker_count: usize,
        capacity: usize,
        result_tx: mpsc::Sender<JobResult>,
        metrics: Arc<Mutex<SchedulerMetrics>>,
    ) -> Self {
        let shared = Arc::new(PreviewShared {
            state: Mutex::new(PreviewState {
                pending_high: VecDeque::new(),
                pending_low: VecDeque::new(),
                queued_high_keys: HashSet::new(),
                queued_low_keys: HashSet::new(),
                active_keys: HashSet::new(),
                active_jobs: Vec::new(),
                closed: false,
                capacity,
            }),
            available: Condvar::new(),
        });
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let shared = Arc::clone(&shared);
            let result_tx = result_tx.clone();
            let metrics = Arc::clone(&metrics);
            workers.push(thread::spawn(move || {
                while let Some((request, canceled)) = PreviewShared::pop(&shared) {
                    let key = PreviewJobKey::from_request(&request);
                    let started_at = Instant::now();
                    let result = crate::preview::build_preview_with_options_and_code_line_limit(
                        &request.entry,
                        &request.variant,
                        request.code_line_limit,
                        request.code_render_limit,
                        request.ffprobe_available,
                        request.ffmpeg_available,
                        &|| canceled.load(Ordering::Relaxed),
                    );
                    PreviewShared::finish(&shared, &key);
                    lock_unpoison(&metrics).record_preview_completed(started_at.elapsed());
                    if canceled.load(Ordering::Relaxed) {
                        continue;
                    }
                    if result_tx
                        .send(JobResult::Preview(Box::new(PreviewBuild {
                            token: request.token,
                            entry: request.entry,
                            variant: request.variant,
                            code_line_limit: request.code_line_limit,
                            code_render_limit: request.code_render_limit,
                            result,
                        })))
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

    pub(in crate::app::jobs) fn submit(&self, request: PreviewRequest) -> bool {
        let key = PreviewJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        match request.priority {
            PreviewPriority::High => {
                if state.queued_high_keys.contains(&key) {
                    replace_preview_request(&mut state.pending_high, request, &key);
                    return true;
                }
                if state.queued_low_keys.remove(&key) {
                    remove_preview_request(&mut state.pending_low, &key);
                    lock_unpoison(&self.metrics).preview_promotions += 1;
                }
                let evicted = trim_preview_queue_for_high(&mut state);
                cancel_stale_active_previews(&state, &key);
                state.queued_high_keys.insert(key);
                state.pending_high.push_back(request);
                let mut metrics = lock_unpoison(&self.metrics);
                metrics.preview_jobs_submitted_high += 1;
                metrics.preview_low_priority_evictions += evicted;
            }
            PreviewPriority::Low => {
                if state.queued_high_keys.contains(&key) {
                    replace_preview_request_with_priority(
                        &mut state.pending_high,
                        request,
                        &key,
                        PreviewPriority::High,
                    );
                    return true;
                }
                if state.queued_low_keys.contains(&key) {
                    replace_preview_request(&mut state.pending_low, request, &key);
                    return true;
                }
                if preview_active_contains(&state, &key) {
                    return true;
                }
                let evicted = if preview_pending_len(&state) >= state.capacity {
                    u64::from(evict_oldest_low_priority_preview(&mut state))
                } else {
                    0
                };
                if preview_pending_len(&state) >= state.capacity && evicted == 0 {
                    return true;
                }
                state.queued_low_keys.insert(key);
                state.pending_low.push_back(request);
                let mut metrics = lock_unpoison(&self.metrics);
                metrics.preview_jobs_submitted_low += 1;
                metrics.preview_low_priority_evictions += evicted;
            }
        }
        self.shared.available.notify_one();
        true
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        !state.pending_high.is_empty()
            || !state.pending_low.is_empty()
            || !state.active_keys.is_empty()
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn pending_keys(
        &self,
        priority: PreviewPriority,
    ) -> Vec<PreviewJobKey> {
        let state = lock_unpoison(&self.shared.state);
        let queue = match priority {
            PreviewPriority::High => &state.pending_high,
            PreviewPriority::Low => &state.pending_low,
        };
        queue.iter().map(PreviewJobKey::from_request).collect()
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn active_keys(&self) -> Vec<PreviewJobKey> {
        let mut keys = lock_unpoison(&self.shared.state)
            .active_jobs
            .iter()
            .map(|job| job.key.clone())
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.path.cmp(&right.path));
        keys
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn pending_len(&self, priority: PreviewPriority) -> usize {
        let state = lock_unpoison(&self.shared.state);
        match priority {
            PreviewPriority::High => state.pending_high.len(),
            PreviewPriority::Low => state.pending_low.len(),
        }
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn active_len(&self) -> usize {
        lock_unpoison(&self.shared.state).active_jobs.len()
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn canceled_active_keys(&self) -> Vec<PreviewJobKey> {
        let mut keys = lock_unpoison(&self.shared.state)
            .active_jobs
            .iter()
            .filter(|job| job.canceled.load(Ordering::Relaxed))
            .map(|job| job.key.clone())
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.path.cmp(&right.path));
        keys
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn pop_next_pending_for_tests(&self) -> Option<PreviewRequest> {
        let mut state = lock_unpoison(&self.shared.state);
        if let Some(request) = state.pending_high.pop_front() {
            let key = PreviewJobKey::from_request(&request);
            state.queued_high_keys.remove(&key);
            let _ = start_preview_request(&mut state, key, &request);
            return Some(request);
        }
        pop_low_priority_request(&mut state).map(|(request, _)| request)
    }
}

impl Drop for PreviewPool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending_high.clear();
            state.pending_low.clear();
            state.queued_high_keys.clear();
            state.queued_low_keys.clear();
            for job in &state.active_jobs {
                job.canceled.store(true, Ordering::Relaxed);
            }
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl PreviewShared {
    fn pop(shared: &Arc<Self>) -> Option<(PreviewRequest, Arc<AtomicBool>)> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending_high.pop_front() {
                let key = PreviewJobKey::from_request(&request);
                state.queued_high_keys.remove(&key);
                let canceled = start_preview_request(&mut state, key, &request);
                return Some((request, canceled));
            }
            if let Some(request) = pop_low_priority_request(&mut state) {
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &PreviewJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active_keys.remove(key);
        state.active_jobs.retain(|job| &job.key != key);
        shared.available.notify_all();
    }
}

impl PreviewJobKey {
    fn from_request(request: &PreviewRequest) -> Self {
        Self {
            path: request.entry.path.clone(),
            size: request.entry.size,
            modified: request.entry.modified,
            variant: request.variant.clone(),
            code_line_limit: request.code_line_limit,
            code_render_limit: request.code_render_limit,
        }
    }
}

fn remove_preview_request(queue: &mut VecDeque<PreviewRequest>, key: &PreviewJobKey) {
    if let Some(index) = queue
        .iter()
        .position(|request| PreviewJobKey::from_request(request) == *key)
    {
        queue.remove(index);
    }
}

fn start_preview_request(
    state: &mut PreviewState,
    key: PreviewJobKey,
    request: &PreviewRequest,
) -> Arc<AtomicBool> {
    let canceled = Arc::new(AtomicBool::new(false));
    state.active_keys.insert(key.clone());
    state.active_jobs.push(ActivePreviewJob {
        key,
        priority: request.priority,
        work_class: request.work_class,
        canceled: Arc::clone(&canceled),
    });
    canceled
}

fn pop_low_priority_request(state: &mut PreviewState) -> Option<(PreviewRequest, Arc<AtomicBool>)> {
    let heavy_limit_reached =
        active_low_priority_heavy_count(state) >= MAX_CONCURRENT_LOW_PRIORITY_HEAVY_PREVIEWS;
    let index = if heavy_limit_reached {
        state
            .pending_low
            .iter()
            .position(|request| request.work_class != PreviewWorkClass::Heavy)?
    } else {
        0
    };
    let request = state.pending_low.remove(index)?;
    let key = PreviewJobKey::from_request(&request);
    state.queued_low_keys.remove(&key);
    let canceled = start_preview_request(state, key, &request);
    Some((request, canceled))
}

fn active_low_priority_heavy_count(state: &PreviewState) -> usize {
    state
        .active_jobs
        .iter()
        .filter(|job| {
            job.priority == PreviewPriority::Low && job.work_class == PreviewWorkClass::Heavy
        })
        .count()
}

fn replace_preview_request(
    queue: &mut VecDeque<PreviewRequest>,
    request: PreviewRequest,
    key: &PreviewJobKey,
) {
    let priority = request.priority;
    replace_preview_request_with_priority(queue, request, key, priority);
}

fn replace_preview_request_with_priority(
    queue: &mut VecDeque<PreviewRequest>,
    mut request: PreviewRequest,
    key: &PreviewJobKey,
    priority: PreviewPriority,
) {
    request.priority = priority;
    if let Some(index) = queue
        .iter()
        .position(|queued| PreviewJobKey::from_request(queued) == *key)
    {
        queue[index] = request;
    }
}

fn preview_pending_len(state: &PreviewState) -> usize {
    state.pending_high.len() + state.pending_low.len()
}

fn preview_active_contains(state: &PreviewState, key: &PreviewJobKey) -> bool {
    state.active_jobs.iter().any(|job| job.key == *key)
}

fn cancel_stale_active_previews(state: &PreviewState, keep: &PreviewJobKey) {
    for job in &state.active_jobs {
        if &job.key != keep {
            job.canceled.store(true, Ordering::Relaxed);
        }
    }
}

fn evict_oldest_low_priority_preview(state: &mut PreviewState) -> bool {
    let Some(stale) = state.pending_low.pop_front() else {
        return false;
    };
    state
        .queued_low_keys
        .remove(&PreviewJobKey::from_request(&stale));
    true
}

fn trim_preview_queue_for_high(state: &mut PreviewState) -> u64 {
    let mut evicted = 0;
    while preview_pending_len(state) >= state.capacity {
        if evict_oldest_low_priority_preview(state) {
            evicted += 1;
            continue;
        }

        let Some(stale) = state.pending_high.pop_front() else {
            break;
        };
        state
            .queued_high_keys
            .remove(&PreviewJobKey::from_request(&stale));
    }
    evicted
}
