use super::*;

#[test]
fn comic_zip_preview_uses_natural_page_order_and_page_selection() {
    let root = temp_path("comic-zip-pages");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("issue.cbz");
    write_zip_binary_entries(
        &path,
        &[
            ("18376941278364981273/10.jpg", b"page-ten"),
            ("18376941278364981273/2.jpg", b"page-two"),
            ("18376941278364981273/1.jpg", b"page-one"),
        ],
    );

    let first_preview = build_preview(&file_entry(path.clone()));
    let first_visual = first_preview
        .preview_visual
        .as_ref()
        .expect("first page should be extracted");
    let second_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::ComicPage(1),
    );
    let second_visual = second_preview
        .preview_visual
        .as_ref()
        .expect("second page should be extracted");
    let third_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::ComicPage(2),
    );
    let third_visual = third_preview
        .preview_visual
        .as_ref()
        .expect("third page should be extracted");

    assert_eq!(
        fs::read(&first_visual.path).expect("failed to read first page"),
        b"page-one"
    );
    assert_eq!(
        fs::read(&second_visual.path).expect("failed to read second page"),
        b"page-two"
    );
    assert_eq!(
        fs::read(&third_visual.path).expect("failed to read third page"),
        b"page-ten"
    );
    assert_eq!(
        second_preview
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert_eq!(
        third_preview
            .navigation_position
            .as_ref()
            .map(|position| position.count),
        Some(3)
    );
    let second_line_texts: Vec<_> = second_preview.lines.iter().map(line_text).collect();
    assert!(second_line_texts.is_empty());
    assert!(!second_line_texts.iter().any(|text| text.contains("2.jpg")));
    assert!(
        !second_line_texts
            .iter()
            .any(|text| text.contains("18376941278364981273"))
    );

    let _ = fs::remove_file(first_visual.path.clone());
    let _ = fs::remove_file(second_visual.path.clone());
    let _ = fs::remove_file(third_visual.path.clone());
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cbr_file_with_zip_content_renders_as_comic_preview() {
    let root = temp_path("comic-rar-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("issue.cbr");
    let source_cover = root.join("cover.jpg");
    write_test_raster_image(&source_cover, ImageFormat::Jpeg, 160, 240);
    let cover_bytes = fs::read(&source_cover).expect("failed to read cover image");
    write_zip_binary_entries(
        &path,
        &[
            ("001-cover.jpg", &cover_bytes),
            ("002-page.jpg", &cover_bytes),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic rar should expose a page visual");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic RAR archive"));
    assert_eq!(visual.kind, PreviewVisualKind::PageImage);
    assert_eq!(visual.layout, PreviewVisualLayout::FullHeight);
    assert_eq!(
        preview.navigation_position.as_ref().map(|position| (
            position.label,
            position.index,
            position.count
        )),
        Some(("Page", 0, 2))
    );
    assert!(visual.path.exists());
    assert!(line_texts.is_empty());

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
