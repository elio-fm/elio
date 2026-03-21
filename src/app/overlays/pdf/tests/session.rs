use super::super::*;
use super::helpers::*;

#[test]
fn pdf_preview_page_navigation_clamps_to_document_bounds() {
    let mut app = App::new_at(std::env::temp_dir()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;
    app.pdf_preview.session = Some(PdfSession {
        path: PathBuf::from("demo.pdf"),
        size: 1,
        modified: None,
        current_page: 2,
        total_pages: Some(3),
    });

    assert!(app.step_pdf_page(1));
    assert_eq!(
        app.pdf_preview
            .session
            .as_ref()
            .map(|session| session.current_page),
        Some(3)
    );
    assert!(!app.step_pdf_page(1));
    assert_eq!(
        app.pdf_preview
            .session
            .as_ref()
            .map(|session| session.current_page),
        Some(3)
    );
    assert!(app.step_pdf_page(-2));
    assert_eq!(
        app.pdf_preview
            .session
            .as_ref()
            .map(|session| session.current_page),
        Some(1)
    );
    assert!(app.status.is_empty());
}

#[test]
fn present_pdf_overlay_waits_for_selection_activation_before_queueing_probe() {
    let (mut app, root) = build_pdf_overlay_test_app("activation-delay");
    app.pdf_preview.activation_ready_at = Some(Instant::now() + Duration::from_secs(5));

    app.present_preview_overlay()
        .expect("presenting a delayed PDF overlay should not fail");

    assert!(app.pdf_preview.pending_page_probes.is_empty());
    assert!(!app.scheduler.has_pending_work());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn present_pdf_overlay_queues_current_probe_only_once() {
    let (mut app, root) = build_pdf_overlay_test_app("probe-queue");
    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let key = PdfPageKey::from_request(&request);

    app.present_preview_overlay()
        .expect("presenting a PDF overlay should not fail");
    app.present_preview_overlay()
        .expect("retrying a PDF overlay should not fail");

    assert_eq!(app.pdf_preview.pending_page_probes.len(), 1);
    assert!(app.pdf_preview.pending_page_probes.contains(&key));
    assert!(app.scheduler.has_pending_work());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn process_pdf_preview_timers_releases_selection_activation_once() {
    let (mut app, root) = build_pdf_overlay_test_app("activation-timer");
    app.pdf_preview.activation_ready_at = Some(Instant::now() - Duration::from_millis(1));

    assert!(app.process_pdf_preview_timers());
    assert!(!app.process_pdf_preview_timers());
    assert!(app.pdf_preview.activation_ready_at.is_none());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn pdf_prefetch_pages_prefers_backward_order_after_reverse_navigation() {
    let (mut app, root) = build_pdf_overlay_test_app("prefetch-backward-order");
    let session = app
        .pdf_preview
        .session
        .as_mut()
        .expect("PDF session should exist");
    session.current_page = 4;
    session.total_pages = Some(6);
    app.pdf_preview.last_navigation_direction = -1;

    assert_eq!(app.pdf_prefetch_probe_pages(), vec![3, 2, 5]);
    assert_eq!(app.pdf_prefetch_render_pages(), vec![3, 2, 5]);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sync_pdf_preview_selection_reuses_cached_total_page_count() {
    let root = temp_root("cached-page-count");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    let entry = Entry {
        path: root.join("cached.pdf"),
        name: "cached.pdf".to_string(),
        name_key: "cached.pdf".to_string(),
        kind: EntryKind::File,
        size: 64,
        modified: None,
        readonly: false,
    };
    app.entries = vec![entry.clone()];
    app.selected = 0;
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;
    app.pdf_preview
        .document_page_counts
        .insert(PdfDocumentKey::from_entry(&entry), 12);

    app.sync_pdf_preview_selection();

    assert_eq!(
        app.pdf_preview
            .session
            .as_ref()
            .and_then(|session| session.total_pages),
        Some(12)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sync_pdf_preview_selection_prefetches_forward_probe_window_when_page_count_is_known() {
    let root = temp_root("selection-probe-prefetch");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    let entry = Entry {
        path: root.join("queued.pdf"),
        name: "queued.pdf".to_string(),
        name_key: "queued.pdf".to_string(),
        kind: EntryKind::File,
        size: 64,
        modified: None,
        readonly: false,
    };
    app.entries = vec![entry.clone()];
    app.selected = 0;
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;
    app.pdf_preview
        .document_page_counts
        .insert(PdfDocumentKey::from_entry(&entry), 12);

    app.sync_pdf_preview_selection();

    assert_eq!(
        app.pdf_preview.pending_page_probes,
        [PDF_PAGE_MIN, 2, 3]
            .into_iter()
            .map(|page| PdfPageKey {
                path: entry.path.clone(),
                size: entry.size,
                modified: entry.modified,
                page,
            })
            .collect()
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sync_pdf_preview_selection_queues_initial_probe_for_current_page() {
    let root = temp_root("selection-probe");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    let entry = Entry {
        path: root.join("queued.pdf"),
        name: "queued.pdf".to_string(),
        name_key: "queued.pdf".to_string(),
        kind: EntryKind::File,
        size: 64,
        modified: None,
        readonly: false,
    };
    app.entries = vec![entry.clone()];
    app.selected = 0;
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;

    app.sync_pdf_preview_selection();

    assert!(app.scheduler.has_pending_work());
    assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
        path: entry.path,
        size: entry.size,
        modified: entry.modified,
        page: PDF_PAGE_MIN,
    }));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_uses_image_overlay_only_for_current_render_target() {
    let (mut app, root) = build_pdf_overlay_test_app("overlay-match");
    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let key = PdfPageKey::from_request(&request);
    app.pdf_preview.page_dimensions.insert(
        key,
        PdfPageDimensions {
            width_pts: 595.0,
            height_pts: 842.0,
        },
    );
    let placement = app
        .overlay_placement_for_request(&request)
        .expect("overlay placement should be available");
    let render_key = PdfRenderKey::from_request(&request, placement);
    app.pdf_preview.rendered_page_dimensions.insert(
        render_key,
        RenderedImageDimensions {
            width_px: placement.render_width_px,
            height_px: placement.render_height_px,
        },
    );
    app.pdf_preview.displayed = Some(DisplayedPdfPreview::from_request(&request, placement));

    assert!(app.preview_uses_image_overlay());

    app.pdf_preview
        .session
        .as_mut()
        .expect("PDF session should exist")
        .current_page = 2;

    assert!(!app.preview_uses_image_overlay());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn leaving_static_image_selection_clears_overlay_without_recursion() {
    let root = temp_root("static-image-transition");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let fade_path = root.join("fade.png");
    let html_path = root.join("index.html");
    write_test_raster_image(&fade_path, ImageFormat::Png, 8, 8);
    fs::write(&html_path, "<html><body>demo</body></html>\n")
        .expect("failed to write html placeholder");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.frame_state.metrics.cols = 1;
    app.frame_state.metrics.rows_visible = 6;
    app.refresh_preview();

    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(fade_path.as_path())
    );

    app.last_selection_change_at = Instant::now() - Duration::from_secs(1);
    app.sync_image_preview_selection_activation();
    app.present_preview_overlay()
        .expect("presenting a static image overlay should not fail");
    assert!(app.static_image_overlay_displayed());
    assert!(app.preview_uses_image_overlay());

    app.select_index(1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.path.as_path()),
        Some(html_path.as_path())
    );

    app.present_preview_overlay()
        .expect("clearing a stale static image overlay should not fail");
    assert!(!app.static_image_overlay_displayed());
    assert!(!app.preview_uses_image_overlay());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn step_pdf_page_queues_render_immediately_when_dimensions_are_cached() {
    let (mut app, root) = build_pdf_overlay_test_app("page-step-render");
    let next_request = PdfOverlayRequest {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 2,
        area: app
            .frame_state
            .preview_content_area
            .expect("preview content area should be set"),
    };
    app.pdf_preview.page_dimensions.insert(
        PdfPageKey::from_request(&next_request),
        PdfPageDimensions {
            width_pts: 612.0,
            height_pts: 792.0,
        },
    );
    app.pdf_preview
        .session
        .as_mut()
        .expect("PDF session should exist")
        .total_pages = Some(3);

    assert!(app.step_pdf_page(1));

    let active_request = app
        .active_pdf_overlay_request()
        .expect("updated PDF overlay request should be available");
    let placement = app
        .overlay_placement_for_request(&active_request)
        .expect("overlay placement should be available");
    let render_key = PdfRenderKey::from_request(&active_request, placement);
    assert!(app.pdf_preview.pending_renders.contains(&render_key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn handle_pdf_overlay_resize_prunes_stale_render_variant_and_queues_new_current_render() {
    let (mut app, root) = build_pdf_overlay_test_app("resize-render-prune");
    app.terminal_images.window = Some(TerminalWindowSize {
        cells_width: 100,
        cells_height: 50,
        pixels_width: 1000,
        pixels_height: 1000,
    });
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 16,
        height: 8,
    });

    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let page_key = PdfPageKey::from_request(&request);
    app.pdf_preview.page_dimensions.insert(
        page_key,
        PdfPageDimensions {
            width_pts: 612.0,
            height_pts: 792.0,
        },
    );
    let old_placement = app
        .overlay_placement_for_request(&request)
        .expect("original placement should be available");
    let old_render_key = PdfRenderKey::from_request(&request, old_placement);
    app.pdf_preview
        .pending_renders
        .insert(old_render_key.clone());

    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 64,
        height: 32,
    });
    app.handle_pdf_overlay_resize();

    let resized_request = app
        .active_pdf_overlay_request()
        .expect("resized PDF overlay request should be available");
    let resized_placement = app
        .overlay_placement_for_request(&resized_request)
        .expect("resized placement should be available");
    let resized_render_key = PdfRenderKey::from_request(&resized_request, resized_placement);

    assert_ne!(old_render_key, resized_render_key);
    assert!(app.pdf_preview.activation_ready_at.is_none());
    assert!(!app.pdf_preview.pending_renders.contains(&old_render_key));
    assert!(
        app.pdf_preview
            .pending_renders
            .contains(&resized_render_key)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn step_pdf_page_prunes_stale_probe_window_and_prefetches_forward_pages() {
    let (mut app, root) = build_pdf_overlay_test_app("page-step-prune");
    let session = app
        .pdf_preview
        .session
        .as_mut()
        .expect("PDF session should exist");
    session.current_page = 2;
    session.total_pages = Some(5);

    for page in [1, 2, 3] {
        app.pdf_preview.pending_page_probes.insert(PdfPageKey {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page,
        });
    }

    assert!(app.step_pdf_page(1));

    assert!(!app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 1,
    }));
    assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 2,
    }));
    assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 3,
    }));
    assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 4,
    }));
    assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 5,
    }));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn pdf_preview_placeholder_message_stays_silent_while_loading() {
    let (mut app, root) = build_pdf_overlay_test_app("placeholder");

    assert_eq!(app.preview_overlay_placeholder_message(), None);

    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let page_key = PdfPageKey::from_request(&request);
    app.pdf_preview.page_dimensions.insert(
        page_key,
        PdfPageDimensions {
            width_pts: 595.0,
            height_pts: 842.0,
        },
    );
    let placement = app
        .overlay_placement_for_request(&request)
        .expect("overlay placement should be available");
    app.pdf_preview
        .pending_renders
        .insert(PdfRenderKey::from_request(&request, placement));

    assert_eq!(app.preview_overlay_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn preview_prefers_pdf_surface_falls_back_after_overlay_failure() {
    let (mut app, root) = build_pdf_overlay_test_app("fallback");
    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let page_key = PdfPageKey::from_request(&request);
    app.pdf_preview.failed_page_probes.insert(page_key);

    assert!(!app.preview_prefers_pdf_surface());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn refresh_preview_uses_blank_pdf_surface_preview_when_active() {
    let (mut app, root) = build_selected_pdf_app("skip-pdf-metadata");
    let before = app.scheduler_metrics();

    app.refresh_preview();

    let after = app.scheduler_metrics();
    assert_eq!(
        after.preview_jobs_submitted_high,
        before.preview_jobs_submitted_high
    );
    assert_eq!(app.preview_state.content.kind, PreviewKind::Document);
    assert_eq!(
        app.preview_state.content.detail.as_deref(),
        Some("PDF document")
    );
    assert!(app.preview_state.content.lines.is_empty());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn refresh_preview_restores_pdf_metadata_fallback_after_probe_failure() {
    let (mut app, root) = build_selected_pdf_app("pdf-fallback-preview");
    app.pdf_preview.activation_ready_at = Some(Instant::now());
    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    app.pdf_preview
        .failed_page_probes
        .insert(PdfPageKey::from_request(&request));

    app.refresh_preview();

    assert!(!app.preview_prefers_pdf_surface());
    assert!(!app.preview_state.content.lines.is_empty());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sync_pdf_preview_selection_clears_stale_pdf_page_status() {
    let mut app = App::new_at(std::env::temp_dir()).expect("app should initialize");
    app.status = "PDF page 3/10".to_string();
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;

    app.sync_pdf_preview_selection();

    assert!(app.status.is_empty());
}
