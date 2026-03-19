use super::*;
use crate::app::overlays::images::StaticImageKey;
use crate::app::overlays::inline_image::{ImageProtocol, TerminalWindowSize};
use crate::preview::PreviewKind;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-pdf-preview-{label}-{unique}"))
}

fn configure_terminal_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.terminal_images.protocol = ImageProtocol::KittyGraphics;
    app.terminal_images.window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
}

fn build_pdf_overlay_test_app(label: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;
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
    app.pdf_preview.activation_ready_at = Some(Instant::now());
    (app, root)
}

fn build_selected_pdf_app(label: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let pdf_path = root.join("demo.pdf");
    fs::write(&pdf_path, b"%PDF-1.7\n").expect("failed to write pdf placeholder");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.refresh_preview();
    (app, root)
}

fn write_test_png(path: &Path, width_px: u32, height_px: u32) {
    let mut bytes = vec![
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R',
    ];
    bytes.extend_from_slice(&width_px.to_be_bytes());
    bytes.extend_from_slice(&height_px.to_be_bytes());
    fs::write(path, bytes).expect("failed to write png header");
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

fn write_test_transparent_png(path: &Path, width_px: u32, height_px: u32) {
    let mut image = RgbaImage::new(width_px, height_px);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = if (x + y) % 2 == 0 {
            Rgba([32, 128, 224, 255])
        } else {
            Rgba([0, 0, 0, 0])
        };
    }

    DynamicImage::ImageRgba8(image)
        .save_with_format(path, ImageFormat::Png)
        .expect("failed to write transparent png");
}

fn write_test_oriented_jpeg(path: &Path, width_px: u32, height_px: u32, orientation: u16) {
    write_test_raster_image(path, ImageFormat::Jpeg, width_px, height_px);

    let jpeg = fs::read(path).expect("failed to read jpeg placeholder");
    assert!(
        jpeg.starts_with(&[0xff, 0xd8]),
        "test jpeg should start with SOI"
    );

    let mut exif = Vec::with_capacity(36);
    exif.extend_from_slice(&[0xff, 0xe1, 0x00, 0x22]);
    exif.extend_from_slice(b"Exif\0\0");
    exif.extend_from_slice(b"II");
    exif.extend_from_slice(&42_u16.to_le_bytes());
    exif.extend_from_slice(&8_u32.to_le_bytes());
    exif.extend_from_slice(&1_u16.to_le_bytes());
    exif.extend_from_slice(&0x0112_u16.to_le_bytes());
    exif.extend_from_slice(&3_u16.to_le_bytes());
    exif.extend_from_slice(&1_u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&0_u16.to_le_bytes());
    exif.extend_from_slice(&0_u32.to_le_bytes());

    let mut oriented = Vec::with_capacity(jpeg.len() + exif.len());
    oriented.extend_from_slice(&jpeg[..2]);
    oriented.extend_from_slice(&exif);
    oriented.extend_from_slice(&jpeg[2..]);
    fs::write(path, oriented).expect("failed to write oriented jpeg");
}

fn write_test_svg_image(path: &Path, width_px: u32, height_px: u32) {
    fs::write(
        path,
        format!(
            r#"<svg viewBox="0 0 {width_px} {height_px}" xmlns="http://www.w3.org/2000/svg"></svg>"#
        ),
    )
    .expect("failed to write svg placeholder");
}

fn set_single_test_entry(app: &mut App, path: &Path) {
    let metadata = fs::metadata(path).expect("file metadata should exist");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("file name should be valid utf-8");
    app.entries = vec![Entry {
        path: path.to_path_buf(),
        name: name.to_string(),
        name_key: name.to_ascii_lowercase(),
        kind: EntryKind::File,
        size: metadata.len(),
        modified: None,
        readonly: false,
    }];
    app.selected = 0;
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.frame_state.metrics.cols = 1;
    app.frame_state.metrics.rows_visible = 6;
}

