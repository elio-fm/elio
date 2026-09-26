//! A browser-only worker, independent of selected-folder preview statistics.
use super::scheduler::{lock_unpoison, wait_unpoison};
use crate::filesystem::{DirectoryStatsScanResult, scan_directory_stats};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread,
};

pub(crate) struct FolderSizeResult {
    pub token: u64,
    pub path: PathBuf,
    pub size: Option<u64>,
}

#[derive(Default)]
struct State {
    token: u64,
    pending: Option<Vec<PathBuf>>,
    results: VecDeque<FolderSizeResult>,
    stopped: bool,
}

pub(crate) struct FolderSizeWorker {
    shared: Arc<(Mutex<State>, Condvar)>,
}

impl FolderSizeWorker {
    pub(crate) fn new() -> Self {
        Self::with_scanner(scan_directory_stats)
    }

    fn with_scanner(
        scan: impl Fn(&std::path::Path, &dyn Fn() -> bool) -> DirectoryStatsScanResult + Send + 'static,
    ) -> Self {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker = Arc::clone(&shared);
        thread::spawn(move || {
            let (mutex, ready) = &*worker;
            loop {
                let (token, paths) = {
                    let mut state = lock_unpoison(mutex);
                    while state.pending.is_none() && !state.stopped {
                        state = wait_unpoison(ready, state);
                    }
                    if state.stopped {
                        return;
                    }
                    (state.token, state.pending.take().unwrap())
                };
                for path in paths {
                    let canceled = || {
                        let state = lock_unpoison(mutex);
                        state.stopped || state.token != token
                    };
                    let result = scan(&path, &canceled);
                    let mut state = lock_unpoison(mutex);
                    if state.stopped {
                        return;
                    }
                    if state.token != token {
                        break;
                    }
                    let size = match result {
                        DirectoryStatsScanResult::Complete(stats) => Some(stats.total_size_bytes),
                        DirectoryStatsScanResult::Incomplete { .. } => None,
                        DirectoryStatsScanResult::Canceled => break,
                    };
                    // Bound unread results as well as the single replaceable request.
                    while state.results.len() >= 256 && state.token == token && !state.stopped {
                        state = wait_unpoison(ready, state);
                    }
                    if state.stopped {
                        return;
                    }
                    if state.token != token {
                        break;
                    }
                    state
                        .results
                        .push_back(FolderSizeResult { token, path, size });
                }
            }
        });
        Self { shared }
    }

    pub(crate) fn replace(&self, paths: Vec<PathBuf>) -> u64 {
        let (mutex, ready) = &*self.shared;
        let mut state = lock_unpoison(mutex);
        state.token = state.token.wrapping_add(1);
        state.results.clear();
        state.pending = (!paths.is_empty()).then_some(paths);
        ready.notify_all();
        state.token
    }

    pub(crate) fn drain(&self) -> Vec<FolderSizeResult> {
        let (mutex, ready) = &*self.shared;
        let mut state = lock_unpoison(mutex);
        let results = state.results.drain(..).collect();
        ready.notify_all();
        results
    }
}

#[cfg(test)]
#[path = "tests/folder_sizes.rs"]
mod tests;

impl Drop for FolderSizeWorker {
    fn drop(&mut self) {
        let (mutex, ready) = &*self.shared;
        lock_unpoison(mutex).stopped = true;
        ready.notify_all();
    }
}
