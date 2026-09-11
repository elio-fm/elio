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