fn build_selected_static_image_app(label: &str, file_name: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let image_path = root.join(file_name);
    match image_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => write_test_raster_image(&image_path, ImageFormat::Png, 600, 300),
        Some("jpg") | Some("jpeg") => {
            write_test_raster_image(&image_path, ImageFormat::Jpeg, 600, 300)
        }
        Some("gif") => write_test_raster_image(&image_path, ImageFormat::Gif, 600, 300),
        Some("webp") => write_test_raster_image(&image_path, ImageFormat::WebP, 600, 300),
        Some("svg") => write_test_svg_image(&image_path, 600, 300),
        _ => panic!("unsupported test image extension: {file_name}"),
    }

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.frame_state.metrics.cols = 1;
    app.frame_state.metrics.rows_visible = 6;
    app.refresh_preview();
    (app, root)
}

fn build_selected_extensionless_png_app(label: &str, file_name: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let image_path = root.join(file_name);
    write_test_raster_image(&image_path, ImageFormat::Png, 600, 300);

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.frame_state.metrics.cols = 1;
    app.frame_state.metrics.rows_visible = 6;
    app.refresh_preview();
    (app, root)
}

fn build_multi_static_image_app(label: &str, file_names: &[&str]) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    for file_name in file_names {
        let image_path = root.join(file_name);
        match image_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref()
        {
            Some("png") => write_test_raster_image(&image_path, ImageFormat::Png, 600, 300),
            Some("jpg") | Some("jpeg") => {
                write_test_raster_image(&image_path, ImageFormat::Jpeg, 600, 300)
            }
            Some("gif") => write_test_raster_image(&image_path, ImageFormat::Gif, 600, 300),
            Some("webp") => write_test_raster_image(&image_path, ImageFormat::WebP, 600, 300),
            Some("svg") => write_test_svg_image(&image_path, 600, 300),
            Some("txt") => {
                fs::write(&image_path, "plain text").expect("failed to write text placeholder");
            }
            _ => panic!("unsupported test image extension: {file_name}"),
        }
    }

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;
    app.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.frame_state.metrics.cols = 1;
    app.frame_state.metrics.rows_visible = 6;
    app.refresh_preview();
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
    write_test_png(&path, 600, 300);

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
fn refresh_preview_uses_blank_static_image_surface_preview_when_backend_enabled() {
    for (file_name, detail) in [
        ("demo.png", "PNG image"),
        ("demo.jpg", "JPEG image"),
        ("demo.jpeg", "JPEG image"),
        ("demo.gif", "GIF image"),
        ("demo.webp", "WebP image"),
        ("demo.svg", "SVG image"),
    ] {
        let (app, root) = build_selected_static_image_app("image-placeholder", file_name);

        assert_eq!(app.preview_state.content.kind, PreviewKind::Image);
        assert_eq!(app.preview_state.content.detail.as_deref(), Some(detail));
        assert!(app.preview_state.content.lines.is_empty());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn preview_prefers_image_surface_for_supported_static_images_when_backend_enabled() {
    for (file_name, placeholder) in [
        ("demo.png", None),
        ("demo.jpg", Some("Preparing image preview")),
        ("demo.jpeg", Some("Preparing image preview")),
        ("demo.gif", Some("Preparing image preview")),
        ("demo.webp", Some("Preparing image preview")),
        ("demo.svg", Some("Preparing image preview")),
    ] {
        let (app, root) = build_selected_static_image_app("image-surface", file_name);

        assert!(app.preview_prefers_image_surface());
        assert_eq!(
            app.preview_overlay_placeholder_message().as_deref(),
            placeholder
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn preview_prefers_image_surface_for_extensionless_png_when_backend_enabled() {
    let (app, root) = build_selected_extensionless_png_app("image-surface-noext", "background");

    assert_eq!(app.preview_state.content.kind, PreviewKind::Image);
    assert_eq!(
        app.preview_state.content.detail.as_deref(),
        Some("PNG image")
    );
    assert!(app.preview_prefers_static_image_surface());
    assert!(app.preview_prefers_image_surface());
    assert_eq!(app.preview_overlay_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn current_extensionless_png_uses_direct_kitty_source_overlay() {
    let root = temp_root("image-inline-noext");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;

    let path = root.join("background");
    write_test_raster_image(&path, ImageFormat::Png, 600, 300);
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let key = StaticImageKey::from_request(&request);
    match app.prepared_static_image_for_overlay(&request) {
        crate::app::overlays::images::StaticImageOverlayPreparation::Ready(prepared) => {
            assert_eq!(prepared.display_path, path);
            assert_eq!(
                prepared.dimensions,
                RenderedImageDimensions {
                    width_px: 600,
                    height_px: 300,
                }
            );
        }
        _ => panic!("extensionless png should render directly in kitty"),
    }
    assert!(!app.image_preview.pending_prepares.contains(&key));
    assert!(app.pending_image_preview_timer().is_none());
    assert_eq!(app.preview_overlay_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn current_small_jpeg_queues_background_prepare_for_overlay() {
    let root = temp_root("image-inline-jpeg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;

    let path = root.join("photo.jpg");
    write_test_raster_image(&path, ImageFormat::Jpeg, 600, 300);
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let key = StaticImageKey::from_request(&request);
    match app.prepared_static_image_for_overlay(&request) {
        crate::app::overlays::images::StaticImageOverlayPreparation::Pending => {}
        _ => panic!("small jpeg should prepare in the background"),
    }
    assert!(app.image_preview.pending_prepares.contains(&key));
    assert_eq!(
        app.preview_overlay_placeholder_message().as_deref(),
        Some("Preparing image preview")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn current_large_jpeg_queues_background_prepare_when_ffmpeg_is_available() {
    if !crate::app::overlays::inline_image::command_exists("ffmpeg") {
        return;
    }

    let root = temp_root("image-inline-large-jpeg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.pdf_preview.pdf_tools_available = true;

    let path = root.join("photo.jpg");
    write_test_raster_image(&path, ImageFormat::Jpeg, 3200, 1800);
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let key = StaticImageKey::from_request(&request);
    match app.prepared_static_image_for_overlay(&request) {
        crate::app::overlays::images::StaticImageOverlayPreparation::Pending => {}
        _ => panic!("large jpeg should prepare in the background when ffmpeg is available"),
    }
    assert!(app.image_preview.pending_prepares.contains(&key));
    assert_eq!(
        app.preview_overlay_placeholder_message().as_deref(),
        Some("Preparing image preview")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn prepared_full_pane_image_uses_full_pane_kitty_placement() {
    let root = temp_root("image-placement-from-rendered-png");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("app should initialize");
    app.terminal_images.protocol = ImageProtocol::KittyGraphics;
    app.terminal_images.window = Some(TerminalWindowSize {
        cells_width: 100,
        cells_height: 50,
        pixels_width: 1000,
        pixels_height: 1000,
    });
    app.pdf_preview.pdf_tools_available = true;

    let path = root.join("photo.jpg");
    write_test_raster_image(&path, ImageFormat::Jpeg, 1600, 900);
    set_single_test_entry(&mut app, &path);
    app.refresh_preview();

    let request = app
        .active_static_image_overlay_request()
        .expect("image request should be available");
    let metadata = fs::metadata(&path).expect("image metadata should exist");
    let rendered = root.join("photo-rendered.png");
    write_test_raster_image(&rendered, ImageFormat::Png, 250, 540);

    let dirty = app.apply_image_prepare_build(crate::app::jobs::ImagePrepareBuild {
        path: path.clone(),
        size: metadata.len(),
        modified: None,
        target_width_px: request.target_width_px,
        target_height_px: request.target_height_px,
        force_render_to_cache: false,
        canceled: false,
        result: Some(crate::app::overlays::images::PreparedStaticImageAsset {
            display_path: rendered,
            dimensions: RenderedImageDimensions {
                width_px: 250,
                height_px: 540,
            },
        }),
    });

    assert!(dirty);
    app.image_preview.selection_activation_delay = Duration::ZERO;
    app.sync_image_preview_selection_activation();

    let output = String::from_utf8(
        app.present_preview_overlay()
            .expect("presenting prepared jpeg overlay should not fail"),
    )
    .expect("kitty output should be utf8");
    assert!(output.contains(&format!("c={}", request.area.width)));
    assert!(output.contains(&format!("r={}", request.area.height)));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn immediate_selection_changes_do_not_delay_static_image_activation() {
    let (mut app, root) = build_selected_static_image_app("image-activation", "demo.png");

    app.select_index(0);

    assert!(app.image_selection_activation_ready());
    assert!(app.pending_image_preview_timer().is_none());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn oriented_jpeg_fallback_preview_uses_exif_corrected_dimensions() {
    let root = temp_root("image-oriented-jpeg-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("portrait.jpg");
    write_test_oriented_jpeg(&path, 60, 30, 6);
    let metadata = fs::metadata(&path).expect("jpeg metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 60,
            target_height_px: 60,
            ffmpeg_available: false,
            magick_available: true,
            force_render_to_cache: false,
        },
        || false,
    )
    .expect("oriented jpeg should prepare successfully");

    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 30,
            height_px: 60,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("prepared image should open")
            .with_guessed_format()
            .expect("prepared image format should be detected")
            .into_dimensions()
            .expect("prepared image dimensions should be readable"),
        (30, 60)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn oriented_jpeg_ffmpeg_preview_uses_exif_corrected_dimensions() {
    if !crate::app::overlays::inline_image::command_exists("ffmpeg") {
        return;
    }

    let root = temp_root("image-oriented-jpeg-ffmpeg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("portrait.jpg");
    write_test_oriented_jpeg(&path, 60, 30, 6);
    let metadata = fs::metadata(&path).expect("jpeg metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 60,
            target_height_px: 60,
            ffmpeg_available: true,
            magick_available: true,
            force_render_to_cache: false,
        },
        || false,
    )
    .expect("oriented jpeg should prepare successfully");

    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 30,
            height_px: 60,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("prepared image should open")
            .with_guessed_format()
            .expect("prepared image format should be detected")
            .into_dimensions()
            .expect("prepared image dimensions should be readable"),
        (30, 60)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_png_static_image_preparation_succeeds() {
    let (_app, root) = build_selected_extensionless_png_app("image-prepare-noext", "background");
    let path = root.join("background");
    let metadata = fs::metadata(&path).expect("image metadata should exist");
    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            magick_available: true,
            force_render_to_cache: false,
        },
        || false,
    )
    .expect("static image should prepare successfully");

    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 600,
            height_px: 300,
        }
    );
    assert_ne!(prepared.display_path, path);
    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("rendered image should open")
            .with_guessed_format()
            .expect("rendered image format should be detected")
            .into_dimensions()
            .expect("rendered image should be readable"),
        (600, 300)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn static_image_surface_remains_available_without_pdf_tooling() {
    let (mut app, root) = build_selected_static_image_app("image-no-pdf-tools", "demo.png");
    app.pdf_preview.pdf_tools_available = false;
    app.refresh_preview();

    assert!(app.preview_prefers_static_image_surface());
    assert!(app.preview_prefers_image_surface());
    assert_eq!(app.preview_overlay_placeholder_message(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn raster_static_images_use_png_display_paths() {
    for file_name in ["demo.png", "demo.jpg", "demo.jpeg", "demo.gif", "demo.webp"] {
        let (_app, root) = build_selected_static_image_app("image-cache", file_name);
        let path = root.join(file_name);
        let metadata = fs::metadata(&path).expect("image metadata should exist");
        let prepared = crate::app::overlays::images::prepare_static_image_asset(
            &jobs::ImagePrepareRequest {
                path: path.clone(),
                size: metadata.len(),
                modified: None,
                target_width_px: 768,
                target_height_px: 540,
                ffmpeg_available: true,
                magick_available: true,
                force_render_to_cache: false,
            },
            || false,
        )
        .expect("static image should prepare successfully");

        assert_eq!(
            prepared
                .display_path
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("png")
        );
        assert_eq!(
            Some(prepared.dimensions),
            read_png_dimensions(&prepared.display_path)
        );
        assert_ne!(prepared.display_path, path);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn svg_static_images_are_normalized_to_cached_png_overlays() {
    let (_app, root) = build_selected_static_image_app("svg-cache", "demo.svg");
    let path = root.join("demo.svg");
    let metadata = fs::metadata(&path).expect("svg metadata should exist");
    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            magick_available: true,
            force_render_to_cache: false,
        },
        || false,
    )
    .expect("svg image should prepare successfully");

    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 600,
            height_px: 300,
        }
    );
    assert_ne!(prepared.display_path, path);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_svg_static_image_preparation_succeeds() {
    let root = temp_root("svg-noext-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("logo");
    write_test_svg_image(&path, 600, 300);
    let metadata = fs::metadata(&path).expect("svg metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            magick_available: true,
            force_render_to_cache: false,
        },
        || false,
    )
    .expect("extensionless svg should prepare successfully");

    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 600,
            height_px: 300,
        }
    );
    assert_ne!(prepared.display_path, path);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn oversized_png_static_images_are_normalized_to_cached_overlays() {
    let root = temp_root("large-png-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("large.png");
    write_test_raster_image(&path, ImageFormat::Png, 3200, 1800);
    let metadata = fs::metadata(&path).expect("png metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            magick_available: true,
            force_render_to_cache: false,
        },
        || false,
    )
    .expect("large png should prepare successfully");

    assert_ne!(prepared.display_path, path);
    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 768,
            height_px: 432,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("rendered image should open")
            .with_guessed_format()
            .expect("rendered image format should be detected")
            .into_dimensions()
            .expect("rendered image should be readable"),
        (768, 432)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn forced_png_preview_renders_a_cached_overlay_asset() {
    let root = temp_root("forced-png-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("page.png");
    write_test_raster_image(&path, ImageFormat::Png, 3200, 1800);
    let metadata = fs::metadata(&path).expect("png metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            magick_available: true,
            force_render_to_cache: true,
        },
        || false,
    )
    .expect("forced png preview should prepare successfully");

    assert_ne!(prepared.display_path, path);
    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 768,
            height_px: 432,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("rendered image should open")
            .with_guessed_format()
            .expect("rendered image format should be detected")
            .into_dimensions()
            .expect("rendered image should be readable"),
        (768, 432)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn oversized_extensionless_png_static_images_are_normalized_to_cached_overlays() {
    let root = temp_root("large-png-noext-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("background");
    write_test_raster_image(&path, ImageFormat::Png, 3200, 1800);
    let metadata = fs::metadata(&path).expect("png metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 768,
            target_height_px: 540,
            ffmpeg_available: true,
            magick_available: true,
            force_render_to_cache: false,
        },
        || false,
    )
    .expect("large extensionless png should prepare successfully");

    assert_ne!(prepared.display_path, path);
    assert_eq!(
        prepared
            .display_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!(
        prepared.dimensions,
        RenderedImageDimensions {
            width_px: 768,
            height_px: 432,
        }
    );
    assert_eq!(
        image::ImageReader::open(&prepared.display_path)
            .expect("rendered image should open")
            .with_guessed_format()
            .expect("rendered image format should be detected")
            .into_dimensions()
            .expect("rendered image should be readable"),
        (768, 432)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn refresh_preview_preloads_current_and_visible_nearby_static_images() {
    let (mut app, root) = build_multi_static_image_app(
        "image-preload-window",
        &["a.jpg", "b.txt", "c.png", "d.webp", "e.svg"],
    );
    app.set_selected(2);

    let current_request = app
        .active_static_image_overlay_request()
        .expect("current image request should be available");
    let target_width_px = current_request.target_width_px;
    let target_height_px = current_request.target_height_px;

    let expected = app
        .visible_entry_indices()
        .into_iter()
        .filter_map(|index| app.entries.get(index))
        .filter(|entry| crate::app::overlays::images::static_image_detail_label(entry).is_some())
        .filter(|entry| {
            crate::file_info::inspect_path_cached(
                &entry.path,
                entry.kind,
                entry.size,
                entry.modified,
            )
            .specific_type_label
                != Some("PNG image")
        })
        .map(|entry| {
            StaticImageKey::from_parts(
                entry.path.clone(),
                entry.size,
                entry.modified,
                target_width_px,
                target_height_px,
                false,
            )
        })
        .collect::<Vec<_>>();

    for key in expected {
        assert!(
            app.image_preview.pending_prepares.contains(&key)
                || app.image_preview.dimensions.contains_key(&key),
            "expected image preload for {:?}",
            key
        );
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn bucket_render_dimensions_rounds_up_longest_edge_without_distortion() {
    assert_eq!(bucket_render_dimensions((512, 768)), (512, 768));
    assert_eq!(bucket_render_dimensions((530, 742)), (549, 768));
}

#[test]
fn select_image_protocol_kitty_always_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::Kitty, false),
        ImageProtocol::KittyGraphics
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Kitty, true),
        ImageProtocol::KittyGraphics
    );
}

#[test]
fn select_image_protocol_ghostty_always_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::Ghostty, false),
        ImageProtocol::KittyGraphics
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Ghostty, true),
        ImageProtocol::KittyGraphics
    );
}

#[test]
fn select_image_protocol_wezterm_always_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::WezTerm, false),
        ImageProtocol::ItermInline
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::WezTerm, true),
        ImageProtocol::ItermInline
    );
}

#[test]
fn select_image_protocol_warp_always_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::Warp, false),
        ImageProtocol::KittyGraphics
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Warp, true),
        ImageProtocol::KittyGraphics
    );
}

#[test]
fn select_image_protocol_alacritty_disabled_and_other_override_enabled() {
    assert_eq!(
        select_image_protocol(TerminalIdentity::Alacritty, true),
        ImageProtocol::None
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Other, false),
        ImageProtocol::None
    );
    assert_eq!(
        select_image_protocol(TerminalIdentity::Other, true),
        ImageProtocol::KittyGraphics
    );
}

#[test]
fn fallback_window_size_pixels_uses_reasonable_cell_defaults() {
    assert_eq!(fallback_window_size_pixels(100, 40), (800, 640));
    assert_eq!(fallback_window_size_pixels(0, 0), (8, 16));
}

#[test]
fn build_kitty_upload_sequence_uses_unicode_placeholder_mode() {
    let path = Path::new("/tmp/demo.pdf-preview.png");
    let id = 42_u32;
    let area = Rect {
        x: 10,
        y: 4,
        width: 30,
        height: 20,
    };

    let sequence = build_kitty_upload_sequence(path, id, area);

    assert!(sequence.starts_with("\u{1b}_G"));
    assert!(sequence.contains("a=T"));
    assert!(sequence.contains("q=2"));
    assert!(sequence.contains("t=f"));
    assert!(sequence.contains("U=1"));
    assert!(sequence.contains(&format!("i={id}")));
    assert!(sequence.contains("p=1"));
    assert!(sequence.contains("c=30"));
    assert!(sequence.contains("r=20"));
    assert!(sequence.contains("C=1"));
    assert!(sequence.contains(&BASE64_STANDARD.encode(path.as_os_str().as_encoded_bytes())));
    assert!(sequence.ends_with("\u{1b}\\"));
}

#[test]
fn kitty_placeholder_sequence_sets_panel_background_for_transparency() {
    let sequence = String::from_utf8(build_kitty_placeholder_sequence(
        42,
        Rect {
            x: 1,
            y: 2,
            width: 2,
            height: 2,
        },
        &[],
    ))
    .expect("placeholder sequence should be utf8");

    assert!(sequence.contains("[38;2;"));
    assert!(sequence.contains(";48;2;"));
    assert!(sequence.contains(";58;2;0;0;1m"));
}

#[test]
fn png_static_image_preparation_preserves_alpha_channel() {
    let root = temp_root("png-alpha");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("alpha.png");
    write_test_transparent_png(&path, 8, 8);
    let metadata = fs::metadata(&path).expect("png metadata should exist");

    let prepared = crate::app::overlays::images::prepare_static_image_asset(
        &jobs::ImagePrepareRequest {
            path: path.clone(),
            size: metadata.len(),
            modified: None,
            target_width_px: 8,
            target_height_px: 8,
            ffmpeg_available: true,
            magick_available: true,
            force_render_to_cache: false,
        },
        || false,
    )
    .expect("png should prepare successfully");

    let pixels = image::ImageReader::open(&prepared.display_path)
        .expect("prepared image should open")
        .with_guessed_format()
        .expect("prepared image format should be detected")
        .decode()
        .expect("prepared image should decode")
        .into_rgba8();

    assert_eq!(pixels.get_pixel(1, 0).0[3], 0);
    assert_eq!(pixels.get_pixel(0, 0).0[3], 255);

    fs::remove_dir_all(root).expect("failed to remove temp root");
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
