use super::*;

#[test]
fn odt_preview_shows_document_metadata() {
    let root = temp_path("odt");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("report.odt");
    write_zip_entries(
        &path,
        &[(
            "meta.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                    xmlns:dc="http://purl.org/dc/elements/1.1/"
                    xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
                  <office:meta>
                    <dc:title>Project Notes</dc:title>
                    <meta:initial-creator>Elio</meta:initial-creator>
                    <meta:creation-date>2026-03-10T18:00:00Z</meta:creation-date>
                    <meta:generator>LibreOffice</meta:generator>
                    <meta:document-statistic meta:page-count="3" meta:word-count="980" meta:character-count="6400"/>
                  </office:meta>
                </office:document-meta>"#,
        )],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("ODT document"));
    assert_eq!(line_texts[0], "Details");
    assert!(line_texts.iter().all(|text| text != "People"));
    assert!(line_texts.iter().all(|text| text != "Dates"));
    assert!(line_texts.iter().all(|text| text != "Stats"));
    assert!(line_texts.iter().any(|text| text.contains("Project Notes")));
    assert!(line_texts.iter().any(|text| text.contains("LibreOffice")));
    assert!(line_texts.iter().any(|text| text.contains("980")));
    assert!(line_texts.iter().any(|text| text.contains("6,400")));
    // The exact time and offset label depend on the local timezone; check that
    // the date is shown in a human-readable form, not as raw "2026-03-10T18:00:00Z".
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("2026"))
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("2026-03-10T18:00:00Z"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn ods_preview_shows_spreadsheet_statistics() {
    let root = temp_path("ods");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("budget.ods");
    write_zip_entries(
        &path,
        &[(
            "meta.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                    xmlns:dc="http://purl.org/dc/elements/1.1/"
                    xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
                  <office:meta>
                    <dc:title>Operations Budget</dc:title>
                    <meta:initial-creator>Elio</meta:initial-creator>
                    <meta:generator>LibreOffice Calc</meta:generator>
                    <meta:document-statistic meta:table-count="4" meta:cell-count="512" meta:object-count="2"/>
                  </office:meta>
                </office:document-meta>"#,
        )],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("ODS spreadsheet"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Operations Budget"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("LibreOffice Calc"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Tables") && text.contains("4"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Cells") && text.contains("512"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Objects") && text.contains("2"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn odp_preview_shows_presentation_statistics() {
    let root = temp_path("odp");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deck.odp");
    write_zip_entries(
        &path,
        &[(
            "meta.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                    xmlns:dc="http://purl.org/dc/elements/1.1/"
                    xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
                  <office:meta>
                    <dc:title>Launch Deck</dc:title>
                    <meta:initial-creator>Elio</meta:initial-creator>
                    <meta:generator>LibreOffice Impress</meta:generator>
                    <meta:document-statistic meta:page-count="14" meta:object-count="3"/>
                  </office:meta>
                </office:document-meta>"#,
        )],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("ODP presentation"));
    assert!(line_texts.iter().any(|text| text.contains("Launch Deck")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("LibreOffice Impress"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Slides") && text.contains("14"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Objects") && text.contains("3"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn large_odp_preview_reads_metadata_from_full_zip_archive() {
    let root = temp_path("large-odp");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("large-deck.odp");
    let filler = vec![b'x'; 600 * 1024];
    let meta_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
            xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0">
          <office:meta>
            <dc:title>Large Impress Deck</dc:title>
            <meta:generator>LibreOffice Impress</meta:generator>
            <meta:document-statistic meta:page-count="18" meta:object-count="5"/>
          </office:meta>
        </office:document-meta>"#;
    write_zip_binary_entries(
        &path,
        &[
            ("Pictures/image1.bin", filler.as_slice()),
            ("meta.xml", meta_xml.as_bytes()),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("ODP presentation"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Large Impress Deck"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Slides") && text.contains("18"))
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("No document metadata available"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
