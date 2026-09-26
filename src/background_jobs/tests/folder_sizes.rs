use super::*;
use crate::filesystem::DirectoryStats;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn folder_sizes_replacement_cancels_active_scan_and_skips_obsolete_queue() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = FolderSizeWorker::with_scanner(move |path, canceled| {
        started_tx.send(path.to_path_buf()).unwrap();
        if path == std::path::Path::new("old") {
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(canceled());
            // Even a completion racing with cancellation must not be published.
        }
        DirectoryStatsScanResult::Complete(DirectoryStats::default())
    });
    worker.replace(vec!["old".into(), "never".into()]);
    assert_eq!(
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        PathBuf::from("old")
    );
    worker.replace(vec!["superseded".into()]);
    let token = worker.replace(vec!["new".into()]);
    release_tx.send(()).unwrap();
    assert_eq!(
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        PathBuf::from("new")
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let results = worker.drain();
        if !results.is_empty() {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].token, token);
            assert_eq!(results[0].path, PathBuf::from("new"));
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert!(started_rx.try_recv().is_err());
    assert!(worker.drain().is_empty());
}

#[test]
fn folder_sizes_backpressure_is_bounded_and_cancel_wakes_worker() {
    let (started_tx, started_rx) = mpsc::channel();
    let worker = FolderSizeWorker::with_scanner(move |_, _| {
        started_tx.send(()).unwrap();
        DirectoryStatsScanResult::Complete(DirectoryStats::default())
    });
    worker.replace(vec![PathBuf::from("old"); 300]);
    for _ in 0..257 {
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    assert_eq!(lock_unpoison(&worker.shared.0).results.len(), 256);
    let token = worker.replace(vec!["new".into()]);
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let results = worker.drain();
        if !results.is_empty() {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].token, token);
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert!(started_rx.try_recv().is_err());
}
