use super::*;
use crate::app::overlays::images::{
    StaticImageKey, StaticImageOverlayMode, StaticImageOverlayRequest, image_target_height_px,
    image_target_width_px,
};
use crate::app::overlays::inline_image::{TerminalImageBackend, TerminalWindowSize};
use crate::preview::{
    PreviewContent, PreviewKind, PreviewVisual, PreviewVisualKind, PreviewVisualLayout,
};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use ratatui::layout::Rect;
use std::{
    fs,
    fs::File,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-preview-visual-{label}-{unique}"))
}

fn configure_terminal_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.terminal_images.backend = Some(TerminalImageBackend::KittyProtocol);
    app.terminal_images.window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
}

fn write_test_raster_image(path: &Path, format: ImageFormat, width_px: u32, height_px: u32) {
    let mut image = RgbaImage::new(width_px, height_px);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([32, 128, 224, 255]);
    }

    DynamicImage::ImageRgba8(image)
        .save_with_format(path, format)
        .expect("failed to write raster test image");
}

fn raster_image_bytes(format: ImageFormat, width_px: u32, height_px: u32) -> Vec<u8> {
    let mut image = RgbaImage::new(width_px, height_px);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([32, 128, 224, 255]);
    }

    let mut bytes = Vec::new();
    DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut bytes), format)
        .expect("failed to write raster image bytes");
    bytes
}

fn write_binary_zip_entries(path: &Path, entries: &[(&str, &[u8])]) {
    let file = File::create(path).expect("failed to create zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (name, contents) in entries {
        zip.start_file(name, options)
            .expect("failed to start zip entry");
        zip.write_all(contents).expect("failed to write zip entry");
    }

    zip.finish().expect("failed to finish zip");
}

fn wait_for_displayed_preview_overlay(app: &mut App) {
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        app.present_preview_overlay()
            .expect("presenting preview overlay should not fail");
        if app.static_image_overlay_displayed() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for preview overlay");
}

#[test]
fn preview_visual_overlay_request_uses_asset_metadata() {
    let root = temp_root("request-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let asset_path = root.join("page.jpg");
    fs::write(&asset_path, b"jpeg").expect("failed to write image placeholder");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.entries = vec![Entry {
        path: root.join("book.cbz"),
        name: "book.cbz".to_string(),
        name_key: "book.cbz".to_string(),
        kind: EntryKind::File,
        size: 134 * 1024 * 1024,
        modified: None,
        readonly: false,
    }];
    app.selected = 0;
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Archive, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::Inline,
            path: asset_path.clone(),
            size: 11 * 1024,
            modified: None,
        });

    let request = app
        .active_preview_visual_overlay_request()
        .expect("preview visual overlay request should be available");

    assert_eq!(request.path, asset_path);
    assert_eq!(request.size, 11 * 1024);
    assert_eq!(request.modified, None);
    assert!(!request.force_render_to_cache);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_page_overlay_request_forces_rendered_cache() {
    let root = temp_root("comic-force-render");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: root.join("page.png"),
            size: 11 * 1024,
            modified: None,
        });

    let request = app
        .active_preview_visual_overlay_request()
        .expect("comic preview visual overlay request should be available");

    assert!(request.force_render_to_cache);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_page_ffmpeg_render_uses_fast_raster_args() {
    let default_args: &[&str] = &[];
    let comic_args: &[&str] = &["-compression_level", "1", "-sws_flags", "fast_bilinear"];

    assert_eq!(
        crate::app::overlays::images::ffmpeg_raster_render_args(false),
        default_args
    );
    assert_eq!(
        crate::app::overlays::images::ffmpeg_raster_render_args(true),
        comic_args
    );
}

