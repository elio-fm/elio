use super::*;

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
