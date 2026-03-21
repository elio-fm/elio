use super::*;

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
