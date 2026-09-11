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

    assert_eq!(app.preview.state.content.ebook_section_index, Some(0));
    assert_eq!(app.preview.state.content.ebook_section_count, Some(2));
    assert!(app.step_epub_section(1));
    assert!(app.preview_lines().is_empty());
    assert_eq!(app.preview.state.content.ebook_section_index, Some(1));
    assert_eq!(app.preview.state.content.ebook_section_count, Some(2));
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("EPUB ebook  •  Section 2/2")
    );

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview.state.content.ebook_section_index, Some(1));
    assert_eq!(
        app.preview.state.content.ebook_section_title.as_deref(),
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
fn comic_preview_loads_when_token_is_stale_but_placeholder_is_current() {
    // Regression test for the stale-token race: if refresh_preview() is called
    // again (e.g. due to rapid navigation) after a preview job was submitted but
    // before it completes, the token is bumped and the in-flight result arrives
    // "stale".  The fix rescues such results when load_state is still Placeholder
    // for the same path and the entry + variant still match, because the comic
    // page list is deterministic and a token skew does not mean wrong content.
    let root = temp_path("comic-stale-token");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    write_binary_zip_entries(&archive, &[("1.jpg", b"page-one"), ("2.jpg", b"page-two")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    assert_eq!(app.preview_section_label(), "Comic");
    assert!(app.preview_lines().is_empty());

    // Simulate the race: bump the token without calling refresh_preview() so
    // load_state stays Placeholder and no replacement job is submitted.  The
    // original job will arrive with the now-stale token.
    app.preview.state.token = app.preview.state.token.wrapping_add(1);

    // Without the rescue the result would be dropped and this would time out.
    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Comic");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive  •  Page 1/2")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_preview_loads_when_token_is_stale_and_load_state_is_refreshing() {
    // Regression test for the Refreshing variant of the stale-token race.
    // When a comic already has a stale cached result (e.g. from a prefetch run
    // on a nearby entry), refresh_preview sets load_state = Refreshing instead
    // of Placeholder.  The in-flight job can arrive with a stale token while
    // load_state = Refreshing, which the original rescue did not cover.  The
    // result is already in the cache (line 304 caches before the stale check),
    // so without the rescue the UI stays blank until the user interacts.
    let root = temp_path("comic-stale-token-refreshing");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    write_binary_zip_entries(&archive, &[("1.jpg", b"page-one"), ("2.jpg", b"page-two")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");

    assert_eq!(app.preview_section_label(), "Comic");
    assert!(app.preview_lines().is_empty());

    // Simulate the Refreshing race: switch load_state to Refreshing (as it
    // would be after refresh_preview found a stale cache hit) and bump the
    // token so the in-flight job will arrive stale against a Refreshing state.
    app.preview.state.load_state = Some(PreviewLoadState::Refreshing(archive.clone()));
    app.preview.state.token = app.preview.state.token.wrapping_add(1);

    // Without the extended rescue (Placeholder OR Refreshing), the result
    // would be dropped and this would time out.
    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Comic");
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive  •  Page 1/2")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cbr_file_with_zip_content_loads_in_background_and_steps_pages() {
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
