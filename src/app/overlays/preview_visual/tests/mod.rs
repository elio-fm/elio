use super::*;
use crate::app::overlays::images::{
    StaticImageKey, StaticImageOverlayMode, StaticImageOverlayRequest, image_target_height_px,
    image_target_width_px,
};
use crate::app::overlays::inline_image::{ImageProtocol, TerminalWindowSize};
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
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

mod cache;
mod comic;
mod document;
mod preload;

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-preview-visual-{label}-{unique}"))
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

fn configure_iterm_image_support(app: &mut App) {
    let (cells_width, cells_height) = crossterm::terminal::size().unwrap_or((120, 40));
    app.terminal_images.protocol = ImageProtocol::ItermInline;
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
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let _ = app.process_background_jobs();
        let _ = app.process_image_preview_timers();
        app.present_preview_overlay()
            .expect("presenting preview overlay should not fail");
        if app.static_image_overlay_displayed() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for preview overlay");
}

fn wait_for_preview_prefetch(app: &mut App) {
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        let _ = app.process_preview_prefetch_timers();
        if app.pending_preview_prefetch_timer().is_none() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for preview prefetch");
}
