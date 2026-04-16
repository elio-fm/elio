use super::*;
use crate::preview::{PreviewRequestOptions, PreviewWorkClass, default_code_preview_line_limit};
use std::sync::Arc;

fn image_prepare_request(name: &str) -> ImagePrepareRequest {
    ImagePrepareRequest {
        path: PathBuf::from(name),
        size: 1,
        modified: None,
        target_width_px: 640,
        target_height_px: 480,
        ffmpeg_available: true,
        resvg_available: true,
        magick_available: true,
        force_render_to_cache: false,
        prepare_inline_payload: false,
        sixel_prepare: None,
    }
}

fn preview_request(entry: Entry, token: u64, priority: PreviewPriority) -> PreviewRequest {
    PreviewRequest {
        token,
        entry,
        variant: PreviewRequestOptions::Default,
        code_line_limit: default_code_preview_line_limit(),
        code_render_limit: default_code_preview_line_limit(),
        priority,
        work_class: PreviewWorkClass::Light,
        ffprobe_available: false,
        ffmpeg_available: false,
    }
}

fn preview_job_key(path: &str, size: u64) -> PreviewJobKey {
    PreviewJobKey {
        path: PathBuf::from(path),
        size,
        modified: None,
        variant: PreviewRequestOptions::Default,
        code_line_limit: default_code_preview_line_limit(),
        code_render_limit: default_code_preview_line_limit(),
    }
}

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

    assert!(scheduler.submit_preview(preview_request(entry.clone(), 1, PreviewPriority::Low,)));
    assert!(scheduler.submit_preview(preview_request(entry, 2, PreviewPriority::Low,)));
    let snapshot = scheduler.snapshot();
    assert!(snapshot.preview_pending_high.is_empty());
    assert_eq!(
        snapshot.preview_pending_low,
        vec![preview_job_key("archive.zip", 42)]
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
        show_hidden: false,
        fingerprint: crate::fs::DirectoryFingerprint::default(),
    }));
    assert!(scheduler.submit_search(SearchRequest {
        token: 2,
        cwd: PathBuf::from("/tmp/b"),
        scope: SearchScope::Files,
        show_hidden: false,
        fingerprint: crate::fs::DirectoryFingerprint::default(),
    }));
    assert_eq!(
        scheduler.snapshot().search_pending,
        Some(SearchJobKey {
            cwd: PathBuf::from("/tmp/b"),
            scope: SearchScope::Files,
            show_hidden: false,
            fingerprint: crate::fs::DirectoryFingerprint::default(),
        })
    );
}

#[test]
fn preview_pool_discards_oldest_queued_request_when_full() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);

    for name in ["a.zip", "b.zip", "c.zip"] {
        assert!(scheduler.submit_preview(preview_request(
            Entry {
                path: PathBuf::from(name),
                name: name.to_string(),
                name_key: name.to_string(),
                kind: EntryKind::File,
                size: 1,
                modified: None,
                readonly: false,
            },
            1,
            PreviewPriority::Low,
        )));
    }

    assert!(scheduler.snapshot().preview_pending_high.is_empty());
    assert_eq!(
        scheduler.snapshot().preview_pending_low,
        vec![preview_job_key("b.zip", 1), preview_job_key("c.zip", 1)]
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

    assert!(scheduler.submit_preview(preview_request(entry.clone(), 1, PreviewPriority::Low,)));
    assert!(scheduler.submit_preview(preview_request(entry, 2, PreviewPriority::High,)));

    let snapshot = scheduler.snapshot();
    assert!(snapshot.preview_pending_low.is_empty());
    assert_eq!(
        snapshot.preview_pending_high,
        vec![preview_job_key("archive.zip", 42)]
    );
    assert_eq!(scheduler.metrics_snapshot().preview_promotions, 1);
}

#[test]
fn low_priority_preview_does_not_displace_full_high_priority_queue() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 1);

    assert!(scheduler.submit_preview(preview_request(
        Entry {
            path: PathBuf::from("a.zip"),
            name: "a.zip".to_string(),
            name_key: "a.zip".to_string(),
            kind: EntryKind::File,
            size: 1,
            modified: None,
            readonly: false,
        },
        1,
        PreviewPriority::High,
    )));
    assert!(scheduler.submit_preview(preview_request(
        Entry {
            path: PathBuf::from("b.zip"),
            name: "b.zip".to_string(),
            name_key: "b.zip".to_string(),
            kind: EntryKind::File,
            size: 1,
            modified: None,
            readonly: false,
        },
        2,
        PreviewPriority::Low,
    )));

    let snapshot = scheduler.snapshot();
    assert_eq!(
        snapshot.preview_pending_high,
        vec![preview_job_key("a.zip", 1)]
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
        assert!(scheduler.submit_preview(preview_request(
            Entry {
                path: PathBuf::from(name),
                name: name.to_string(),
                name_key: name.to_string(),
                kind: EntryKind::File,
                size: 1,
                modified: None,
                readonly: false,
            },
            1,
            PreviewPriority::Low,
        )));
    }

    let metrics = scheduler.metrics_snapshot();
    assert_eq!(metrics.preview_jobs_submitted_low, 2);
    assert_eq!(metrics.preview_low_priority_evictions, 1);
}

