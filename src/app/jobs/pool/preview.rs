use super::*;
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::{Instant, SystemTime},
};

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
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app::jobs) struct PreviewJobKey {
    pub(in crate::app::jobs) path: PathBuf,
    pub(in crate::app::jobs) size: u64,
    pub(in crate::app::jobs) modified: Option<SystemTime>,
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
                while let Some(request) = PreviewShared::pop(&shared) {
                    let key = PreviewJobKey::from_entry(&request.entry);
                    let started_at = Instant::now();
                    let result = crate::preview::build_preview(&request.entry);
                    PreviewShared::finish(&shared, &key);
                    lock_unpoison(&metrics).record_preview_completed(started_at.elapsed());
                    if result_tx
                        .send(JobResult::Preview(Box::new(PreviewBuild {
                            token: request.token,
                            entry: request.entry,
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
        let key = PreviewJobKey::from_entry(&request.entry);
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
        queue
            .iter()
            .map(|request| PreviewJobKey::from_entry(&request.entry))
            .collect()
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn active_keys(&self) -> Vec<PreviewJobKey> {
        let mut keys = lock_unpoison(&self.shared.state)
            .active_keys
            .iter()
            .cloned()
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
        lock_unpoison(&self.shared.state).active_keys.len()
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
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl PreviewShared {
    fn pop(shared: &Arc<Self>) -> Option<PreviewRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending_high.pop_front() {
                let key = PreviewJobKey::from_entry(&request.entry);
                state.queued_high_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            if let Some(request) = state.pending_low.pop_front() {
                let key = PreviewJobKey::from_entry(&request.entry);
                state.queued_low_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &PreviewJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active_keys.remove(key);
    }
}

impl PreviewJobKey {
    fn from_entry(entry: &Entry) -> Self {
        Self {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
        }
    }
}

fn remove_preview_request(queue: &mut VecDeque<PreviewRequest>, key: &PreviewJobKey) {
    if let Some(index) = queue
        .iter()
        .position(|request| PreviewJobKey::from_entry(&request.entry) == *key)
    {
        queue.remove(index);
    }
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
        .position(|queued| PreviewJobKey::from_entry(&queued.entry) == *key)
    {
        queue[index] = request;
    }
}

fn preview_pending_len(state: &PreviewState) -> usize {
    state.pending_high.len() + state.pending_low.len()
}

fn evict_oldest_low_priority_preview(state: &mut PreviewState) -> bool {
    let Some(stale) = state.pending_low.pop_front() else {
        return false;
    };
    state
        .queued_low_keys
        .remove(&PreviewJobKey::from_entry(&stale.entry));
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
            .remove(&PreviewJobKey::from_entry(&stale.entry));
    }
    evicted
}
