use super::*;

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
