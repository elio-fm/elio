use super::*;

#[test]
fn doc_preview_shows_legacy_document_metadata() {
    let root = temp_path("doc");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("report.doc");
    write_doc_summary_information(
        &path,
        &[
            (2, DocTestPropertyValue::Text("Quarterly Report")),
            (4, DocTestPropertyValue::Text("Regueiro")),
            (12, DocTestPropertyValue::Timestamp(1_767_225_600)),
            (14, DocTestPropertyValue::Count(12)),
            (15, DocTestPropertyValue::Count(4_238)),
            (18, DocTestPropertyValue::Text("LibreOffice Writer")),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("DOC document"));
    assert_eq!(line_texts[0], "Document");
    assert!(line_texts.iter().any(|text| text == "People"));
    assert!(line_texts.iter().any(|text| text == "Dates"));
    assert!(line_texts.iter().any(|text| text == "Stats"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Quarterly Report"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("Jan 1, 2026 00:00 UTC"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Pages") && text.contains("12"))
    );
    assert!(line_texts.iter().any(|text| text.contains("4,238")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("LibreOffice Writer"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn docx_preview_shows_document_metadata() {
    let root = temp_path("docx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("report.docx");
    write_zip_entries(
        &path,
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

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("DOCX document"));
    assert_eq!(line_texts[0], "Document");
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("Format") || !text.contains("DOCX document"))
    );
    assert!(line_texts.iter().any(|text| text == "People"));
    assert!(line_texts.iter().any(|text| text == "Dates"));
    assert!(line_texts.iter().any(|text| text == "Stats"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Quarterly Report"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
    assert!(line_texts.iter().any(|text| text.contains("4,238")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("Mar 11, 2026 09:00 UTC"))
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("2026-03-11T09:00:00Z"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("LibreOffice"))
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("ApplicationLibreOffice"))
    );
    assert!(
        line_texts
            .iter()
            .position(|text| text == "Document")
            .unwrap()
            < line_texts.iter().position(|text| text == "People").unwrap()
    );
    assert!(
        line_texts.iter().position(|text| text == "People").unwrap()
            < line_texts.iter().position(|text| text == "Dates").unwrap()
    );
    assert!(
        line_texts.iter().position(|text| text == "Dates").unwrap()
            < line_texts.iter().position(|text| text == "Stats").unwrap()
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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
    assert_eq!(line_texts[0], "Document");
    assert!(line_texts.iter().any(|text| text == "People"));
    assert!(line_texts.iter().any(|text| text == "Dates"));
    assert!(line_texts.iter().any(|text| text == "Stats"));
    assert!(line_texts.iter().any(|text| text.contains("Project Notes")));
    assert!(line_texts.iter().any(|text| text.contains("LibreOffice")));
    assert!(line_texts.iter().any(|text| text.contains("980")));
    assert!(line_texts.iter().any(|text| text.contains("6,400")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("Mar 10, 2026 18:00 UTC"))
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("2026-03-10T18:00:00Z"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn pptx_preview_shows_presentation_metadata() {
    let root = temp_path("pptx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deck.pptx");
    write_zip_entries(
        &path,
        &[
            (
                "docProps/core.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/"
                        xmlns:dcterms="http://purl.org/dc/terms/">
                      <dc:title>Launch Deck</dc:title>
                      <dc:creator>Elio</dc:creator>
                      <dcterms:modified>2026-03-12T09:30:00Z</dcterms:modified>
                    </cp:coreProperties>"#,
            ),
            (
                "docProps/app.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>PowerPoint</Application>
                      <Slides>24</Slides>
                      <Notes>6</Notes>
                      <HiddenSlides>2</HiddenSlides>
                    </Properties>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("PPTX presentation"));
    assert!(line_texts.iter().any(|text| text.contains("Launch Deck")));
    assert!(line_texts.iter().any(|text| text.contains("PowerPoint")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Slides") && text.contains("24"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Notes") && text.contains("6"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Hidden Slides") && text.contains("2"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn xlsx_preview_shows_spreadsheet_metadata() {
    let root = temp_path("xlsx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("budget.xlsx");
    write_zip_entries(
        &path,
        &[
            (
                "docProps/core.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/">
                      <dc:title>Q2 Budget</dc:title>
                      <dc:creator>Finance Team</dc:creator>
                    </cp:coreProperties>"#,
            ),
            (
                "docProps/app.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>Excel</Application>
                      <Company>Elio Labs</Company>
                      <Manager>Regueiro</Manager>
                    </Properties>"#,
            ),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("XLSX spreadsheet"));
    assert!(line_texts.iter().any(|text| text.contains("Q2 Budget")));
    assert!(line_texts.iter().any(|text| text.contains("Finance Team")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("Excel"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Company") && text.contains("Elio Labs"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Manager") && text.contains("Regueiro"))
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
fn pages_preview_shows_document_metadata() {
    let root = temp_path("pages");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("design-review.pages");
    write_zip_entries(
        &path,
        &[
            (
                "Metadata/Properties.plist",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <plist version="1.0">
                      <dict>
                        <key>document-title</key>
                        <string>Design Review</string>
                        <key>kMDItemAuthors</key>
                        <array>
                          <string>Regueiro</string>
                          <string>Elio</string>
                        </array>
                        <key>creationDate</key>
                        <date>2026-03-10T18:00:00Z</date>
                        <key>modificationDate</key>
                        <date>2026-03-12T09:30:00Z</date>
                      </dict>
                    </plist>"#,
            ),
            ("Index/Document.iwa", "iwa"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("Pages document"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Variant") && text.contains("iWork package"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Design Review")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Regueiro, Elio"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("Mar 10, 2026 18:00 UTC"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Modified") && text.contains("Mar 12, 2026 09:30 UTC"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("Apple Pages"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
