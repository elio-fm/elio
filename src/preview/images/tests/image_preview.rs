use crate::fs::{Entry, EntryKind};
use crate::preview::{PreviewKind, build_preview};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use ratatui::text::Line;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-preview-{label}-{unique}"))
}

fn file_entry(path: PathBuf) -> Entry {
    Entry {
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        name_key: path.file_name().unwrap().to_string_lossy().to_lowercase(),
        path,
        kind: EntryKind::File,
        symlink: None,
        size: 0,
        modified: None,
        readonly: false,
    }
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
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

#[test]
fn raster_image_preview_uses_image_metadata_fallback() {
    let root = temp_path("image-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("cover.png");
    write_test_raster_image(&path, ImageFormat::Png, 600, 300);

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Image);
    assert_eq!(preview.detail.as_deref(), Some("PNG image"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Dimensions") && line.contains("600x300"))
    );
    assert!(
        line_texts
            .iter()
            .all(|line| !line.contains("Binary or unsupported file"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_png_preview_uses_image_metadata_fallback() {
    let root = temp_path("image-metadata-noext");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("background");
    write_test_raster_image(&path, ImageFormat::Png, 600, 300);

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Image);
    assert_eq!(preview.detail.as_deref(), Some("PNG image"));
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Dimensions") && line.contains("600x300"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn ico_preview_uses_image_metadata_fallback() {
    let root = temp_path("image-metadata-ico");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("favicon.ico");
    write_test_raster_image(&path, ImageFormat::Ico, 64, 64);

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Image);
    assert_eq!(preview.detail.as_deref(), Some("ICO image"));
    assert!(
        line_texts
            .iter()
            .any(|line| line.contains("Dimensions") && line.contains("64x64"))
    );
    assert!(
        line_texts
            .iter()
            .all(|line| !line.contains("Binary or unsupported file"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
