use super::*;
use crate::app::jobs::SixelPrepareConfig;
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::SystemTime,
};

pub(in crate::app::jobs) struct ImagePreparePool {
    shared: Arc<ImagePrepareShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct ImagePrepareShared {
    state: Mutex<ImagePrepareState>,
    available: Condvar,
}

struct ImagePrepareState {
    pending_current: VecDeque<ImagePrepareRequest>,
    pending_nearby: VecDeque<ImagePrepareRequest>,
    queued_current_keys: HashSet<ImagePrepareJobKey>,
    queued_nearby_keys: HashSet<ImagePrepareJobKey>,
    active: Vec<(ImagePrepareJobKey, Arc<AtomicBool>)>,
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app::jobs) struct ImagePrepareJobKey {
    pub(in crate::app::jobs) path: PathBuf,
    pub(in crate::app::jobs) size: u64,
    pub(in crate::app::jobs) modified: Option<SystemTime>,
    pub(in crate::app::jobs) target_width_px: u32,
    pub(in crate::app::jobs) target_height_px: u32,
    pub(in crate::app::jobs) force_render_to_cache: bool,
    pub(in crate::app::jobs) prepare_inline_payload: bool,
    /// Included so that Sixel and non-Sixel jobs for the same image are not
    /// incorrectly deduplicated against each other.
    pub(in crate::app::jobs) sixel_prepare: Option<SixelPrepareConfig>,
}

impl ImagePreparePool {
    pub(in crate::app::jobs) fn new(
        worker_count: usize,
        capacity: usize,
        result_tx: mpsc::Sender<JobResult>,
    ) -> Self {
        let shared = Arc::new(ImagePrepareShared {
            state: Mutex::new(ImagePrepareState {
                pending_current: VecDeque::new(),
                pending_nearby: VecDeque::new(),
                queued_current_keys: HashSet::new(),
                queued_nearby_keys: HashSet::new(),
                active: Vec::new(),
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
                while let Some((request, canceled)) = ImagePrepareShared::pop(&shared) {
                    let key = ImagePrepareJobKey::from_request(&request);
                    let result = overlays::images::prepare_static_image_asset(&request, || {
                        canceled.load(Ordering::Relaxed)
                    });
                    ImagePrepareShared::finish(&shared, &key);
                    if result_tx
                        .send(JobResult::ImagePrepare(ImagePrepareBuild {
                            path: request.path,
                            size: request.size,
                            modified: request.modified,
                            target_width_px: request.target_width_px,
                            target_height_px: request.target_height_px,
                            force_render_to_cache: request.force_render_to_cache,
                            prepare_inline_payload: request.prepare_inline_payload,
                            canceled: canceled.load(Ordering::Relaxed),
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
        request: ImagePrepareRequest,
        priority: ImageJobPriority,
    ) -> bool {
        let key = ImagePrepareJobKey::from_request(&request);
        let mut state = lock_unpoison(&self.shared.state);
        if state.closed {
            return false;
        }
        match priority {
            ImageJobPriority::Current => {
                if state.queued_current_keys.contains(&key)
                    || image_prepare_active_contains(&state, &key)
                {
                    return true;
                }
                if state.queued_nearby_keys.remove(&key) {
                    remove_image_prepare_request(&mut state.pending_nearby, &key);
                }
                for (active_key, canceled) in &state.active {
                    if active_key != &key {
                        canceled.store(true, Ordering::Relaxed);
                    }
                }
            }
            ImageJobPriority::Nearby => {
                if state.queued_current_keys.contains(&key)
                    || state.queued_nearby_keys.contains(&key)
                    || image_prepare_active_contains(&state, &key)
                {
                    return true;
                }
            }
        }
        while image_prepare_pending_len(&state) >= state.capacity {
            if let Some(stale) = state.pending_nearby.pop_front() {
                state
                    .queued_nearby_keys
                    .remove(&ImagePrepareJobKey::from_request(&stale));
                continue;
            }
            if let Some(stale) = state.pending_current.pop_front() {
                state
                    .queued_current_keys
                    .remove(&ImagePrepareJobKey::from_request(&stale));
                continue;
            }
            break;
        }
        match priority {
            ImageJobPriority::Current => {
                state.queued_current_keys.insert(key);
                state.pending_current.push_back(request);
            }
            ImageJobPriority::Nearby => {
                state.queued_nearby_keys.insert(key);
                state.pending_nearby.push_back(request);
            }
        }
        self.shared.available.notify_one();
        true
    }

    pub(in crate::app::jobs) fn has_pending_work(&self) -> bool {
        let state = lock_unpoison(&self.shared.state);
        !state.pending_current.is_empty()
            || !state.pending_nearby.is_empty()
            || !state.active.is_empty()
    }

    pub(in crate::app::jobs) fn retain_pending(
        &self,
        current: Option<&ImagePrepareRequest>,
        nearby: &[ImagePrepareRequest],
    ) {
        let promoted;
        {
            let mut state = lock_unpoison(&self.shared.state);
            let keep_current = current.map(ImagePrepareJobKey::from_request);
            let keep_nearby = nearby
                .iter()
                .map(ImagePrepareJobKey::from_request)
                .collect::<HashSet<_>>();

            let pending_current = std::mem::take(&mut state.pending_current);
            let pending_nearby = std::mem::take(&mut state.pending_nearby);
            state.queued_current_keys.clear();
            state.queued_nearby_keys.clear();

            // Identify a nearby-queued job that needs to be promoted to current
            // priority.  This happens when a job was previously prefetched at Nearby
            // priority for an adjacent entry and the user has since navigated to that
            // entry.  Without promotion the job would be silently dropped from
            // pending_nearby without appearing in pending_current, while the app's
            // pending_prepares set would still hold the key — causing ensure_static_
            // image_preload to skip re-submission and leaving the preview blank.
            let promote_candidate = keep_current.as_ref().and_then(|key| {
                if image_prepare_active_contains(&state, key) {
                    return None;
                }
                pending_nearby
                    .iter()
                    .find(|r| &ImagePrepareJobKey::from_request(r) == key)
                    .cloned()
            });

            state.pending_current = pending_current
                .into_iter()
                .filter(|request| {
                    keep_current
                        .as_ref()
                        .is_some_and(|key| key == &ImagePrepareJobKey::from_request(request))
                })
                .inspect(|request| {
                    state
                        .queued_current_keys
                        .insert(ImagePrepareJobKey::from_request(request));
                })
                .collect();

            promoted = if let Some(ref request) = promote_candidate {
                let key = ImagePrepareJobKey::from_request(request);
                if !state.queued_current_keys.contains(&key) {
                    state.queued_current_keys.insert(key);
                    state.pending_current.push_back(request.clone());
                    true
                } else {
                    false
                }
            } else {
                false
            };

            state.pending_nearby = pending_nearby
                .into_iter()
                .filter(|request| {
                    let key = ImagePrepareJobKey::from_request(request);
                    // Jobs matching keep_current were either already in pending_current
                    // or just promoted above; remove them from the nearby queue either way.
                    if keep_current.as_ref() == Some(&key) {
                        return false;
                    }
                    keep_nearby.contains(&key)
                })
                .inspect(|request| {
                    state
                        .queued_nearby_keys
                        .insert(ImagePrepareJobKey::from_request(request));
                })
                .collect();
            for (key, canceled) in &state.active {
                let keep = keep_current.as_ref() == Some(key) || keep_nearby.contains(key);
                if !keep {
                    canceled.store(true, Ordering::Relaxed);
                }
            }
        }
        if promoted {
            self.shared.available.notify_one();
        }
    }

    #[cfg(test)]
    pub(in crate::app::jobs) fn pending_keys(&self) -> Vec<ImagePrepareJobKey> {
        let state = lock_unpoison(&self.shared.state);
        state
            .pending_current
            .iter()
            .chain(state.pending_nearby.iter())
            .map(ImagePrepareJobKey::from_request)
            .collect()
    }
}

impl Drop for ImagePreparePool {
    fn drop(&mut self) {
        {
            let mut state = lock_unpoison(&self.shared.state);
            state.closed = true;
            state.pending_current.clear();
            state.pending_nearby.clear();
            state.queued_current_keys.clear();
            state.queued_nearby_keys.clear();
            for (_, canceled) in &state.active {
                canceled.store(true, Ordering::Relaxed);
            }
        }
        self.shared.available.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl ImagePrepareShared {
    fn pop(shared: &Arc<Self>) -> Option<(ImagePrepareRequest, Arc<AtomicBool>)> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            let request = state
                .pending_current
                .pop_front()
                .inspect(|request| {
                    state
                        .queued_current_keys
                        .remove(&ImagePrepareJobKey::from_request(request));
                })
                .or_else(|| {
                    state.pending_nearby.pop_front().inspect(|request| {
                        state
                            .queued_nearby_keys
                            .remove(&ImagePrepareJobKey::from_request(request));
                    })
                });
            if let Some(request) = request {
                let key = ImagePrepareJobKey::from_request(&request);
                let canceled = Arc::new(AtomicBool::new(false));
                state.active.push((key, Arc::clone(&canceled)));
                return Some((request, canceled));
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &ImagePrepareJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active.retain(|(active_key, _)| active_key != key);
    }
}

impl ImagePrepareJobKey {
    fn from_request(request: &ImagePrepareRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            target_width_px: request.target_width_px,
            target_height_px: request.target_height_px,
            force_render_to_cache: request.force_render_to_cache,
            prepare_inline_payload: request.prepare_inline_payload,
            sixel_prepare: request.sixel_prepare.clone(),
        }
    }
}

fn image_prepare_pending_len(state: &ImagePrepareState) -> usize {
    state.pending_current.len() + state.pending_nearby.len()
}

fn image_prepare_active_contains(state: &ImagePrepareState, key: &ImagePrepareJobKey) -> bool {
    state.active.iter().any(|(active_key, _)| active_key == key)
}

fn remove_image_prepare_request(
    pending: &mut VecDeque<ImagePrepareRequest>,
    key: &ImagePrepareJobKey,
) {
    pending.retain(|request| ImagePrepareJobKey::from_request(request) != *key);
}
