use super::*;

#[test]
fn mobi_and_azw3_previews_attach_exth_cover_visual() {
    for (file_name, detail) in [
        ("covered.mobi", "MOBI ebook"),
        ("covered.azw3", "AZW3 ebook"),
    ] {
        let root = temp_path(&format!("kindle-cover-{}", file_name.replace('.', "-")));
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join(file_name);
        let cover_bytes = raster_image_bytes(&root, ImageFormat::Png, 24, 36);
        let cover_offset = 0_u32.to_be_bytes();
        write_synthetic_kindle_with_resources(
            &path,
            "Covered Handbook",
            &[
                (201, cover_offset.as_slice()),
                (503, b"Covered Handbook".as_slice()),
            ],
            &[cover_bytes.as_slice()],
        );

        let preview = build_preview(&file_entry(path));
        let visual = preview
            .preview_visual
            .clone()
            .expect("kindle preview should attach the cover image");
        let dimensions = image::ImageReader::open(&visual.path)
            .expect("cover cache should open")
            .with_guessed_format()
            .expect("cover cache format should be detected")
            .into_dimensions()
            .expect("cover cache dimensions should decode");

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some(detail));
        assert_eq!(visual.kind, PreviewVisualKind::Cover);
        assert_eq!(visual.layout, PreviewVisualLayout::LargeInline);
        assert_eq!(visual.size, cover_bytes.len() as u64);
        assert_eq!(dimensions, (24, 36));
        assert!(visual.path.extension().is_some_and(|ext| ext == "png"));

        let _ = fs::remove_file(visual.path);
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn kindle_preview_does_not_guess_cover_without_exth_pointer() {
    let root = temp_path("kindle-cover-no-pointer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("covered.mobi");
    let cover_bytes = raster_image_bytes(&root, ImageFormat::Jpeg, 24, 36);
    write_synthetic_kindle_with_resources(
        &path,
        "Covered Handbook",
        &[(503, b"Covered Handbook".as_slice())],
        &[cover_bytes.as_slice()],
    );

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Document);
    assert!(preview.preview_visual.is_none());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
#[test]
fn kindle_preview_falls_back_to_thumbnail_when_cover_record_is_not_an_image() {
    let root = temp_path("kindle-cover-thumbnail-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("covered.azw3");
    let thumbnail_bytes = raster_image_bytes(&root, ImageFormat::Png, 12, 18);
    let cover_offset = 0_u32.to_be_bytes();
    let thumb_offset = 1_u32.to_be_bytes();
    write_synthetic_kindle_with_resources(
        &path,
        "Covered Handbook",
        &[
            (201, cover_offset.as_slice()),
            (202, thumb_offset.as_slice()),
            (503, b"Covered Handbook".as_slice()),
        ],
        &[b"not an image".as_slice(), thumbnail_bytes.as_slice()],
    );

    let preview = build_preview(&file_entry(path));
    let visual = preview
        .preview_visual
        .clone()
        .expect("kindle preview should fall back to thumbnail image");
    let dimensions = image::ImageReader::open(&visual.path)
        .expect("thumbnail cache should open")
        .with_guessed_format()
        .expect("thumbnail cache format should be detected")
        .into_dimensions()
        .expect("thumbnail cache dimensions should decode");

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(visual.kind, PreviewVisualKind::Cover);
    assert_eq!(visual.layout, PreviewVisualLayout::LargeInline);
    assert_eq!(visual.size, thumbnail_bytes.len() as u64);
    assert_eq!(dimensions, (12, 18));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
#[test]
fn kindle_preview_skips_cover_for_encrypted_books() {
    let root = temp_path("kindle-cover-encrypted");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("encrypted.azw3");
    let cover_bytes = raster_image_bytes(&root, ImageFormat::Png, 24, 36);
    let cover_offset = 0_u32.to_be_bytes();
    write_synthetic_kindle_with_options(
        &path,
        "Encrypted Handbook",
        &[
            (201, cover_offset.as_slice()),
            (503, b"Encrypted Handbook".as_slice()),
        ],
        &[cover_bytes.as_slice()],
        2,
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert!(preview.preview_visual.is_none());
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Encryption") && text.contains("Mobipocket encryption"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
