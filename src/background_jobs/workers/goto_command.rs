use super::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

pub(in crate::background_jobs) struct GotoCommandPool {
    request_tx: Option<mpsc::Sender<GotoCommandRequest>>,
    pending: Arc<AtomicUsize>,
    worker: Option<thread::JoinHandle<()>>,
}

impl GotoCommandPool {
    pub(in crate::background_jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<GotoCommandRequest>();
        let pending = Arc::new(AtomicUsize::new(0));
        let worker_pending = Arc::clone(&pending);
        let worker = thread::spawn(move || {
            while let Ok(request) = request_rx.recv() {
                let result =
                    crate::goto_menu::resolve_command_destination(&request.command, &request.cwd);
                worker_pending.fetch_sub(1, Ordering::Relaxed);
                if result_tx
                    .send(JobResult::GotoCommand(GotoCommandBuild {
                        token: request.token,
                        title: request.title,
                        result,
                    }))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            request_tx: Some(request_tx),
            pending,
            worker: Some(worker),
        }
    }

    pub(in crate::background_jobs) fn submit(&self, request: GotoCommandRequest) -> bool {
        let Some(request_tx) = &self.request_tx else {
            return false;
        };
        self.pending.fetch_add(1, Ordering::Relaxed);
        if request_tx.send(request).is_ok() {
            true
        } else {
            self.pending.fetch_sub(1, Ordering::Relaxed);
            false
        }
    }

    pub(in crate::background_jobs) fn has_pending_work(&self) -> bool {
        self.pending.load(Ordering::Relaxed) > 0
    }
}

impl Drop for GotoCommandPool {
    fn drop(&mut self) {
        self.request_tx = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
