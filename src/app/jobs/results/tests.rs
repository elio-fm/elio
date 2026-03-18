use super::*;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::{
    fs,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-preview-worker-{label}-{unique}"))
}

fn write_zip_entries(path: &Path, entries: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (name, contents) in entries {
        zip.start_file(name, options)
            .expect("failed to start zip entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write zip entry");
    }

    zip.finish().expect("failed to finish zip");
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

fn write_test_raster_image(path: &Path, format: ImageFormat, width_px: u32, height_px: u32) {
    let mut image = RgbaImage::new(width_px, height_px);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([32, 128, 224, 255]);
    }

    DynamicImage::ImageRgba8(image)
        .save_with_format(path, format)
        .expect("failed to write raster test image");
}

fn write_epub_fixture(path: &Path, sections: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("META-INF/container.xml", options)
        .expect("failed to start container entry");
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
            <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
              <rootfiles>
                <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
              </rootfiles>
            </container>"#,
    )
    .expect("failed to write container entry");

    let manifest = sections
        .iter()
        .enumerate()
        .map(|(index, _)| {
            format!(
                r#"<item id="chapter-{id}" href="text/chapter-{id}.xhtml" media-type="application/xhtml+xml"/>"#,
                id = index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let spine = sections
        .iter()
        .enumerate()
        .map(|(index, _)| format!(r#"<itemref idref="chapter-{}"/>"#, index + 1))
        .collect::<Vec<_>>()
        .join("");
    let nav = sections
        .iter()
        .enumerate()
        .map(|(index, (title, _))| {
            format!(
                r#"<li><a href="text/chapter-{id}.xhtml">{title}</a></li>"#,
                id = index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let package = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
              <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                <dc:title>Wheel Book</dc:title>
                <dc:creator>Regueiro</dc:creator>
              </metadata>
              <manifest>
                <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                {manifest}
              </manifest>
              <spine>{spine}</spine>
            </package>"#
    );
    zip.start_file("OPS/package.opf", options)
        .expect("failed to start package entry");
    zip.write_all(package.as_bytes())
        .expect("failed to write package entry");

    let nav_document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
              <body>
                <nav epub:type="toc">
                  <ol>{nav}</ol>
                </nav>
              </body>
            </html>"#
    );
    zip.start_file("OPS/nav.xhtml", options)
        .expect("failed to start nav entry");
    zip.write_all(nav_document.as_bytes())
        .expect("failed to write nav entry");

    for (index, (title, body)) in sections.iter().enumerate() {
        let chapter = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <h1>{title}</h1>
                    {body}
                  </body>
                </html>"#
        );
        zip.start_file(format!("OPS/text/chapter-{}.xhtml", index + 1), options)
            .expect("failed to start chapter entry");
        zip.write_all(chapter.as_bytes())
            .expect("failed to write chapter entry");
    }

    zip.finish().expect("failed to finish epub");
}

fn write_fixed_layout_epub_fixture(path: &Path, section_titles: &[&str]) {
    let file = File::create(path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("META-INF/container.xml", options)
        .expect("failed to start container entry");
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
            <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
              <rootfiles>
                <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
              </rootfiles>
            </container>"#,
    )
    .expect("failed to write container entry");

    let manifest = section_titles
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let id = index + 1;
            format!(
                r#"<item id="page-{id}" href="xhtml/page-{id}.xhtml" media-type="application/xhtml+xml" properties="svg"/><item id="image-{id}" href="image/page-{id}.jpg" media-type="image/jpeg"/>"#
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let spine = section_titles
        .iter()
        .enumerate()
        .map(|(index, _)| format!(r#"<itemref idref="page-{}"/>"#, index + 1))
        .collect::<Vec<_>>()
        .join("");
    let nav = section_titles
        .iter()
        .enumerate()
        .map(|(index, title)| {
            format!(
                r#"<li><a href="xhtml/page-{id}.xhtml">{title}</a></li>"#,
                id = index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let package = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
              <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                <dc:title>Fixed Layout Book</dc:title>
                <meta property="rendition:layout">pre-paginated</meta>
              </metadata>
              <manifest>
                <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                {manifest}
              </manifest>
              <spine>{spine}</spine>
            </package>"#
    );
    zip.start_file("OPS/package.opf", options)
        .expect("failed to start package entry");
    zip.write_all(package.as_bytes())
        .expect("failed to write package entry");

    let nav_document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
              <body>
                <nav epub:type="toc">
                  <ol>{nav}</ol>
                </nav>
              </body>
            </html>"#
    );
    zip.start_file("OPS/nav.xhtml", options)
        .expect("failed to start nav entry");
    zip.write_all(nav_document.as_bytes())
        .expect("failed to write nav entry");

    for (index, _) in section_titles.iter().enumerate() {
        let id = index + 1;
        let chapter = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                      <image width="1600" height="900" xlink:href="../image/page-{id}.jpg"/>
                    </svg>
                  </body>
                </html>"#
        );
        zip.start_file(format!("OPS/xhtml/page-{id}.xhtml"), options)
            .expect("failed to start chapter entry");
        zip.write_all(chapter.as_bytes())
            .expect("failed to write chapter entry");
        zip.start_file(format!("OPS/image/page-{id}.jpg"), options)
            .expect("failed to start image entry");
        zip.write_all(b"jpeg").expect("failed to write image entry");
    }

    zip.finish().expect("failed to finish epub");
}

fn wait_for_background_preview(app: &mut App) {
    for _ in 0..200 {
        if app.process_background_jobs() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for background preview");
}

fn wait_for_preview_header(app: &mut App, visible_rows: usize, width: usize, expected: &str) {
    for _ in 0..200 {
        if app
            .preview_header_detail_for_width(visible_rows, width)
            .as_deref()
            == Some(expected)
        {
            return;
        }
        let _ = app.process_background_jobs();
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for preview header {expected:?}");
}

fn wait_for_directory_load(app: &mut App) {
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.directory_runtime.pending_load.is_none() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for directory load");
}

fn write_docx_fixture(path: &Path) {
    write_zip_entries(
        path,
        &[
            (
                "docProps/core.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/"
                        xmlns:dcterms="http://purl.org/dc/terms/">
                      <dc:title>Quarterly Report</dc:title>
                      <dc:creator>Regueiro</dc:creator>
                      <dcterms:created>2026-03-11T09:00:00Z</dcterms:created>
                    </cp:coreProperties>"#,
            ),
            (
                "docProps/app.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>LibreOffice</Application>
                      <Pages>12</Pages>
                      <Words>4238</Words>
                    </Properties>"#,
            ),
        ],
    );
}

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
fn comic_preview_prefetches_adjacent_pages_for_instant_page_steps() {
    let root = temp_path("comic-page-prefetch");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    write_binary_zip_entries(
        &archive,
        &[
            ("1.jpg", b"page-one"),
            ("2.jpg", b"page-two"),
            ("3.jpg", b"page-three"),
            ("4.jpg", b"page-four"),
        ],
    );

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);

    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.has_cached_comic_preview_page(&archive, 1)
            && app.has_cached_comic_preview_page(&archive, 2)
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(app.has_cached_comic_preview_page(&archive, 1));
    assert!(app.has_cached_comic_preview_page(&archive, 2));
    assert!(app.scheduler_metrics().preview_jobs_submitted_low >= 2);

    let preview_metrics = app.preview_metrics();
    assert!(app.step_comic_page(1));
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive  •  Page 2/4")
    );

    let preview_metrics = app.preview_metrics();
    assert!(app.step_comic_page(1));
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("Comic ZIP archive  •  Page 3/4")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_prefetches_adjacent_sections_for_instant_page_steps() {
    let root = temp_path("epub-section-prefetch");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("story.epub");
    write_fixed_layout_epub_fixture(&archive, &["Page 1", "Page 2", "Page 3", "Page 4"]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);

    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.has_cached_epub_preview_section(&archive, 1)
            && app.has_cached_epub_preview_section(&archive, 2)
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(app.has_cached_epub_preview_section(&archive, 1));
    assert!(app.has_cached_epub_preview_section(&archive, 2));
    assert!(app.scheduler_metrics().preview_jobs_submitted_low >= 2);

    let preview_metrics = app.preview_metrics();
    assert!(app.step_epub_section(1));
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("EPUB ebook  •  Section 2/4")
    );

    let preview_metrics = app.preview_metrics();
    assert!(app.step_epub_section(1));
    assert_eq!(
        app.preview_metrics().cache_hits,
        preview_metrics.cache_hits + 1
    );
    assert_eq!(
        app.preview_header_detail(10).as_deref(),
        Some("EPUB ebook  •  Section 3/4")
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
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("Preparing file preview in background"))
    );

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
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("Preparing file preview in background"))
    );

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

#[test]
fn nearby_archive_preview_is_prefetched_at_low_priority() {
    let root = temp_path("archive-prefetch");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let first = root.join("a.zip");
    let second = root.join("b.zip");
    write_zip_entries(&first, &[("docs/first.txt", "hello")]);
    write_zip_entries(&second, &[("docs/second.txt", "world")]);

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    for _ in 0..100 {
        let _ = app.process_background_jobs();
        if app.has_cached_preview_for_path(&second) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(app.has_cached_preview_for_path(&second));
    let scheduler_metrics = app.scheduler_metrics();
    assert!(scheduler_metrics.preview_jobs_submitted_high >= 1);
    assert!(scheduler_metrics.preview_jobs_submitted_low >= 1);

    app.set_selected(1);
    assert_eq!(app.preview_section_label(), "Archive");
    assert!(
        app.preview_lines()
            .iter()
            .all(|line| !line.to_string().contains("Loading preview"))
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("second.txt"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn stale_archive_preview_result_is_ignored_after_selection_changes() {
    let root = temp_path("archive-stale");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("a.zip");
    write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);
    let text = root.join("z.txt");
    fs::write(&text, "plain text").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    assert_eq!(
        app.selected_entry().map(|entry| entry.name.as_str()),
        Some("a.zip")
    );

    app.set_selected(1);
    assert_eq!(
        app.selected_entry().map(|entry| entry.name.as_str()),
        Some("z.txt")
    );
    assert_eq!(app.preview_section_label(), "Text");
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("Preparing file preview in background"))
    );

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Text");
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("plain text"))
    );

    thread::sleep(Duration::from_millis(50));
    let _ = app.process_background_jobs();

    assert_eq!(app.preview_section_label(), "Text");
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("plain text"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn wrapped_text_header_reports_visual_cap_compactly() {
    let root = temp_path("wrapped-text-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let text = root.join("long.txt");
    fs::write(&text, "word ".repeat(2_000)).expect("failed to write text");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_frame_state(FrameState {
        preview_rows_visible: 8,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    let header = app
        .preview_header_detail(8)
        .expect("header detail should be present");

    assert!(header.contains("1 lines"));
    assert!(header.contains("first 240 wrapped"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn narrow_code_header_prefers_compact_subtype_and_drops_low_priority_notes() {
    let root = temp_path("narrow-code-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let source = root.join("main.rs");
    let contents = (1..=1_500)
        .map(|index| {
            format!(
                "fn line_{index}() {{ println!(\"{}\"); }}",
                "word ".repeat(20)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&source, contents).expect("failed to write source");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);
    let header = app
        .preview_header_detail_for_width(8, 20)
        .expect("header detail should be present");

    assert_eq!(header, "Rust • 240 shown");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn byte_truncated_code_header_upgrades_to_exact_total_lines_after_background_count() {
    let root = temp_path("byte-truncated-code-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let source = root.join("main.rs");
    let contents = (1..=1_500)
        .map(|index| {
            format!(
                "fn line_{index}() {{ println!(\"{}\"); }}",
                "word ".repeat(20)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&source, contents).expect("failed to write source");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);
    assert_eq!(
        app.preview_header_detail_for_width(8, 40).as_deref(),
        Some("Rust • 240 lines shown")
    );

    wait_for_preview_header(&mut app, 8, 40, "Rust • 240 / 1,500 lines shown");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn source_truncated_text_header_prefers_line_limit_over_wrapped_cap_note() {
    let root = temp_path("source-truncated-text-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let text = root.join("long.txt");
    let contents = (1..=300)
        .map(|index| format!("line {index} {}", "word ".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&text, contents).expect("failed to write text");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_frame_state(FrameState {
        preview_rows_visible: 8,
        preview_cols_visible: 20,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    let header = app
        .preview_header_detail(8)
        .expect("header detail should be present");

    assert!(header.contains("300 lines"));
    assert!(header.contains("showing first 240 lines"));
    assert!(!header.contains("wrapped"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn archive_preview_is_reused_from_cache_on_reselection() {
    let root = temp_path("archive-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("a.zip");
    write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);
    let text = root.join("z.txt");
    fs::write(&text, "plain text").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_background_preview(&mut app);

    app.set_selected(1);
    assert_eq!(app.preview_section_label(), "Text");
    let metrics_before_reselect = app.preview_metrics();

    app.set_selected(0);
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
    assert!(
        app.preview_lines()
            .iter()
            .all(|line| !line.to_string().contains("Loading preview"))
    );
    let metrics = app.preview_metrics();
    assert_eq!(metrics.cache_hits, metrics_before_reselect.cache_hits + 1);
    assert!(metrics.cache_misses >= 1);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn archive_preview_resets_scroll_after_async_refresh() {
    let root = temp_path("archive-scroll-restore");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("a.zip");
    let archive_entries = (0..10)
        .map(|index| (format!("docs/{index}.txt"), format!("hello {index}")))
        .collect::<Vec<_>>();
    let archive_refs = archive_entries
        .iter()
        .map(|(name, contents)| (name.as_str(), contents.as_str()))
        .collect::<Vec<_>>();
    write_zip_entries(&archive, &archive_refs);
    let text = root.join("z.txt");
    fs::write(&text, "plain text").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_frame_state(FrameState {
        preview_rows_visible: 4,
        preview_cols_visible: 40,
        ..FrameState::default()
    });
    wait_for_background_preview(&mut app);

    app.preview_state.scroll = 2;
    app.sync_preview_scroll();
    assert_eq!(app.preview_state.scroll, 2);

    app.set_selected(1);

    let updated_entries = (0..12)
        .map(|index| (format!("docs/{index}.txt"), format!("updated {index}")))
        .collect::<Vec<_>>();
    let updated_refs = updated_entries
        .iter()
        .map(|(name, contents)| (name.as_str(), contents.as_str()))
        .collect::<Vec<_>>();
    write_zip_entries(&archive, &updated_refs);
    app.reload().expect("reload should queue successfully");
    wait_for_directory_load(&mut app);

    app.set_selected(0);
    assert_eq!(app.preview_section_label(), "Archive");
    assert_eq!(app.preview_state.scroll, 0);
    assert!(app.preview_header_detail(10).is_some());
    assert!(
        app.preview_lines()
            .iter()
            .all(|line| !line.to_string().contains("Loading preview"))
    );

    wait_for_background_preview(&mut app);

    assert_eq!(app.preview_section_label(), "Archive");
    assert_eq!(app.preview_state.scroll, 0);
    assert!(
        app.preview_header_detail(10)
            .as_deref()
            .is_some_and(|detail| !detail.contains("Refreshing in background"))
    );
    assert!(
        app.preview_lines()
            .iter()
            .any(|line| line.to_string().contains("docs/"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn stale_preview_results_are_counted_in_metrics() {
    let root = temp_path("archive-stale-metrics");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("a.zip");
    write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);
    let text = root.join("z.txt");
    fs::write(&text, "plain text").expect("failed to write text file");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    app.set_selected(1);

    thread::sleep(Duration::from_millis(50));
    let _ = app.process_background_jobs();

    let metrics = app.preview_metrics();
    assert!(metrics.stale_results_dropped >= 1);
    assert!(metrics.applied_results <= 1);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
