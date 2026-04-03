use super::super::*;
pub(super) use crate::app::overlays::images::StaticImageKey;
pub(super) use crate::app::overlays::inline_image::{
    ImageProtocol, TerminalWindowSize, command_exists,
};
pub(super) use crate::preview::PreviewKind;
pub(super) use image::ImageFormat;
use image::{DynamicImage, Rgba, RgbaImage};
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-pdf-preview-{label}-{unique}"))
}

pub(super) fn configure_terminal_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.preview.terminal_images.protocol = ImageProtocol::KittyGraphics;
    app.preview.terminal_images.window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
}

pub(super) fn configure_iterm_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.preview.terminal_images.protocol = ImageProtocol::ItermInline;
    app.preview.terminal_images.window = Some(TerminalWindowSize {
        cells_width,
        cells_height,
        pixels_width: 1920,
        pixels_height: 1080,
    });
}

pub(super) fn build_pdf_overlay_test_app(label: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;
    app.preview.pdf.session = Some(PdfSession {
        path: root.join("demo.pdf"),
        size: 128,
        modified: None,
        current_page: 1,
        total_pages: None,
    });
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.preview.pdf.activation_ready_at = Some(Instant::now());
    (app, root)
}

pub(super) fn build_selected_pdf_app(label: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let pdf_path = root.join("demo.pdf");
    fs::write(&pdf_path, b"%PDF-1.7\n").expect("failed to write pdf placeholder");

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.refresh_preview();
    (app, root)
}

pub(super) fn write_test_png(path: &Path, width_px: u32, height_px: u32) {
    let mut bytes = vec![
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R',
    ];
    bytes.extend_from_slice(&width_px.to_be_bytes());
    bytes.extend_from_slice(&height_px.to_be_bytes());
    fs::write(path, bytes).expect("failed to write png header");
}

pub(super) fn write_test_raster_image(
    path: &Path,
    format: ImageFormat,
    width_px: u32,
    height_px: u32,
) {
    let mut image = RgbaImage::new(width_px, height_px);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([32, 128, 224, 255]);
    }

    DynamicImage::ImageRgba8(image)
        .save_with_format(path, format)
        .expect("failed to write raster test image");
}

pub(super) fn write_test_transparent_png(path: &Path, width_px: u32, height_px: u32) {
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

pub(super) fn write_test_oriented_jpeg(
    path: &Path,
    width_px: u32,
    height_px: u32,
    orientation: u16,
) {
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

pub(super) fn write_test_svg_image(path: &Path, width_px: u32, height_px: u32) {
    fs::write(
        path,
        format!(
            r#"<svg viewBox="0 0 {width_px} {height_px}" xmlns="http://www.w3.org/2000/svg"></svg>"#
        ),
    )
    .expect("failed to write svg placeholder");
}

pub(super) fn set_single_test_entry(app: &mut App, path: &Path) {
    let metadata = fs::metadata(path).expect("file metadata should exist");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("file name should be valid utf-8");
    app.navigation.entries = vec![Entry {
        path: path.to_path_buf(),
        name: name.to_string(),
        name_key: name.to_ascii_lowercase(),
        kind: EntryKind::File,
        size: metadata.len(),
        modified: None,
        readonly: false,
    }];
    app.navigation.selected = 0;
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.frame_state.metrics.cols = 1;
    app.input.frame_state.metrics.rows_visible = 6;
}

pub(super) fn build_selected_static_image_app(label: &str, file_name: &str) -> (App, PathBuf) {
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
        Some("ico") => write_test_raster_image(&image_path, ImageFormat::Ico, 64, 64),
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
    app.preview.pdf.pdf_tools_available = true;
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.frame_state.metrics.cols = 1;
    app.input.frame_state.metrics.rows_visible = 6;
    app.refresh_preview();
    (app, root)
}

pub(super) fn wait_for_displayed_static_image_overlay(app: &mut App) {
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        let _ = app.process_image_preview_timers();
        app.present_preview_overlay()
            .expect("presenting static image overlay should not fail");
        if app.static_image_overlay_displayed() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for static image overlay");
}

pub(super) fn build_selected_extensionless_png_app(label: &str, file_name: &str) -> (App, PathBuf) {
    let root = temp_root(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let image_path = root.join(file_name);
    write_test_raster_image(&image_path, ImageFormat::Png, 600, 300);

    let mut app = App::new_at(root.clone()).expect("app should initialize");
    configure_terminal_image_support(&mut app);
    app.preview.pdf.pdf_tools_available = true;
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.frame_state.metrics.cols = 1;
    app.input.frame_state.metrics.rows_visible = 6;
    app.refresh_preview();
    (app, root)
}

pub(super) fn build_multi_static_image_app(label: &str, file_names: &[&str]) -> (App, PathBuf) {
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
            Some("ico") => write_test_raster_image(&image_path, ImageFormat::Ico, 64, 64),
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
    app.preview.pdf.pdf_tools_available = true;
    app.input.frame_state.preview_content_area = Some(Rect {
        x: 2,
        y: 3,
        width: 48,
        height: 20,
    });
    app.input.frame_state.metrics.cols = 1;
    app.input.frame_state.metrics.rows_visible = 6;
    app.refresh_preview();
    (app, root)
}
