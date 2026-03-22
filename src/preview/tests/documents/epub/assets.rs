use super::*;

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
