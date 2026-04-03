use super::super::*;
use super::helpers::*;

#[test]
fn apply_pdf_probe_build_updates_current_session_and_cached_dimensions() {
    let (mut app, root) = build_pdf_overlay_test_app("probe-apply");
    let session = app
        .preview
        .pdf
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
    app.preview.pdf.pending_page_probes.insert(key.clone());

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
        app.preview
            .pdf
            .session
            .as_ref()
            .map(|session| session.current_page),
        Some(3)
    );
    assert_eq!(
        app.preview
            .pdf
            .session
            .as_ref()
            .and_then(|session| session.total_pages),
        Some(3)
    );
    assert_eq!(
        app.preview.pdf.page_dimensions.get(&key),
        Some(&PdfPageDimensions {
            width_pts: 300.0,
            height_pts: 144.0,
        })
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cached_pdf_render_path_drops_missing_files_from_cache() {
    let (mut app, root) = build_pdf_overlay_test_app("missing-render-cache");
    let key = PdfRenderKey {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 1,
        width_px: 704,
        height_px: 960,
    };
    let missing_path = root.join("missing-render.png");
    app.preview
        .pdf
        .rendered_pages
        .insert(key.clone(), missing_path.clone());
    app.preview.pdf.rendered_page_dimensions.insert(
        key.clone(),
        RenderedImageDimensions {
            width_px: 704,
            height_px: 960,
        },
    );
    app.preview.pdf.render_order.push_back(key.clone());

    assert_eq!(app.cached_pdf_render_path(&key), None);
    assert!(!app.preview.pdf.rendered_pages.contains_key(&key));
    assert!(!app.preview.pdf.rendered_page_dimensions.contains_key(&key));
    assert!(!app.preview.pdf.render_order.contains(&key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn apply_pdf_probe_build_queues_render_for_current_page() {
    let (mut app, root) = build_pdf_overlay_test_app("probe-render-queue");
    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let page_key = PdfPageKey::from_request(&request);
    app.preview.pdf.pending_page_probes.insert(page_key);

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
    assert!(app.preview.pdf.pending_renders.contains(&render_key));
    assert!(app.jobs.scheduler.has_pending_work());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn apply_pdf_probe_build_queues_render_even_before_selection_activation_is_ready() {
    let (mut app, root) = build_pdf_overlay_test_app("probe-render-before-activation");
    app.preview.pdf.activation_ready_at = Some(Instant::now() + Duration::from_secs(5));
    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let page_key = PdfPageKey::from_request(&request);
    app.preview.pdf.pending_page_probes.insert(page_key);

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
    assert!(app.preview.pdf.pending_renders.contains(&render_key));
    assert!(app.jobs.scheduler.has_pending_work());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn apply_pdf_probe_build_prefetches_adjacent_page_probes_once_total_is_known() {
    let (mut app, root) = build_pdf_overlay_test_app("probe-prefetch-pages");
    let session = app
        .preview
        .pdf
        .session
        .as_mut()
        .expect("PDF session should exist");
    session.current_page = 2;

    let request = app
        .active_pdf_overlay_request()
        .expect("PDF overlay request should be available");
    let page_key = PdfPageKey::from_request(&request);
    app.preview.pdf.pending_page_probes.insert(page_key);

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
    assert!(app.preview.pdf.pending_page_probes.contains(&PdfPageKey {
        path: request.path.clone(),
        size: request.size,
        modified: request.modified,
        page: 1,
    }));
    assert!(app.preview.pdf.pending_page_probes.contains(&PdfPageKey {
        path: request.path.clone(),
        size: request.size,
        modified: request.modified,
        page: 3,
    }));
    assert!(app.preview.pdf.pending_page_probes.contains(&PdfPageKey {
        path: request.path,
        size: request.size,
        modified: request.modified,
        page: 4,
    }));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn remember_rendered_pdf_evicts_oldest_cached_page_when_limit_is_exceeded() {
    let (mut app, root) = build_pdf_overlay_test_app("render-cache-eviction");
    let first_key = PdfRenderKey {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        page: 1,
        width_px: 704,
        height_px: 960,
    };
    let first_path = root.join("page-1.png");

    for page in 1..=(PDF_RENDER_CACHE_LIMIT + 1) {
        let key = PdfRenderKey {
            path: root.join("demo.pdf"),
            size: 128,
            modified: None,
            page,
            width_px: 704,
            height_px: 960,
        };
        let rendered_path = root.join(format!("page-{page}.png"));
        write_test_png(&rendered_path, 704, 960);
        app.remember_rendered_pdf(
            key,
            rendered_path,
            Some(RenderedImageDimensions {
                width_px: 704,
                height_px: 960,
            }),
        );
    }

    assert_eq!(app.preview.pdf.rendered_pages.len(), PDF_RENDER_CACHE_LIMIT);
    assert_eq!(app.preview.pdf.render_order.len(), PDF_RENDER_CACHE_LIMIT);
    assert!(!app.preview.pdf.rendered_pages.contains_key(&first_key));
    assert!(
        !app.preview
            .pdf
            .rendered_page_dimensions
            .contains_key(&first_key)
    );
    assert!(!first_path.exists());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn apply_pdf_render_build_prefetches_next_page_when_current_page_is_ready() {
    let (mut app, root) = build_pdf_overlay_test_app("render-prefetch-next");
    let session = app
        .preview
        .pdf
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
                .input
                .frame_state
                .preview_content_area
                .expect("preview content area should be set"),
        };
        app.preview.pdf.page_dimensions.insert(
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
    app.preview.pdf.pending_renders.insert(current_key.clone());

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
    assert!(app.preview.pdf.pending_renders.contains(&next_key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
