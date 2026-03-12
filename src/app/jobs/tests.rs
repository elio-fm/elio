use super::*;

#[test]
fn preview_pool_deduplicates_identical_active_or_queued_requests() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 8);
    let entry = Entry {
        path: PathBuf::from("archive.zip"),
        name: "archive.zip".to_string(),
        name_key: "archive.zip".to_string(),
        kind: EntryKind::File,
        size: 42,
        modified: None,
        readonly: false,
    };

    assert!(scheduler.submit_preview(PreviewRequest {
        token: 1,
        entry: entry.clone(),
        priority: PreviewPriority::Low,
    }));
    assert!(scheduler.submit_preview(PreviewRequest {
        token: 2,
        entry,
        priority: PreviewPriority::Low,
    }));
    let snapshot = scheduler.snapshot();
    assert!(snapshot.preview_pending_high.is_empty());
    assert_eq!(
        snapshot.preview_pending_low,
        vec![PreviewJobKey {
            path: PathBuf::from("archive.zip"),
            size: 42,
            modified: None,
        }]
    );
    assert!(snapshot.preview_active.is_empty());
}

#[test]
fn search_pool_replaces_pending_request_with_latest_distinct_job() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 8);

    assert!(scheduler.submit_search(SearchRequest {
        token: 1,
        cwd: PathBuf::from("/tmp/a"),
        scope: SearchScope::Files,
    }));
    assert!(scheduler.submit_search(SearchRequest {
        token: 2,
        cwd: PathBuf::from("/tmp/b"),
        scope: SearchScope::Files,
    }));
    assert_eq!(
        scheduler.snapshot().search_pending,
        Some(SearchJobKey {
            cwd: PathBuf::from("/tmp/b"),
            scope: SearchScope::Files,
        })
    );
}

#[test]
fn preview_pool_discards_oldest_queued_request_when_full() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);

    for name in ["a.zip", "b.zip", "c.zip"] {
        assert!(scheduler.submit_preview(PreviewRequest {
            token: 1,
            entry: Entry {
                path: PathBuf::from(name),
                name: name.to_string(),
                name_key: name.to_string(),
                kind: EntryKind::File,
                size: 1,
                modified: None,
                readonly: false,
            },
            priority: PreviewPriority::Low,
        }));
    }

    assert!(scheduler.snapshot().preview_pending_high.is_empty());
    assert_eq!(
        scheduler.snapshot().preview_pending_low,
        vec![
            PreviewJobKey {
                path: PathBuf::from("b.zip"),
                size: 1,
                modified: None,
            },
            PreviewJobKey {
                path: PathBuf::from("c.zip"),
                size: 1,
                modified: None,
            },
        ]
    );
}

#[test]
fn high_priority_preview_promotes_over_low_priority_duplicate() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 4);
    let entry = Entry {
        path: PathBuf::from("archive.zip"),
        name: "archive.zip".to_string(),
        name_key: "archive.zip".to_string(),
        kind: EntryKind::File,
        size: 42,
        modified: None,
        readonly: false,
    };

    assert!(scheduler.submit_preview(PreviewRequest {
        token: 1,
        entry: entry.clone(),
        priority: PreviewPriority::Low,
    }));
    assert!(scheduler.submit_preview(PreviewRequest {
        token: 2,
        entry,
        priority: PreviewPriority::High,
    }));

    let snapshot = scheduler.snapshot();
    assert!(snapshot.preview_pending_low.is_empty());
    assert_eq!(
        snapshot.preview_pending_high,
        vec![PreviewJobKey {
            path: PathBuf::from("archive.zip"),
            size: 42,
            modified: None,
        }]
    );
    assert_eq!(scheduler.metrics_snapshot().preview_promotions, 1);
}