#[test]
fn low_priority_heavy_preview_allows_two_concurrent_heavy_jobs() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 4);
    let first = Entry {
        path: PathBuf::from("first.zip"),
        name: "first.zip".to_string(),
        name_key: "first.zip".to_string(),
        kind: EntryKind::File,
        size: 1,
        modified: None,
        readonly: false,
    };
    let second = Entry {
        path: PathBuf::from("second.zip"),
        name: "second.zip".to_string(),
        name_key: "second.zip".to_string(),
        kind: EntryKind::File,
        size: 1,
        modified: None,
        readonly: false,
    };

    assert!(scheduler.submit_preview(PreviewRequest {
        token: 1,
        entry: first.clone(),
        variant: PreviewRequestOptions::Default,
        code_line_limit: default_code_preview_line_limit(),
        code_render_limit: default_code_preview_line_limit(),
        priority: PreviewPriority::Low,
        work_class: PreviewWorkClass::Heavy,
        ffprobe_available: false,
        ffmpeg_available: false,
    }));
    assert!(scheduler.submit_preview(PreviewRequest {
        token: 2,
        entry: second.clone(),
        variant: PreviewRequestOptions::Default,
        code_line_limit: default_code_preview_line_limit(),
        code_render_limit: default_code_preview_line_limit(),
        priority: PreviewPriority::Low,
        work_class: PreviewWorkClass::Heavy,
        ffprobe_available: false,
        ffmpeg_available: false,
    }));

    let started = scheduler
        .pop_next_pending_preview_for_tests()
        .expect("first heavy preview should start");
    assert_eq!(started.entry.path, first.path);
    let second_started = scheduler
        .pop_next_pending_preview_for_tests()
        .expect("second heavy preview should also start");
    assert_eq!(second_started.entry.path, second.path);
    assert!(scheduler.snapshot().preview_pending_low.is_empty());
}

#[test]
fn low_priority_light_preview_can_start_while_heavy_preview_is_active() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 4);
    let heavy = Entry {
        path: PathBuf::from("archive.zip"),
        name: "archive.zip".to_string(),
        name_key: "archive.zip".to_string(),
        kind: EntryKind::File,
        size: 1,
        modified: None,
        readonly: false,
    };
    let light = Entry {
        path: PathBuf::from("notes.txt"),
        name: "notes.txt".to_string(),
        name_key: "notes.txt".to_string(),
        kind: EntryKind::File,
        size: 1,
        modified: None,
        readonly: false,
    };

    assert!(scheduler.submit_preview(PreviewRequest {
        token: 1,
        entry: heavy.clone(),
        variant: PreviewRequestOptions::Default,
        code_line_limit: default_code_preview_line_limit(),
        code_render_limit: default_code_preview_line_limit(),
        priority: PreviewPriority::Low,
        work_class: PreviewWorkClass::Heavy,
        ffprobe_available: false,
        ffmpeg_available: false,
    }));
    assert!(scheduler.submit_preview(PreviewRequest {
        token: 2,
        entry: light.clone(),
        variant: PreviewRequestOptions::Default,
        code_line_limit: default_code_preview_line_limit(),
        code_render_limit: default_code_preview_line_limit(),
        priority: PreviewPriority::Low,
        work_class: PreviewWorkClass::Light,
        ffprobe_available: false,
        ffmpeg_available: false,
    }));

    let started_heavy = scheduler
        .pop_next_pending_preview_for_tests()
        .expect("heavy preview should start");
    assert_eq!(started_heavy.entry.path, heavy.path);
    let started_light = scheduler
        .pop_next_pending_preview_for_tests()
        .expect("light preview should still start");
    assert_eq!(started_light.entry.path, light.path);
}

