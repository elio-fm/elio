use super::*;

#[test]
fn epub_preview_uses_section_image_for_fixed_layout_pages() {
    let root = temp_path("epub-fixed-layout");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("fixed-layout.epub");
    let source_cover = root.join("fixed-layout-cover.jpg");
    write_test_raster_image(&source_cover, ImageFormat::Jpeg, 160, 240);
    let cover_bytes = fs::read(&source_cover).expect("failed to read cover image");

    let file = File::create(&path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, contents) in [
        (
            "META-INF/container.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                  <rootfiles>
                    <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                  </rootfiles>
                </container>"#,
        ),
        (
            "OPS/package.opf",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:title>Fixed Layout Story</dc:title>
                    <dc:creator>Elio</dc:creator>
                  </metadata>
                  <manifest>
                    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                    <item id="cover" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>
                    <item id="page-1" href="xhtml/page-1.xhtml" media-type="application/xhtml+xml" properties="svg"/>
                  </manifest>
                  <spine>
                    <itemref idref="page-1"/>
                  </spine>
                </package>"#,
        ),
        (
            "OPS/nav.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
                  <body>
                    <nav epub:type="toc">
                      <ol>
                        <li><a href="xhtml/page-1.xhtml">Page 1</a></li>
                      </ol>
                    </nav>
                  </body>
                </html>"#,
        ),
        (
            "OPS/xhtml/page-1.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                      <image width="160" height="240" xlink:href="../images/cover.jpg"/>
                    </svg>
                  </body>
                </html>"#,
        ),
    ] {
        zip.start_file(name, options)
            .expect("failed to start epub text entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write epub text entry");
    }
    zip.start_file("OPS/images/cover.jpg", options)
        .expect("failed to start image entry");
    zip.write_all(&cover_bytes)
        .expect("failed to write image entry");
    zip.finish().expect("failed to finish epub");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("fixed-layout page image should be extracted");

    assert_eq!(preview.detail.as_deref(), Some("EPUB ebook"));
    assert_eq!(preview.ebook_section_index, Some(0));
    assert_eq!(preview.ebook_section_count, Some(1));
    assert_eq!(preview.ebook_section_title.as_deref(), Some("Page 1"));
    assert_eq!(visual.kind, PreviewVisualKind::PageImage);
    assert_eq!(visual.layout, PreviewVisualLayout::FullHeight);
    assert_eq!(
        line_texts,
        vec!["Page   1", "Title  Fixed Layout Story", "Author Elio"]
    );
    assert!(visual.path.exists());
    assert!(
        visual
            .path
            .parent()
            .is_some_and(|parent| parent.ends_with("elio-epub-asset-v2"))
    );

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_fixed_layout_asset_cache_reuses_existing_extracted_file() {
    let root = temp_path("epub-fixed-layout-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("fixed-layout.epub");
    let source_image = root.join("shared.jpg");
    write_test_raster_image(&source_image, ImageFormat::Jpeg, 160, 240);
    let image_bytes = fs::read(&source_image).expect("failed to read shared image");

    let file = File::create(&path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, contents) in [
        (
            "META-INF/container.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                  <rootfiles>
                    <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                  </rootfiles>
                </container>"#,
        ),
        (
            "OPS/package.opf",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                  <manifest>
                    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                    <item id="page-1" href="xhtml/page-1.xhtml" media-type="application/xhtml+xml" properties="svg"/>
                  </manifest>
                  <spine>
                    <itemref idref="page-1"/>
                  </spine>
                </package>"#,
        ),
        (
            "OPS/nav.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
                  <body>
                    <nav epub:type="toc">
                      <ol>
                        <li><a href="xhtml/page-1.xhtml">Page 1</a></li>
                      </ol>
                    </nav>
                  </body>
                </html>"#,
        ),
        (
            "OPS/xhtml/page-1.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                      <image width="160" height="240" xlink:href="../images/shared.jpg"/>
                    </svg>
                  </body>
                </html>"#,
        ),
    ] {
        zip.start_file(name, options)
            .expect("failed to start epub entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write epub entry");
    }
    zip.start_file("OPS/images/shared.jpg", options)
        .expect("failed to start shared image entry");
    zip.write_all(&image_bytes)
        .expect("failed to write shared image entry");
    zip.finish().expect("failed to finish epub");

    let first_preview = build_preview(&file_entry(path.clone()));
    let first_visual = first_preview
        .preview_visual
        .clone()
        .expect("first preview should expose a page image");
    let second_preview = build_preview(&file_entry(path));
    let second_visual = second_preview
        .preview_visual
        .clone()
        .expect("second preview should expose a page image");

    assert_eq!(first_visual.kind, PreviewVisualKind::PageImage);
    assert_eq!(first_visual.layout, PreviewVisualLayout::FullHeight);
    assert_eq!(first_visual.path, second_visual.path);
    assert_eq!(first_visual.size, second_visual.size);
    assert!(first_visual.path.exists());

    let _ = fs::remove_file(first_visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn concurrent_fixed_layout_epub_section_builds_keep_shared_image_cache_readable() {
    let root = temp_path("epub-fixed-layout-concurrent");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("fixed-layout.epub");
    let source_image = root.join("shared.jpg");
    write_test_raster_image(&source_image, ImageFormat::Jpeg, 160, 240);
    let image_bytes = fs::read(&source_image).expect("failed to read shared image");

    let file = File::create(&path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, contents) in [
        (
            "META-INF/container.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                  <rootfiles>
                    <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
                  </rootfiles>
                </container>"#,
        ),
        (
            "OPS/package.opf",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:title>Shared Fixed Layout</dc:title>
                  </metadata>
                  <manifest>
                    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                    <item id="page-1" href="xhtml/page-1.xhtml" media-type="application/xhtml+xml" properties="svg"/>
                    <item id="page-2" href="xhtml/page-2.xhtml" media-type="application/xhtml+xml" properties="svg"/>
                  </manifest>
                  <spine>
                    <itemref idref="page-1"/>
                    <itemref idref="page-2"/>
                  </spine>
                </package>"#,
        ),
        (
            "OPS/nav.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
                  <body>
                    <nav epub:type="toc">
                      <ol>
                        <li><a href="xhtml/page-1.xhtml">Page 1</a></li>
                        <li><a href="xhtml/page-2.xhtml">Page 2</a></li>
                      </ol>
                    </nav>
                  </body>
                </html>"#,
        ),
        (
            "OPS/xhtml/page-1.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                      <image width="160" height="240" xlink:href="../images/shared.jpg"/>
                    </svg>
                  </body>
                </html>"#,
        ),
        (
            "OPS/xhtml/page-2.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                      <image width="160" height="240" xlink:href="../images/shared.jpg"/>
                    </svg>
                  </body>
                </html>"#,
        ),
    ] {
        zip.start_file(name, options)
            .expect("failed to start epub entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write epub entry");
    }
    zip.start_file("OPS/images/shared.jpg", options)
        .expect("failed to start shared image entry");
    zip.write_all(&image_bytes)
        .expect("failed to write shared image entry");
    zip.finish().expect("failed to finish epub");

    let path = Arc::new(path);
    let barrier = Arc::new(Barrier::new(9));
    let mut handles = Vec::new();
    for worker in 0..8 {
        let path = Arc::clone(&path);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for iteration in 0..20 {
                let preview = build_preview_with_options(
                    &file_entry((*path).clone()),
                    &PreviewRequestOptions::EpubSection((worker + iteration) % 2),
                );
                let visual = preview
                    .preview_visual
                    .as_ref()
                    .expect("fixed-layout section should expose a page image");
                let dimensions = image::ImageReader::open(&visual.path)
                    .expect("cached shared image should open")
                    .with_guessed_format()
                    .expect("shared image format should be detected")
                    .into_dimensions()
                    .expect("shared image dimensions should be readable");
                assert_eq!(dimensions, (160, 240));
            }
        }));
    }

    barrier.wait();
    for handle in handles {
        handle
            .join()
            .expect("concurrent fixed-layout worker should finish");
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
