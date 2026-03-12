use super::*;
use std::{
    fs,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-pdf-preview-{label}-{unique}"))
}

fn build_pdf_overlay_test_app(label: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.pdf_preview.enabled = true;
    app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);
    app.pdf_preview.session = Some(PdfSession {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        current_page: 1,
        total_pages: None,
    });
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.pdf_preview.terminal_window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
    app.pdf_preview.activation_ready_at = Some(Instant::now());
    (app, root)
}

#[test]
fn parse_pdfinfo_page_count_reads_page_field() {
    assert_eq!(
        parse_pdfinfo_page_count("Title: demo\nPages: 18\nProducer: test\n"),
        Some(18)
    );
}

#[test]
fn parse_pdfinfo_page_dimensions_reads_global_and_per_page_sizes() {
    assert_eq!(
        parse_pdfinfo_page_dimensions("Page size: 595.276 x 841.89 pts (A4)\n"),
        Some(PdfPageDimensions {
            width_pts: 595.276,
            height_pts: 841.89,
        })
    );
    assert_eq!(
        parse_pdfinfo_page_dimensions("Page    2 size: 300 x 144 pts\n"),
        Some(PdfPageDimensions {
            width_pts: 300.0,
            height_pts: 144.0,
        })
    );
}

#[test]
fn parse_window_size_reads_pixel_dimensions() {
    assert_eq!(parse_window_size("1575x919\n"), Some((1575, 919)));
}

#[test]
fn read_png_dimensions_reads_ihdr_size() {
    let root = temp_root("png-dimensions");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("page.png");
    let bytes = [
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R', 0x00, 0x00, 0x02, 0x58, 0x00, 0x00, 0x01, 0x2c,
    ];
    fs::write(&path, bytes).expect("failed to write png header");

    assert_eq!(
        read_png_dimensions(&path),
        Some(RenderedImageDimensions {
            width_px: 600,
            height_px: 300,
        })
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn bucket_render_dimensions_rounds_up_longest_edge_without_distortion() {
    assert_eq!(bucket_render_dimensions((512, 768)), (512, 768));
    assert_eq!(bucket_render_dimensions((530, 742)), (549, 768));
}

#[test]
fn select_terminal_image_backend_prefers_known_kitty_protocol_terminals() {
    assert_eq!(
        select_terminal_image_backend("xterm-kitty", "", false, false, false),
        Some(TerminalImageBackend::KittyProtocol)
    );
    assert_eq!(
        select_terminal_image_backend("xterm-256color", "ghostty", false, false, false),
        Some(TerminalImageBackend::KittyProtocol)
    );
    assert_eq!(
        select_terminal_image_backend("xterm-256color", "WezTerm", false, false, false),
        Some(TerminalImageBackend::KittyProtocol)
    );
    assert_eq!(
        select_terminal_image_backend("screen-256color", "", true, false, false),
        Some(TerminalImageBackend::KittyProtocol)
    );
}

#[test]
fn select_terminal_image_backend_falls_back_to_kitten_detection() {
    assert_eq!(
        select_terminal_image_backend("xterm-256color", "", false, true, true),
        Some(TerminalImageBackend::Kitten)
    );
    assert_eq!(
        select_terminal_image_backend("xterm-256color", "", false, true, false),
        None
    );
}

#[test]
fn fallback_window_size_pixels_uses_reasonable_cell_defaults() {
    assert_eq!(fallback_window_size_pixels(100, 40), (800, 640));
    assert_eq!(fallback_window_size_pixels(0, 0), (8, 16));
}

#[test]
fn build_kitty_display_sequence_positions_png_without_cursor_motion() {
    let path = Path::new("/tmp/demo.pdf-preview.png");
    let area = Rect {
        x: 7,
        y: 4,
        width: 30,
        height: 12,
    };

    let sequence = build_kitty_display_sequence(path, area);

    assert!(sequence.starts_with("\u{1b}[5;8H\u{1b}_G"));
    assert!(sequence.contains("a=T"));
    assert!(sequence.contains("q=2"));
    assert!(sequence.contains("f=100"));
    assert!(sequence.contains("t=f"));
    assert!(sequence.contains("c=30"));
    assert!(sequence.contains("r=12"));
    assert!(sequence.contains("C=1"));
    assert!(sequence.contains(&BASE64_STANDARD.encode(path.as_os_str().as_encoded_bytes())));
    assert!(sequence.ends_with("\u{1b}\\"));
}

#[test]
fn build_kitty_clear_sequence_deletes_visible_images() {
    assert_eq!(build_kitty_clear_sequence(), "\u{1b}_Ga=d,d=A,q=2\u{1b}\\");
}

#[test]
fn fit_pdf_page_preserves_aspect_ratio_for_wide_pages() {
    let placement = fit_pdf_page(
        Rect {
            x: 10,
            y: 4,
            width: 30,
            height: 20,
        },
        TerminalWindowSize {
            cells_width: 100,
            cells_height: 50,
            pixels_width: 1000,
            pixels_height: 1000,
        },
        PdfPageDimensions {
            width_pts: 300.0,
            height_pts: 144.0,
        },
    );

    assert!(placement.image_area.width <= 30);
    assert!(placement.image_area.height <= 20);
    assert_eq!(placement.image_area.height, 7);
    assert_eq!(placement.image_area.y, 10);
    assert!(placement.render_width_px > placement.render_height_px);
}

#[test]
fn fit_image_area_preserves_actual_rendered_png_aspect_ratio() {
    let area = fit_image_area(
        Rect {
            x: 10,
            y: 4,
            width: 30,
            height: 20,
        },
        TerminalWindowSize {
            cells_width: 100,
            cells_height: 50,
            pixels_width: 1000,
            pixels_height: 1000,
        },
        0.25,
    );

    assert_eq!(area.width, 10);
    assert_eq!(area.height, 20);
    assert_eq!(area.x, 20);
    assert_eq!(area.y, 4);
}

#[test]
fn pdf_preview_page_navigation_clamps_to_document_bounds() {
    let mut app = App::new_at(std::env::temp_dir()).expect("app should initialize");
    app.pdf_preview.enabled = true;
    app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);
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

    app.present_pdf_overlay()
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

    app.present_pdf_overlay()
        .expect("presenting a PDF overlay should not fail");
    app.present_pdf_overlay()
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
    app.pdf_preview.enabled = true;
    app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);
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
    app.pdf_preview.enabled = true;
    app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);

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
        path: request.path,
        size: request.size,
        modified: request.modified,
        page: 3,
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
fn step_pdf_page_prunes_stale_prefetch_probe_window() {
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

#[test]
fn pdf_preview_placeholder_message_tracks_loading_state() {
    let (mut app, root) = build_pdf_overlay_test_app("placeholder");

    assert_eq!(
        app.pdf_preview_placeholder_message().as_deref(),
        Some("Loading PDF page...")
    );

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

    assert_eq!(
        app.pdf_preview_placeholder_message().as_deref(),
        Some("Rendering PDF page...")
    );

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
fn sync_pdf_preview_selection_clears_stale_pdf_page_status() {
    let mut app = App::new_at(std::env::temp_dir()).expect("app should initialize");
    app.status = "PDF page 3/10".to_string();
    app.pdf_preview.enabled = true;
    app.pdf_preview.backend = Some(TerminalImageBackend::KittyProtocol);

    app.sync_pdf_preview_selection();

    assert!(app.status.is_empty());
}