#[test]
fn high_priority_preview_cancels_active_stale_preview_work() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 4);
    let active = Entry {
        path: PathBuf::from("active.rs"),
        name: "active.rs".to_string(),
        name_key: "active.rs".to_string(),
        kind: EntryKind::File,
        size: 1,
        modified: None,
        readonly: false,
    };
    let current = Entry {
        path: PathBuf::from("current.rs"),
        name: "current.rs".to_string(),
        name_key: "current.rs".to_string(),
        kind: EntryKind::File,
        size: 1,
        modified: None,
        readonly: false,
    };

    assert!(scheduler.submit_preview(preview_request(active.clone(), 1, PreviewPriority::Low,)));
    let started = scheduler
        .pop_next_pending_preview_for_tests()
        .expect("stale preview should start");
    assert_eq!(started.entry.path, active.path);

    assert!(scheduler.submit_preview(preview_request(current.clone(), 2, PreviewPriority::High,)));

    assert_eq!(
        scheduler.canceled_active_preview_keys_for_tests(),
        vec![preview_job_key("active.rs", 1)]
    );
    assert_eq!(
        scheduler.snapshot().preview_pending_high,
        vec![preview_job_key("current.rs", 1)]
    );
}

#[test]
fn scheduler_reports_pending_work_when_jobs_are_queued() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    assert!(!scheduler.has_pending_work());

    assert!(scheduler.submit_search(SearchRequest {
        token: 1,
        cwd: PathBuf::from("/tmp/a"),
        scope: SearchScope::Files,
        show_hidden: false,
        fingerprint: crate::fs::DirectoryFingerprint::default(),
    }));
    assert!(scheduler.has_pending_work());
}

#[test]
fn scheduler_reports_pending_work_for_buffered_results() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    scheduler.defer_result(JobResult::Search(SearchBuild {
        token: 7,
        cwd: PathBuf::from("/tmp/search"),
        scope: SearchScope::Files,
        show_hidden: false,
        fingerprint: crate::fs::DirectoryFingerprint::default(),
        result: Ok(Arc::new(Vec::new())),
    }));

    assert!(scheduler.has_pending_work());
    match scheduler.try_recv() {
        Ok(JobResult::Search(build)) => {
            assert_eq!(build.token, 7);
            assert_eq!(build.cwd, PathBuf::from("/tmp/search"));
        }
        other => panic!("expected buffered search result, got {other:?}"),
    }
    assert!(!scheduler.has_pending_work());
}

#[test]
fn current_image_prepare_priority_outranks_nearby_requests() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);

    assert!(scheduler.submit_nearby_image_prepare(image_prepare_request("nearby.png")));
    assert!(scheduler.submit_image_prepare(image_prepare_request("current.png")));

    assert_eq!(
        scheduler.snapshot().image_prepare_pending,
        vec![
            ImagePrepareJobKey {
                path: PathBuf::from("current.png"),
                size: 1,
                modified: None,
                target_width_px: 640,
                target_height_px: 480,
                force_render_to_cache: false,
                prepare_inline_payload: false,
                sixel_prepare: None,
            },
            ImagePrepareJobKey {
                path: PathBuf::from("nearby.png"),
                size: 1,
                modified: None,
                target_width_px: 640,
                target_height_px: 480,
                force_render_to_cache: false,
                prepare_inline_payload: false,
                sixel_prepare: None,
            },
        ]
    );
}

#[test]
fn retain_image_prepares_discards_stale_nearby_requests() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    let current = image_prepare_request("current.png");
    let nearby_keep = image_prepare_request("keep.png");
    let nearby_drop = image_prepare_request("drop.png");

    assert!(scheduler.submit_image_prepare(current.clone()));
    assert!(scheduler.submit_nearby_image_prepare(nearby_keep.clone()));
    assert!(scheduler.submit_nearby_image_prepare(nearby_drop));

    scheduler.retain_image_prepares(Some(&current), std::slice::from_ref(&nearby_keep));

    assert_eq!(
        scheduler.snapshot().image_prepare_pending,
        vec![
            ImagePrepareJobKey {
                path: PathBuf::from("current.png"),
                size: 1,
                modified: None,
                target_width_px: 640,
                target_height_px: 480,
                force_render_to_cache: false,
                prepare_inline_payload: false,
                sixel_prepare: None,
            },
            ImagePrepareJobKey {
                path: PathBuf::from("keep.png"),
                size: 1,
                modified: None,
                target_width_px: 640,
                target_height_px: 480,
                force_render_to_cache: false,
                prepare_inline_payload: false,
                sixel_prepare: None,
            },
        ]
    );
}

