use super::*;
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::SystemTime,
};

pub(in crate::app::jobs) struct PdfRenderPool {
    shared: Arc<PdfRenderShared>,
    workers: Vec<thread::JoinHandle<()>>,
}

struct PdfRenderShared {
    state: Mutex<PdfRenderState>,
    available: Condvar,
}

struct PdfRenderState {
    pending_current: VecDeque<PdfRenderRequest>,
    pending_prefetch: VecDeque<PdfRenderRequest>,
    queued_current_keys: HashSet<PdfRenderJobKey>,
    queued_prefetch_keys: HashSet<PdfRenderJobKey>,
    active_keys: HashSet<PdfRenderJobKey>,
    closed: bool,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app::jobs) struct PdfRenderJobKey {
    pub(in crate::app::jobs) path: PathBuf,
    pub(in crate::app::jobs) size: u64,
    pub(in crate::app::jobs) modified: Option<SystemTime>,
    pub(in crate::app::jobs) page: usize,
    pub(in crate::app::jobs) width_px: u32,
    pub(in crate::app::jobs) height_px: u32,
}

impl PdfRenderPool {
    pub(in crate::app::jobs) fn new(
        worker_count: usize,
        capacity: usize,
        result_tx: mpsc::Sender<JobResult>,
    ) -> Self {
        let shared = Arc::new(PdfRenderShared {
            state: Mutex::new(PdfRenderState {
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
                while let Some(request) = PdfRenderShared::pop(&shared) {
                    let key = PdfRenderJobKey::from_request(&request);
                    let result = overlays::pdf::render_pdf_page_to_cache(
                        &request.path,
                        request.size,
                        request.modified,
                        request.page,
                        request.width_px,
                        request.height_px,
                    );
                    let (sixel_dcs, sixel_dcs_key) = match result.as_ref().ok().and_then(|path| {
                        path.as_ref().and_then(|path| {
                            request.sixel_prepare.as_ref().and_then(|config| {
                                let placement = ratatui::layout::Rect {
                                    x: 0,
                                    y: 0,
                                    width: config.area_width,
                                    height: config.area_height,
                                };
                                let (target_w, target_h) = overlays::inline_image::area_pixel_size(
                                    placement,
                                    config.window_size,
                                );
                                let dcs = overlays::inline_image::encode_sixel_dcs(
                                    path, target_w, target_h,
                                )
                                .ok()?;
                                let dcs_key = overlays::images::SixelDcsKey::new(
                                    path,
                                    placement,
                                    config.window_size,
                                );
                                Some((dcs, dcs_key))
                            })
                        })
                    }) {
                        Some((dcs, dcs_key)) => (Some(dcs), Some(dcs_key)),
                        None => (None, None),
                    };
                    PdfRenderShared::finish(&shared, &key);
                    if result_tx
                        .send(JobResult::PdfRender(PdfRenderBuild {
                            path: request.path,
                            size: request.size,
                            modified: request.modified,
                            page: request.page,
                            width_px: request.width_px,
                            height_px: request.height_px,
                            sixel_dcs,
                            sixel_dcs_key,
                            result: result.map_err(|error| error.to_string()),
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
        request: PdfRenderRequest,
        priority: PdfJobPriority,
    ) -> bool {
        let key = PdfRenderJobKey::from_request(&request);
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
                    remove_pdf_render_request(&mut state.pending_prefetch, &key);
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
        while pdf_render_pending_len(&state) >= state.capacity {
            if let Some(stale) = state.pending_prefetch.pop_front() {
                state
                    .queued_prefetch_keys
                    .remove(&PdfRenderJobKey::from_request(&stale));
                continue;
            }
            if let Some(stale) = state.pending_current.pop_front() {
                state
                    .queued_current_keys
                    .remove(&PdfRenderJobKey::from_request(&stale));
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
    pub(in crate::app::jobs) fn pending_keys(&self) -> Vec<PdfRenderJobKey> {
        let state = lock_unpoison(&self.shared.state);
        state
            .pending_current
            .iter()
            .chain(state.pending_prefetch.iter())
            .map(PdfRenderJobKey::from_request)
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
        keep_variants: &[(usize, u32, u32)],
    ) {
        let mut state = lock_unpoison(&self.shared.state);
        let pending_current = std::mem::take(&mut state.pending_current);
        let pending_prefetch = std::mem::take(&mut state.pending_prefetch);
        state.queued_current_keys.clear();
        state.queued_prefetch_keys.clear();
        state.pending_current = retain_pdf_render_requests(
            pending_current,
            path,
            size,
            modified,
            keep_variants,
            &mut state.queued_current_keys,
        );
        state.pending_prefetch = retain_pdf_render_requests(
            pending_prefetch,
            path,
            size,
            modified,
            keep_variants,
            &mut state.queued_prefetch_keys,
        );
    }
}

impl Drop for PdfRenderPool {
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

impl PdfRenderShared {
    fn pop(shared: &Arc<Self>) -> Option<PdfRenderRequest> {
        let mut state = lock_unpoison(&shared.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.pending_current.pop_front() {
                let key = PdfRenderJobKey::from_request(&request);
                state.queued_current_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            if let Some(request) = state.pending_prefetch.pop_front() {
                let key = PdfRenderJobKey::from_request(&request);
                state.queued_prefetch_keys.remove(&key);
                state.active_keys.insert(key);
                return Some(request);
            }
            state = wait_unpoison(&shared.available, state);
        }
    }

    fn finish(shared: &Arc<Self>, key: &PdfRenderJobKey) {
        let mut state = lock_unpoison(&shared.state);
        state.active_keys.remove(key);
    }
}

impl PdfRenderJobKey {
    fn from_request(request: &PdfRenderRequest) -> Self {
        Self {
            path: request.path.clone(),
            size: request.size,
            modified: request.modified,
            page: request.page,
            width_px: request.width_px,
            height_px: request.height_px,
        }
    }
}

fn pdf_render_pending_len(state: &PdfRenderState) -> usize {
    state.pending_current.len() + state.pending_prefetch.len()
}

fn remove_pdf_render_request(pending: &mut VecDeque<PdfRenderRequest>, key: &PdfRenderJobKey) {
    pending.retain(|request| PdfRenderJobKey::from_request(request) != *key);
}

fn retain_pdf_render_requests(
    pending: VecDeque<PdfRenderRequest>,
    path: &Path,
    size: u64,
    modified: Option<SystemTime>,
    keep_variants: &[(usize, u32, u32)],
    queued_keys: &mut HashSet<PdfRenderJobKey>,
) -> VecDeque<PdfRenderRequest> {
    let mut retained = VecDeque::with_capacity(pending.len());
    for request in pending {
        let keep = request.path == path
            && request.size == size
            && request.modified == modified
            && keep_variants.contains(&(request.page, request.width_px, request.height_px));
        if keep {
            queued_keys.insert(PdfRenderJobKey::from_request(&request));
            retained.push_back(request);
        }
    }
    retained
}
