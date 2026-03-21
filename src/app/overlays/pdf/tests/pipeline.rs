use super::super::*;
use super::helpers::*;

#[test]
fn apply_pdf_probe_build_updates_current_session_and_cached_dimensions() {
    let (mut app, root) = build_pdf_overlay_test_app("probe-apply");
    let session = app
        .pdf_preview
        .session
        .as_mut()
        .expect("PDF session should exist");
    session.current_page = 5;
    let key = PdfPageKey {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 5,
    };
    app.pdf_preview.pending_page_probes.insert(key.clone());

    let dirty = app.apply_pdf_probe_build(jobs::PdfProbeBuild {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 5,
        result: Ok(PdfProbeResult {
            total_pages: Some(3),
            width_pts: Some(300.0),
            height_pts: Some(144.0),
        }),
    });

    assert!(dirty);
    assert_eq!(
        app.pdf_preview
            .session
            .as_ref()
            .map(|session| session.current_page),
        Some(3)
    );
    assert_eq!(
        app.pdf_preview
            .session
            .as_ref()
            .and_then(|session| session.total_pages),
        Some(3)
    );
    assert_eq!(
        app.pdf_preview.page_dimensions.get(&key),
        Some(&PdfPageDimensions {
            width_pts: 300.0,
            height_pts: 144.0,
        })
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn apply_pdf_probe_build_queues_render_for_current_page() {
    let (mut app, root) = build_pdf_overlay_test_app("probe-render-queue");
    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let page_key = PdfPageKey::from_request(&request);
    app.pdf_preview.pending_page_probes.insert(page_key);

    let dirty = app.apply_pdf_probe_build(jobs::PdfProbeBuild {
        path: request.path.clone(),
        size: request.size,
        modified: request.modified,
        page: request.page,
        result: Ok(PdfProbeResult {
            total_pages: Some(8),
            width_pts: Some(595.0),
            height_pts: Some(842.0),
        }),
    });

    let placement = app
        .overlay_placement_for_request(&request)
        .expect("overlay placement should be available after probe");
    let render_key = PdfRenderKey::from_request(&request, placement);

    assert!(dirty);
    assert!(app.pdf_preview.pending_renders.contains(&render_key));
    assert!(app.scheduler.has_pending_work());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn apply_pdf_probe_build_queues_render_even_before_selection_activation_is_ready() {
    let (mut app, root) = build_pdf_overlay_test_app("probe-render-before-activation");
    app.pdf_preview.activation_ready_at = Some(Instant::now() + Duration::from_secs(5));
    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let page_key = PdfPageKey::from_request(&request);
    app.pdf_preview.pending_page_probes.insert(page_key);

    let dirty = app.apply_pdf_probe_build(jobs::PdfProbeBuild {
        path: request.path.clone(),
        size: request.size,
        modified: request.modified,
        page: request.page,
        result: Ok(PdfProbeResult {
            total_pages: Some(8),
            width_pts: Some(595.0),
            height_pts: Some(842.0),
        }),
    });

    let placement = app
        .overlay_placement_for_request(&request)
        .expect("overlay placement should be available after probe");
    let render_key = PdfRenderKey::from_request(&request, placement);

    assert!(dirty);
    assert!(app.pdf_preview.pending_renders.contains(&render_key));
    assert!(app.scheduler.has_pending_work());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn apply_pdf_probe_build_prefetches_adjacent_page_probes_once_total_is_known() {
    let (mut app, root) = build_pdf_overlay_test_app("probe-prefetch-pages");
    let session = app
        .pdf_preview
        .session
        .as_mut()
        .expect("PDF session should exist");
    session.current_page = 2;

    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let page_key = PdfPageKey::from_request(&request);
    app.pdf_preview.pending_page_probes.insert(page_key);

    let dirty = app.apply_pdf_probe_build(jobs::PdfProbeBuild {
        path: request.path.clone(),
        size: request.size,
        modified: request.modified,
        page: request.page,
        result: Ok(PdfProbeResult {
            total_pages: Some(4),
            width_pts: Some(595.0),
            height_pts: Some(842.0),
        }),
    });

    assert!(dirty);
    assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
        path: request.path.clone(),
        size: request.size,
        modified: request.modified,
        page: 1,
    }));
    assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
        path: request.path.clone(),
        size: request.size,
        modified: request.modified,
        page: 3,
    }));
    assert!(app.pdf_preview.pending_page_probes.contains(&PdfPageKey {
        path: request.path,
        size: request.size,
        modified: request.modified,
        page: 4,
    }));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn apply_pdf_render_build_prefetches_next_page_when_current_page_is_ready() {
    let (mut app, root) = build_pdf_overlay_test_app("render-prefetch-next");
    let session = app
        .pdf_preview
        .session
        .as_mut()
        .expect("PDF session should exist");
    session.current_page = 2;
    session.total_pages = Some(4);

    for page in [2, 3] {
        let request = PdfOverlayRequest {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page,
            area: app
                .frame_state
                .preview_content_area
                .expect("preview content area should be set"),
        };
        app.pdf_preview.page_dimensions.insert(
            PdfPageKey::from_request(&request),
            PdfPageDimensions {
                width_pts: 612.0,
                height_pts: 792.0,
            },
        );
    }

    let current_request = app
        .pdf_overlay_request_for_page(2)
        .expect("current PDF overlay request should be available");
    let current_key = app
        .pdf_render_key_for_page(2)
        .expect("current PDF render key should be available");
    app.pdf_preview.pending_renders.insert(current_key.clone());

    let rendered_path = root.join("current-page.png");
    fs::write(&rendered_path, b"png").expect("failed to write rendered page placeholder");

    let dirty = app.apply_pdf_render_build(jobs::PdfRenderBuild {
        path: current_request.path.clone(),
        size: current_request.size,
        modified: current_request.modified,
        page: current_request.page,
        width_px: current_key.width_px,
        height_px: current_key.height_px,
        result: Ok(Some(rendered_path)),
    });

    let next_key = app
        .pdf_render_key_for_page(3)
        .expect("next page render key should be available");

    assert!(dirty);
    assert!(app.pdf_preview.pending_renders.contains(&next_key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
