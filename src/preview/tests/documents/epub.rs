use super::*;

#[test]
fn epub_preview_shows_package_metadata() {
    let root = temp_path("epub");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("novel.epub");
    write_zip_entries(
        &path,
        &[
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
                        <dc:title>Elio Handbook</dc:title>
                        <dc:creator>Regueiro</dc:creator>
                        <dc:language>en</dc:language>
                        <dc:publisher>Elio Docs</dc:publisher>
                        <dc:identifier>urn:uuid:elio-handbook</dc:identifier>
                        <dc:date>2026-03-12T08:00:00Z</dc:date>
                      </metadata>
                    </package>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("Elio Handbook"));
    assert_eq!(
        preview.status_note.as_deref(),
        Some("EPUB ebook  •  Regueiro")
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Variant") && text.contains("EPUB package"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Elio Handbook")));
    assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Language") && text.contains("en"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Publisher") && text.contains("Elio Docs"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Identifier") && text.contains("urn:uuid:elio-handbook"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_shows_contents_and_excerpt() {
    let root = temp_path("epub-excerpt");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("story.epub");
    write_zip_entries(
        &path,
        &[
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
                        <dc:title>Elio Story</dc:title>
                        <dc:creator>Regueiro</dc:creator>
                      </metadata>
                      <manifest>
                        <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                        <item id="chapter-1" href="text/chapter-1.xhtml" media-type="application/xhtml+xml"/>
                        <item id="chapter-2" href="text/chapter-2.xhtml" media-type="application/xhtml+xml"/>
                      </manifest>
                      <spine>
                        <itemref idref="chapter-1"/>
                        <itemref idref="chapter-2"/>
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
                            <li><a href="text/chapter-1.xhtml">Opening</a></li>
                            <li><a href="text/chapter-2.xhtml">Second Step</a></li>
                          </ol>
                        </nav>
                      </body>
                    </html>"#,
            ),
            (
                "OPS/text/chapter-1.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body>
                        <h1>Opening</h1>
                        <p>Elio begins with a small terminal window and a very opinionated file browser.</p>
                      </body>
                    </html>"#,
            ),
            (
                "OPS/text/chapter-2.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body>
                        <h2>Second Step</h2>
                        <p>The preview pane grows into an actual reading surface instead of stopping at metadata.</p>
                      </body>
                    </html>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path.clone()));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("EPUB ebook"));
    assert_eq!(preview.status_note.as_deref(), None);
    assert_eq!(preview.ebook_section_index, Some(0));
    assert_eq!(preview.ebook_section_count, Some(2));
    assert_eq!(preview.ebook_section_title.as_deref(), Some("Opening"));
    assert!(line_texts.iter().any(|text| {
        text.contains(
            "Elio begins with a small terminal window and a very opinionated file browser.",
        )
    }));

    let second_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::EpubSection(1),
    );
    let second_line_texts: Vec<_> = second_preview.lines.iter().map(line_text).collect();
    assert_eq!(second_preview.ebook_section_index, Some(1));
    assert_eq!(second_preview.ebook_section_count, Some(2));
    assert_eq!(
        second_preview.ebook_section_title.as_deref(),
        Some("Second Step")
    );
    assert!(second_line_texts.iter().any(|text| {
        text.contains(
            "The preview pane grows into an actual reading surface instead of stopping at metadata."
        )
    }));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_uses_doc_toc_role_and_normalizes_nested_labels() {
    let root = temp_path("epub-doc-toc-role");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("story.epub");
    write_zip_entries(
        &path,
        &[
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
                        <item id="chapter-1" href="text/chapter-1.xhtml" media-type="application/xhtml+xml"/>
                        <item id="chapter-2" href="text/chapter-2.xhtml" media-type="application/xhtml+xml"/>
                      </manifest>
                      <spine>
                        <itemref idref="chapter-1"/>
                        <itemref idref="chapter-2"/>
                      </spine>
                    </package>"#,
            ),
            (
                "OPS/nav.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body>
                        <nav role="doc-toc">
                          <ol>
                            <li>
                              <a href="text/chapter-1.xhtml">
                                <span>Opening</span>
                                <br />
                                Move
                              </a>
                            </li>
                            <li><a href="text/chapter-2.xhtml">Deep Dive</a></li>
                          </ol>
                        </nav>
                      </body>
                    </html>"#,
            ),
            (
                "OPS/text/chapter-1.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body>
                        <p>First chapter content.</p>
                      </body>
                    </html>"#,
            ),
            (
                "OPS/text/chapter-2.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body>
                        <p>Second chapter content.</p>
                      </body>
                    </html>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path.clone()));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    assert_eq!(preview.ebook_section_index, Some(0));
    assert_eq!(preview.ebook_section_count, Some(2));
    assert_eq!(preview.ebook_section_title.as_deref(), Some("Opening Move"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("First chapter content."))
    );

    let second_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::EpubSection(1),
    );
    let second_line_texts: Vec<_> = second_preview.lines.iter().map(line_text).collect();
    assert_eq!(
        second_preview.ebook_section_title.as_deref(),
        Some("Deep Dive")
    );
    assert!(
        second_line_texts
            .iter()
            .any(|text| text.contains("Second chapter content."))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_uses_ncx_toc_when_navigation_document_is_missing() {
    let root = temp_path("epub-ncx-toc");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("story.epub");
    write_zip_entries(
        &path,
        &[
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
                    <package xmlns="http://www.idpf.org/2007/opf" version="2.0">
                      <manifest>
                        <item id="toc" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
                        <item id="chapter-1" href="Text/chapter-1.xhtml" media-type="application/xhtml+xml"/>
                        <item id="chapter-2" href="Text/chapter-2.xhtml" media-type="application/xhtml+xml"/>
                      </manifest>
                      <spine toc="toc">
                        <itemref idref="chapter-1"/>
                        <itemref idref="chapter-2"/>
                      </spine>
                    </package>"#,
            ),
            (
                "OPS/toc.ncx",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
                      <navMap>
                        <navPoint id="chapter-1" playOrder="1">
                          <navLabel><text>Prelude</text></navLabel>
                          <content src="Text/chapter-1.xhtml"/>
                        </navPoint>
                        <navPoint id="chapter-2" playOrder="2">
                          <navLabel><text>Finale</text></navLabel>
                          <content src="Text/chapter-2.xhtml"/>
                        </navPoint>
                      </navMap>
                    </ncx>"#,
            ),
            (
                "OPS/Text/chapter-1.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body><p>Prelude text.</p></body>
                    </html>"#,
            ),
            (
                "OPS/Text/chapter-2.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body><p>Finale text.</p></body>
                    </html>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path.clone()));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    assert_eq!(preview.ebook_section_index, Some(0));
    assert_eq!(preview.ebook_section_count, Some(2));
    assert_eq!(preview.ebook_section_title.as_deref(), Some("Prelude"));
    assert!(line_texts.iter().any(|text| text.contains("Prelude text.")));

    let second_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::EpubSection(1),
    );
    assert_eq!(
        second_preview.ebook_section_title.as_deref(),
        Some("Finale")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_clamps_requested_section_and_falls_back_to_path_titles_without_toc() {
    let root = temp_path("epub-section-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("story.epub");
    write_zip_entries(
        &path,
        &[
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
                        <item id="chapter-1" href="text/Intro_Step.xhtml" media-type="application/xhtml+xml"/>
                        <item id="chapter-2" href="text/Final-Step.xhtml" media-type="application/xhtml+xml"/>
                      </manifest>
                      <spine>
                        <itemref idref="chapter-1"/>
                        <itemref idref="chapter-2"/>
                      </spine>
                    </package>"#,
            ),
            (
                "OPS/text/Intro_Step.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body><p>Intro text.</p></body>
                    </html>"#,
            ),
            (
                "OPS/text/Final-Step.xhtml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <html xmlns="http://www.w3.org/1999/xhtml">
                      <body><p>Final text.</p></body>
                    </html>"#,
            ),
        ],
    );

    let preview =
        build_preview_with_options(&file_entry(path), &PreviewRequestOptions::EpubSection(99));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.ebook_section_index, Some(1));
    assert_eq!(preview.ebook_section_count, Some(2));
    assert_eq!(preview.ebook_section_title.as_deref(), Some("Final Step"));
    assert!(line_texts.iter().any(|text| text.contains("Final text.")));
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn epub_preview_extracts_cover_image() {
    let root = temp_path("epub-cover");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("cover.epub");
    let source_cover = root.join("source-cover.png");
    write_test_raster_image(&source_cover, ImageFormat::Png, 160, 240);
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
                    <dc:title>Covered Story</dc:title>
                    <meta name="cover" content="cover-image"/>
                  </metadata>
                  <manifest>
                    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                    <item id="cover-image" href="images/cover.png" media-type="image/png"/>
                    <item id="chapter-1" href="text/chapter-1.xhtml" media-type="application/xhtml+xml"/>
                  </manifest>
                  <spine>
                    <itemref idref="chapter-1"/>
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
                        <li><a href="text/chapter-1.xhtml">Opening</a></li>
                      </ol>
                    </nav>
                  </body>
                </html>"#,
        ),
        (
            "OPS/text/chapter-1.xhtml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <p>The cover should be extracted for inline preview.</p>
                  </body>
                </html>"#,
        ),
    ] {
        zip.start_file(name, options)
            .expect("failed to start epub text entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write epub text entry");
    }
    zip.start_file("OPS/images/cover.png", options)
        .expect("failed to start cover entry");
    zip.write_all(&cover_bytes)
        .expect("failed to write cover entry");
    zip.finish().expect("failed to finish epub");

    let preview = build_preview(&file_entry(path));
    let visual = preview
        .preview_visual
        .clone()
        .expect("cover visual should be extracted");

    assert_eq!(visual.kind, PreviewVisualKind::Cover);
    assert_eq!(visual.layout, PreviewVisualLayout::Inline);
    assert!(visual.path.exists());
    assert!(visual.size > 0);
    assert_eq!(preview.ebook_section_index, Some(0));
    assert_eq!(preview.ebook_section_count, Some(1));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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
    assert!(line_texts.is_empty());
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

#[test]
fn epub_package_cache_reuses_parse_across_section_switches() {
    super::document::clear_epub_package_cache();

    let root = temp_path("epub-package-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("story.epub");
    super::document::reset_epub_package_parse_count(&path);
    write_zip_entries(
        &path,
        &[
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
                        <dc:title>Cached Story</dc:title>
                      </metadata>
                      <manifest>
                        <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                        <item id="chapter-1" href="text/chapter-1.xhtml" media-type="application/xhtml+xml"/>
                        <item id="chapter-2" href="text/chapter-2.xhtml" media-type="application/xhtml+xml"/>
                      </manifest>
                      <spine>
                        <itemref idref="chapter-1"/>
                        <itemref idref="chapter-2"/>
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
                            <li><a href="text/chapter-1.xhtml">Opening</a></li>
                            <li><a href="text/chapter-2.xhtml">Second Step</a></li>
                          </ol>
                        </nav>
                      </body>
                    </html>"#,
            ),
            (
                "OPS/text/chapter-1.xhtml",
                r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>One.</p></body></html>"#,
            ),
            (
                "OPS/text/chapter-2.xhtml",
                r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Two.</p></body></html>"#,
            ),
        ],
    );

    let _ = build_preview(&file_entry(path.clone()));
    let _ = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::EpubSection(1),
    );

    assert_eq!(super::document::epub_package_parse_count(&path), 1);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
