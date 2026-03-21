use super::super::*;
use super::helpers::*;

#[test]
fn archive_preview_loads_in_background() {
    let root = temp_path("archive-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("bundle.zip");
    write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    assert_eq!(app.preview_section_label(), "Archive");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("ZIP archive")
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("Loading preview"))
    );

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Archive");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("ZIP archive")
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("docs/"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_preview_loads_in_background_and_steps_pages() {
    let root = temp_path("comic-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    write_binary_zip_entries(&archive, &[("1.jpg", b"page-one"), ("2.jpg", b"page-two")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    assert_eq!(app.preview_section_label(), "Comic");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive")
    );
    assert!(app.preview_lines().is_empty());

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Comic");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive  •  Page 1/2")
    );
    assert!(
        app.preview_lines()
            .iter()
            .all(|line| !line.to_string().contains("Contents"))
    );

    assert!(app.step_comic_page(1));
    assert_eq!(app.preview_section_label(), "Comic");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive  •  Page 2/2")
    );
    wait_for_background_preview(&mut app);

    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive  •  Page 2/2")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_keeps_section_navigation_while_next_section_loads() {
    let root = temp_path("epub-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("story.epub");
    write_epub_fixture(
        &path,
        &[
            ("Opening", "<p>First chapter text.</p>"),
            ("Second Step", "<p>Second chapter text.</p>"),
        ],
    );

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_state.content.ebook_section_index, Some(0));
    assert_eq!(app.preview_state.content.ebook_section_count, Some(2));
    assert!(app.step_epub_section(1));
    assert!(app.preview_lines().is_empty());
    assert_eq!(app.preview_state.content.ebook_section_index, Some(1));
    assert_eq!(app.preview_state.content.ebook_section_count, Some(2));
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("EPUB ebook  •  Section 2/2")
    );

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_state.content.ebook_section_index, Some(1));
    assert_eq!(
        app.preview_state.content.ebook_section_title.as_deref(),
        Some("Second Step")
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("Second chapter text."))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_rar_preview_loads_in_background_and_steps_pages() {
    let root = temp_path("comic-rar-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbr");
    write_binary_zip_entries(&archive, &[("1.jpg", b"page-one"), ("2.jpg", b"page-two")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    assert_eq!(app.preview_section_label(), "Comic");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic RAR archive")
    );
    assert!(app.preview_lines().is_empty());

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Comic");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic RAR archive  •  Page 1/2")
    );

    assert!(app.step_comic_page(1));
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic RAR archive  •  Page 2/2")
    );
    wait_for_background_preview(&mut app);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn document_preview_loads_in_background() {
    let root = temp_path("document-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let document = root.join("a.docx");
    write_docx_fixture(&document);

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    assert_eq!(app.preview_section_label(), "Document");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("DOCX document")
    );
    assert!(app.preview_lines().iter().any(|line| {
        line.to_string()
            .contains("Extracting document metadata in background")
    }));

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Document");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("DOCX document")
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("Quarterly Report"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn text_preview_loads_in_background() {
    let root = temp_path("text-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let text = root.join("note.txt");
    fs::write(&text, "plain text").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    assert_eq!(app.preview_section_label(), "Text");
    assert!(app.preview_lines().iter().any(|line| {
        line.to_string()
            .contains("Preparing file preview in background")
    }));

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Text");
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("plain text"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn image_metadata_preview_loads_in_background() {
    let root = temp_path("image-background");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let image = root.join("cover.png");
    write_test_raster_image(&image, ImageFormat::Png, 600, 300);

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    assert_eq!(app.preview_section_label(), "Image");
    assert!(app.preview_lines().iter().any(|line| {
        line.to_string()
            .contains("Preparing file preview in background")
    }));

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Image");
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("Dimensions"))
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("600x300"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