#[test]
fn retain_image_prepares_promotes_nearby_job_to_current_when_it_becomes_current() {
    // Regression test: if a job was submitted at Nearby priority (e.g. as a
    // prefetch for an adjacent comic entry) and the user then navigates to that
    // entry, retain_image_prepares is called with the job as `current` and an
    // empty nearby list.  The old code discarded the job from pending_nearby
    // without adding it to pending_current, leaving app.pending_prepares stuck
    // with the key and prevent re-submission — so the image never appeared.
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    let now_current = image_prepare_request("page.jpg");
    let other_nearby = image_prepare_request("other.png");

    // Simulate prefetch: page.jpg was submitted as a nearby job
    assert!(scheduler.submit_nearby_image_prepare(now_current.clone()));
    assert!(scheduler.submit_nearby_image_prepare(other_nearby));

    // User navigates to the entry whose image was prefetched
    scheduler.retain_image_prepares(Some(&now_current), &[]);

    let pending = scheduler.snapshot().image_prepare_pending;
    assert_eq!(
        pending.len(),
        1,
        "promoted job should be the only pending job"
    );
    assert_eq!(
        pending[0],
        ImagePrepareJobKey {
            path: PathBuf::from("page.jpg"),
            size: 1,
            modified: None,
            target_width_px: 640,
            target_height_px: 480,
            force_render_to_cache: false,
            prepare_inline_payload: false,
            sixel_prepare: None,
        },
        "nearby job should have been promoted to current"
    );
}

#[test]
fn retain_pdf_probe_pages_discards_stale_pending_requests() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    let path = PathBuf::from("manual.pdf");

    assert!(scheduler.submit_pdf_probe(
        PdfProbeRequest {
            path: path.clone(),
            size: 64,
            modified: None,
            page: 1,
        },
        PdfJobPriority::Current
    ));
    assert!(scheduler.submit_pdf_probe(
        PdfProbeRequest {
            path: path.clone(),
            size: 64,
            modified: None,
            page: 2,
        },
        PdfJobPriority::Prefetch
    ));
    assert!(scheduler.submit_pdf_probe(
        PdfProbeRequest {
            path: PathBuf::from("other.pdf"),
            size: 64,
            modified: None,
            page: 1,
        },
        PdfJobPriority::Prefetch
    ));

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

    assert!(scheduler.submit_pdf_render(
        PdfRenderRequest {
            path: path.clone(),
            size: 64,
            modified: None,
            page: 2,
            width_px: 640,
            height_px: 896,
            sixel_prepare: None,
        },
        PdfJobPriority::Current
    ));
    assert!(scheduler.submit_pdf_render(
        PdfRenderRequest {
            path: path.clone(),
            size: 64,
            modified: None,
            page: 3,
            width_px: 704,
            height_px: 960,
            sixel_prepare: None,
        },
        PdfJobPriority::Prefetch
    ));
    assert!(scheduler.submit_pdf_render(
        PdfRenderRequest {
            path: PathBuf::from("other.pdf"),
            size: 64,
            modified: None,
            page: 1,
            width_px: 640,
            height_px: 896,
            sixel_prepare: None,
        },
        PdfJobPriority::Prefetch
    ));

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

#[test]
fn current_pdf_probe_priority_outranks_prefetch_requests() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    let path = PathBuf::from("manual.pdf");

    assert!(scheduler.submit_pdf_probe(
        PdfProbeRequest {
            path: path.clone(),
            size: 64,
            modified: None,
            page: 2,
        },
        PdfJobPriority::Prefetch,
    ));
    assert!(scheduler.submit_pdf_probe(
        PdfProbeRequest {
            path: path.clone(),
            size: 64,
            modified: None,
            page: 1,
        },
        PdfJobPriority::Current,
    ));

    assert_eq!(
        scheduler.snapshot().pdf_probe_pending,
        vec![
            PdfProbeJobKey {
                path: path.clone(),
                size: 64,
                modified: None,
                page: 1,
            },
            PdfProbeJobKey {
                path,
                size: 64,
                modified: None,
                page: 2,
            },
        ]
    );
}

#[test]
fn current_pdf_render_priority_outranks_prefetch_requests() {
    let scheduler = JobScheduler::new_for_tests(0, 0, 2);
    let path = PathBuf::from("manual.pdf");

    assert!(scheduler.submit_pdf_render(
        PdfRenderRequest {
            path: path.clone(),
            size: 64,
            modified: None,
            page: 2,
            width_px: 640,
            height_px: 896,
            sixel_prepare: None,
        },
        PdfJobPriority::Prefetch,
    ));
    assert!(scheduler.submit_pdf_render(
        PdfRenderRequest {
            path: path.clone(),
            size: 64,
            modified: None,
            page: 1,
            width_px: 640,
            height_px: 896,
            sixel_prepare: None,
        },
        PdfJobPriority::Current,
    ));

    assert_eq!(
        scheduler.snapshot().pdf_render_pending,
        vec![
            PdfRenderJobKey {
                path: path.clone(),
                size: 64,
                modified: None,
                page: 1,
                width_px: 640,
                height_px: 896,
            },
            PdfRenderJobKey {
                path,
                size: 64,
                modified: None,
                page: 2,
                width_px: 640,
                height_px: 896,
            },
        ]
    );
}