#[test]
fn page_image_visual_uses_full_preview_height() {
    let root = temp_root("full-height");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview_state.content = PreviewContent::new(PreviewKind::Archive, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: root.join("page.jpg"),
            size: 11 * 1024,
            modified: None,
        });

    assert_eq!(
        app.preview_visual_rows(Rect {
            x: 0,
            y: 0,
            width: 48,
            height: 20,
        }),
        Some(20)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn failed_full_height_page_image_falls_back_to_text_layout() {
    let root = temp_root("failed-full-height");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: root.join("page.jpg"),
            size: 11 * 1024,
            modified: None,
        });
    let area = Rect {
        x: 0,
        y: 0,
        width: 48,
        height: 20,
    };
    let request = StaticImageOverlayRequest {
        path: root.join("page.jpg"),
        size: 11 * 1024,
        modified: None,
        area,
        target_width_px: image_target_width_px(area, app.cached_terminal_window()),
        target_height_px: image_target_height_px(area, app.cached_terminal_window()),
        mode: StaticImageOverlayMode::Inline,
        force_render_to_cache: true,
    };
    app.image_preview
        .failed_images
        .insert(StaticImageKey::from_request(&request));

    assert_eq!(app.preview_visual_rows(area), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn inline_page_image_leaves_room_for_summary_text() {
    let root = temp_root("inline-page");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::Inline,
            path: root.join("page.jpg"),
            size: 11 * 1024,
            modified: None,
        });

    assert_eq!(
        app.preview_visual_rows(Rect {
            x: 0,
            y: 0,
            width: 48,
            height: 20,
        }),
        Some(14)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn page_image_placeholder_message_stays_silent() {
    let root = temp_root("page-placeholder");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_media_area = Some(Rect {
        x: 0,
        y: 0,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: root.join("page.jpg"),
            size: 11 * 1024,
            modified: None,
        });

    assert_eq!(app.preview_visual_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_jpeg_page_waits_for_background_prepare_instead_of_inline_display() {
    let root = temp_root("comic-jpeg-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let page = root.join("page.jpg");
    write_test_raster_image(&page, ImageFormat::Jpeg, 1600, 900);
    let page_size = fs::metadata(&page)
        .expect("page image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: page,
            size: page_size,
            modified: None,
        });
    let request = app
        .active_preview_visual_overlay_request()
        .expect("comic jpeg request should be available");
    let key = StaticImageKey::from_request(&request);

    app.refresh_static_image_preloads();
    app.present_preview_overlay()
        .expect("presenting a comic jpeg overlay should not fail");

    assert!(!app.static_image_overlay_displayed());
    assert!(app.image_preview.pending_prepares.contains(&key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn current_comic_prepare_build_marks_preview_dirty() {
    let root = temp_root("comic-prepare-dirty");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let source = root.join("page.jpg");
    let rendered = root.join("page-rendered.png");
    write_test_raster_image(&source, ImageFormat::Jpeg, 1600, 900);
    write_test_raster_image(&rendered, ImageFormat::Png, 768, 432);
    let metadata = fs::metadata(&source).expect("source image metadata should exist");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: source.clone(),
            size: metadata.len(),
            modified: None,
        });

    let dirty = app.apply_image_prepare_build(crate::app::jobs::ImagePrepareBuild {
        path: source,
        size: metadata.len(),
        modified: None,
        target_width_px: image_target_width_px(
            app.frame_state
                .preview_media_area
                .expect("preview media area should exist"),
            app.cached_terminal_window(),
        ),
        target_height_px: image_target_height_px(
            app.frame_state
                .preview_media_area
                .expect("preview media area should exist"),
            app.cached_terminal_window(),
        ),
        force_render_to_cache: true,
        canceled: false,
        result: Some(crate::app::overlays::images::PreparedStaticImageAsset {
            display_path: rendered,
            dimensions: crate::app::overlays::inline_image::RenderedImageDimensions {
                width_px: 1600,
                height_px: 900,
            },
        }),
    });

    assert!(dirty);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cached_adjacent_comic_page_queues_background_image_prepare() {
    let root = temp_root("comic-adjacent-image-prepare");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    fs::write(&archive, b"cbz").expect("failed to write archive placeholder");
    let archive_metadata = fs::metadata(&archive).expect("archive metadata should exist");
    let current_page = root.join("page-1.jpg");
    let next_page = root.join("page-2.jpg");
    write_test_raster_image(&current_page, ImageFormat::Jpeg, 1600, 900);
    write_test_raster_image(&next_page, ImageFormat::Jpeg, 1600, 900);
    let next_page_metadata = fs::metadata(&next_page).expect("next page metadata should exist");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.entries = vec![Entry {
        path: archive.clone(),
        name: "issue.cbz".to_string(),
        name_key: "issue.cbz".to_string(),
        kind: EntryKind::File,
        size: archive_metadata.len(),
        modified: archive_metadata.modified().ok(),
        readonly: false,
    }];
    app.selected = 0;
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.sync_comic_preview_selection();
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_navigation_position("Page", 0, 2, None)
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: current_page,
            size: 11 * 1024,
            modified: None,
        });
    app.apply_current_comic_preview_metadata();

    let adjacent_preview = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_navigation_position("Page", 1, 2, None)
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: next_page,
            size: next_page_metadata.len(),
            modified: next_page_metadata.modified().ok(),
        });
    let entry = app
        .selected_entry()
        .cloned()
        .expect("selected entry should exist");
    app.cache_preview_result(
        &entry,
        &preview::PreviewRequestOptions::ComicPage(1),
        &adjacent_preview,
    );
    let adjacent_request = app.preview_visual_overlay_request_for_visual(
        PreviewKind::Comic,
        adjacent_preview
            .preview_visual
            .as_ref()
            .expect("adjacent preview should have a visual"),
        app.frame_state
            .preview_media_area
            .expect("preview media area should exist"),
    );
    let adjacent_key = StaticImageKey::from_request(&adjacent_request);

    app.refresh_static_image_preloads();

    assert!(app.image_preview.pending_prepares.contains(&adjacent_key));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn stale_adjacent_comic_preview_result_immediately_queues_image_prepare() {
    let root = temp_root("comic-stale-adjacent-image-prepare");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    let page_one = raster_image_bytes(ImageFormat::Jpeg, 1600, 900);
    let page_two = raster_image_bytes(ImageFormat::Jpeg, 1600, 900);
    write_binary_zip_entries(&archive, &[("1.jpg", &page_one), ("2.jpg", &page_two)]);

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });

    let mut adjacent_key = None;
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if !app.has_cached_comic_preview_page(&archive, 1) {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        adjacent_key = app
            .nearby_comic_preview_visual_overlay_requests()
            .into_iter()
            .next()
            .map(|request| StaticImageKey::from_request(&request));
        if adjacent_key.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let adjacent_key = adjacent_key.expect("adjacent comic preview should be cached");
    assert!(
        app.image_preview.pending_prepares.contains(&adjacent_key)
            || app.image_preview.dimensions.contains_key(&adjacent_key)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_overlay_keeps_previous_page_visible_while_next_page_waits() {
    let root = temp_root("comic-overlay-pending");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let first = root.join("001.png");
    let second = root.join("002.png");
    write_test_raster_image(&first, ImageFormat::Png, 600, 300);
    write_test_raster_image(&second, ImageFormat::Png, 600, 300);
    let first_size = fs::metadata(&first)
        .expect("first image metadata should exist")
        .len();
    let second_size = fs::metadata(&second)
        .expect("second image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: first,
            size: first_size,
            modified: None,
        });

    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: second,
            size: second_size,
            modified: None,
        });
    let next_request = app
        .active_preview_visual_overlay_request()
        .expect("next comic preview request should be available");
    let next_key = StaticImageKey::from_request(&next_request);
    app.refresh_static_image_preloads();
    app.last_selection_change_at = Instant::now() - std::time::Duration::from_secs(1);
    app.sync_image_preview_selection_activation();

    app.present_preview_overlay()
        .expect("pending comic page transition should not fail");
    assert!(app.static_image_overlay_displayed());
    assert!(app.image_preview.pending_prepares.contains(&next_key));
    assert!(!app.displayed_static_image_matches_active());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_overlay_keeps_previous_page_visible_while_next_page_preview_loads() {
    let root = temp_root("comic-overlay-preview-loading");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    fs::write(&archive, b"cbz").expect("failed to write archive placeholder");
    let archive_metadata = fs::metadata(&archive).expect("archive metadata should exist");
    let first = root.join("001.png");
    write_test_raster_image(&first, ImageFormat::Png, 600, 300);
    let first_size = fs::metadata(&first)
        .expect("first image metadata should exist")
        .len();

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.frame_state.preview_media_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_navigation_position("Page", 0, 2, None)
        .with_preview_visual(PreviewVisual {
            kind: PreviewVisualKind::PageImage,
            layout: PreviewVisualLayout::FullHeight,
            path: first,
            size: first_size,
            modified: None,
        });
    app.refresh_static_image_preloads();
    wait_for_displayed_preview_overlay(&mut app);
    assert!(app.static_image_overlay_displayed());

    app.entries = vec![Entry {
        path: archive.clone(),
        name: "issue.cbz".to_string(),
        name_key: "issue.cbz".to_string(),
        kind: EntryKind::File,
        size: archive_metadata.len(),
        modified: archive_metadata.modified().ok(),
        readonly: false,
    }];
    app.selected = 0;
    app.sync_comic_preview_selection();
    app.preview_state.content = PreviewContent::new(PreviewKind::Comic, Vec::new())
        .with_navigation_position("Page", 1, 2, None);
    app.preview_state.load_state = Some(PreviewLoadState::Placeholder(archive));
    app.present_preview_overlay()
        .expect("comic preview loading transition should not clear the overlay");

    assert!(app.static_image_overlay_displayed());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