#[test]
fn low_priority_preview_does_not_displace_full_high_priority_queue() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 1);

    assert!(scheduler.submit_preview(PreviewRequest {
        token: 1,
        entry: Entry {
            path: PathBuf::from("a.zip"),
            name: "a.zip".to_string(),
            name_key: "a.zip".to_string(),
            kind: EntryKind::File,
            size: 1,
            modified: None,
            readonly: false,
        },
        priority: PreviewPriority::High,
    }));
    assert!(scheduler.submit_preview(PreviewRequest {
        token: 2,
        entry: Entry {
            path: PathBuf::from("b.zip"),
            name: "b.zip".to_string(),
            name_key: "b.zip".to_string(),
            kind: EntryKind::File,
            size: 1,
            modified: None,
            readonly: false,
        },
        priority: PreviewPriority::Low,
    }));

    let snapshot = scheduler.snapshot();
    assert_eq!(
        snapshot.preview_pending_high,
        vec![PreviewJobKey {
            path: PathBuf::from("a.zip"),
            size: 1,
            modified: None,
        }]
    );
    assert!(snapshot.preview_pending_low.is_empty());
    assert_eq!(
        scheduler.metrics_snapshot().preview_low_priority_evictions,
        0
    );
}

#[test]
fn low_priority_preview_eviction_updates_metrics() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 1);

    for name in ["a.zip", "b.zip"] {
        assert!(scheduler.submit_preview(PreviewRequest {
            token: 1,
            entry: Entry {
                path: PathBuf::from(name),
                name: name.to_string(),
                name_key: name.to_string(),
                kind: EntryKind::File,
                size: 1,
                modified: None,
                readonly: false,
            },
            priority: PreviewPriority::Low,
        }));
    }

    let metrics = scheduler.metrics_snapshot();
    assert_eq!(metrics.preview_jobs_submitted_low, 2);
    assert_eq!(metrics.preview_low_priority_evictions, 1);
}

#[test]
fn scheduler_reports_pending_work_when_jobs_are_queued() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    assert!(!scheduler.has_pending_work());

    assert!(scheduler.submit_search(SearchRequest {
        token: 1,
        cwd: PathBuf::from("/tmp/a"),
        scope: SearchScope::Files,
    }));
    assert!(scheduler.has_pending_work());
}

#[test]
fn retain_pdf_probe_pages_discards_stale_pending_requests() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    let path = PathBuf::from("manual.pdf");

    assert!(scheduler.submit_pdf_probe(PdfProbeRequest {
        path: path.clone(),
        size: 64,
        modified: None,
        page: 1,
    }));
    assert!(scheduler.submit_pdf_probe(PdfProbeRequest {
        path: path.clone(),
        size: 64,
        modified: None,
        page: 2,
    }));
    assert!(scheduler.submit_pdf_probe(PdfProbeRequest {
        path: PathBuf::from("other.pdf"),
        size: 64,
        modified: None,
        page: 1,
    }));

    scheduler.retain_pdf_probe_pages(&path, 64, None, &[2, 3]);

    assert_eq!(
        scheduler.snapshot().pdf_probe_pending,
        vec![PdfProbeJobKey {
            path,
            size: 64,
            modified: None,
            page: 2,
        }]
    );
}

#[test]
fn retain_pdf_render_variants_discards_stale_pending_requests() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    let path = PathBuf::from("manual.pdf");

    assert!(scheduler.submit_pdf_render(PdfRenderRequest {
        path: path.clone(),
        size: 64,
        modified: None,
        page: 2,
        width_px: 640,
        height_px: 896,
    }));
    assert!(scheduler.submit_pdf_render(PdfRenderRequest {
        path: path.clone(),
        size: 64,
        modified: None,
        page: 3,
        width_px: 704,
        height_px: 960,
    }));
    assert!(scheduler.submit_pdf_render(PdfRenderRequest {
        path: PathBuf::from("other.pdf"),
        size: 64,
        modified: None,
        page: 1,
        width_px: 640,
        height_px: 896,
    }));

    scheduler.retain_pdf_render_variants(&path, 64, None, &[(3, 704, 960)]);

    assert_eq!(
        scheduler.snapshot().pdf_render_pending,
        vec![PdfRenderJobKey {
            path,
            size: 64,
            modified: None,
            page: 3,
            width_px: 704,
            height_px: 960,
        }]
    );
}
