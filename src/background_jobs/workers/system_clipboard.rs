use super::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

/// Keeps reads from external clipboard owners off the terminal UI thread.
pub(in crate::background_jobs) struct SystemClipboardReadPool {
    request_tx: Option<mpsc::Sender<SystemClipboardReadRequest>>,
    pending: Arc<AtomicUsize>,
    worker: Option<thread::JoinHandle<()>>,
}

impl SystemClipboardReadPool {
    pub(in crate::background_jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<SystemClipboardReadRequest>();
        let pending = Arc::new(AtomicUsize::new(0));
        let worker_pending = Arc::clone(&pending);
        let worker = thread::Builder::new()
            .name("elio-system-clipboard-reader".to_string())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let build = match request {
                        SystemClipboardReadRequest::Files { token, cwd } => {
                            SystemClipboardReadBuild {
                                token,
                                result: SystemClipboardRead::Files {
                                    cwd,
                                    clipboard: crate::system_clipboard::read_files(),
                                },
                            }
                        }
                        SystemClipboardReadRequest::Text {
                            token,
                            target,
                            input_generation,
                        } => SystemClipboardReadBuild {
                            token,
                            result: SystemClipboardRead::Text {
                                target,
                                input_generation,
                                text: crate::system_clipboard::read_text(),
                            },
                        },
                    };
                    worker_pending.fetch_sub(1, Ordering::Relaxed);
                    if result_tx
                        .send(JobResult::SystemClipboardRead(build))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("start system clipboard reader worker");
        Self {
            request_tx: Some(request_tx),
            pending,
            worker: Some(worker),
        }
    }

    pub(in crate::background_jobs) fn submit(&self, request: SystemClipboardReadRequest) -> bool {
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

impl Drop for SystemClipboardReadPool {
    fn drop(&mut self) {
        self.request_tx = None;
        if let Some(worker) = self.worker.take() {
            // A faulty external clipboard owner can keep this thread blocked.
            // Do not let that delay terminal shutdown.
            drop(worker);
        }
    }
}
