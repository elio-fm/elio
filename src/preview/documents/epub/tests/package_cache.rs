use super::*;

#[test]
fn epub_package_cache_reuses_parse_across_section_switches() {
    crate::preview::documents::clear_epub_package_cache();

    let root = temp_path("epub-package-cache");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("story.epub");
    crate::preview::documents::reset_epub_package_parse_count(&path);
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

    assert_eq!(
        crate::preview::documents::epub_package_parse_count(&path),
        1
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
