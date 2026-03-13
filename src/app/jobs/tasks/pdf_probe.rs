use super::*;
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::SystemTime,
};

pub(in crate::app::jobs) struct PdfProbePool {
    shared: Arc<PdfProbeShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct PdfProbeShared {
    state: Mutex<PdfProbeState>,
    available: Condvar,
}

struct PdfProbeState {
    pending_current: VecDeque<PdfProbeRequest>,
    pending_prefetch: VecDeque<PdfProbeRequest>,
    queued_current_keys: HashSet<PdfProbeJobKey>,
    queued_prefetch_keys: HashSet<PdfProbeJobKey>,
    active_keys: HashSet<PdfProbeJobKey>,
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app::jobs) struct PdfProbeJobKey {
    pub(in crate::app::jobs) path: PathBuf,
    pub(in crate::app::jobs) size: u64,
    pub(in crate::app::jobs) modified: Option<SystemTime>,
    pub(in crate::app::jobs) page: usize,
}

impl PdfProbePool {
    pub(in crate::app::jobs) fn new(
        worker_count: usize,
        capacity: usize,
        result_tx: mpsc::Sender<JobResult>,
    ) -> Self {
        let shared = Arc::new(PdfProbeShared {
            state: Mutex::new(PdfProbeState {
                pending_current: VecDeque::new(),
                pending_prefetch: VecDeque::new(),
                queued_current_keys: HashSet::new(),
                queued_prefetch_keys: HashSet::new(),
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
            workers.push(thread::spawn(move || {
                while let Some(request) = PdfProbeShared::pop(&shared) {
                    let key = PdfProbeJobKey::from_request(&request);
                    let result = overlays::pdf::probe_pdf_page(&request.path, request.page)
                        .map_err(|error| error.to_string());
                    PdfProbeShared::finish(&shared, &key);
                    if result_tx
                        .send(JobResult::PdfProbe(PdfProbeBuild {
                            path: request.path,
                            size: request.size,
                            modified: request.modified,
                            page: request.page,
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

    pub(in crate::app::jobs) fn submit(
        &self,
        request: PdfProbeRequest,
        priority: PdfJobPriority,
    ) -> bool {
        let key = PdfProbeJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        match priority {
            PdfJobPriority::Current => {
                if state.queued_current_keys.contains(&key) || state.active_keys.contains(&key) {
                    return true;
                }
                if state.queued_prefetch_keys.remove(&key) {
                    remove_pdf_probe_request(&mut state.pending_prefetch, &key);
                }
            }
            PdfJobPriority::Prefetch => {
                if state.queued_current_keys.contains(&key)
                    || state.queued_prefetch_keys.contains(&key)
                    || state.active_keys.contains(&key)
                {
                    return true;
                }
            }
        }
        while pdf_probe_pending_len(&state) >= state.capacity {
            if let Some(stale) = state.pending_prefetch.pop_front() {
                state
                    .queued_prefetch_keys
                    .remove(&PdfProbeJobKey::from_request(&stale));
                continue;
            }
            if let Some(stale) = state.pending_current.pop_front() {
                state
                    .queued_current_keys
                    .remove(&PdfProbeJobKey::from_request(&stale));
                continue;
            }
            break;
        }
        match priority {
            PdfJobPriority::Current => {
                state.queued_current_keys.insert(key);
                state.pending_current.push_back(request);
            }
            PdfJobPriority::Prefetch => {
                state.queued_prefetch_keys.insert(key);
                state.pending_prefetch.push_back(request);
            }
        }
        self.shared.available.notify_one();
        true
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        !state.pending_current.is_empty()
            || !state.pending_prefetch.is_empty()
            || !state.active_keys.is_empty()
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn pending_keys(&self) -> Vec<PdfProbeJobKey> {
        let state = lock_unpoison(&self.shared.state);
        state
            .pending_current
            .iter()
            .chain(state.pending_prefetch.iter())
            .map(PdfProbeJobKey::from_request)
            .collect()
    }

    pub(in crate::app::jobs) fn clear_pending(&self) {
        let mut state = lock_unpoison(&self.shared.state);
        state.pending_current.clear();
        state.pending_prefetch.clear();
        state.queued_current_keys.clear();
        state.queued_prefetch_keys.clear();
    }

    pub(in crate::app::jobs) fn retain_pending(
        &self,
        path: &Path,
        size: u64,
        modified: Option<SystemTime>,
        keep_pages: &[usize],
    ) {
        let mut state = lock_unpoison(&self.shared.state);
        let pending_current = std::mem::take(&mut state.pending_current);
        let pending_prefetch = std::mem::take(&mut state.pending_prefetch);
        state.queued_current_keys.clear();
        state.queued_prefetch_keys.clear();
        state.pending_current = retain_pdf_probe_requests(
            pending_current,
            path,
            size,
            modified,
            keep_pages,
            &mut state.queued_current_keys,
        );
        state.pending_prefetch = retain_pdf_probe_requests(
            pending_prefetch,
            path,
            size,
            modified,
            keep_pages,
            &mut state.queued_prefetch_keys,
        );
    }
}

impl Drop for PdfProbePool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending_current.clear();
            state.pending_prefetch.clear();
            state.queued_current_keys.clear();
            state.queued_prefetch_keys.clear();
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl PdfProbeShared {
    fn pop(shared: &Arc<Self>) -> Option<PdfProbeRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending_current.pop_front() {
                let key = PdfProbeJobKey::from_request(&request);
                state.queued_current_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            if let Some(request) = state.pending_prefetch.pop_front() {
                let key = PdfProbeJobKey::from_request(&request);
                state.queued_prefetch_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &PdfProbeJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active_keys.remove(key);
    }
}

impl PdfProbeJobKey {
    fn from_request(request: &PdfProbeRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
        }
    }
}

fn pdf_probe_pending_len(state: &PdfProbeState) -> usize {
    state.pending_current.len() + state.pending_prefetch.len()
}

fn remove_pdf_probe_request(pending: &mut VecDeque<PdfProbeRequest>, key: &PdfProbeJobKey) {
    pending.retain(|request| PdfProbeJobKey::from_request(request) != *key);
}

fn retain_pdf_probe_requests(
    pending: VecDeque<PdfProbeRequest>,
    path: &Path,
    size: u64,
    modified: Option<SystemTime>,
    keep_pages: &[usize],
    queued_keys: &mut HashSet<PdfProbeJobKey>,
) -> VecDeque<PdfProbeRequest> {
    let mut retained = VecDeque::with_capacity(pending.len());
    for request in pending {
        let keep = request.path == path
            && request.size == size
            && request.modified == modified
            && keep_pages.contains(&request.page);
        if keep {
            queued_keys.insert(PdfProbeJobKey::from_request(&request));
            retained.push_back(request);
        }
    }
    retained
}
