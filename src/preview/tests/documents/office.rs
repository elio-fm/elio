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
    assert_eq!(line_texts[0], "Details");
    assert!(line_texts.iter().all(|text| text != "People"));
    assert!(line_texts.iter().all(|text| text != "Dates"));
    assert!(line_texts.iter().all(|text| text != "Stats"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Quarterly Report"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
    // The exact time and offset label depend on the local timezone; check only
    // that the date is shown in a human-readable form (not raw FILETIME or ISO).
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("2026"))
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
    assert_eq!(line_texts[0], "Details");
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("Format") || !text.contains("DOCX document"))
    );
    assert!(line_texts.iter().all(|text| text != "People"));
    assert!(line_texts.iter().all(|text| text != "Dates"));
    assert!(line_texts.iter().all(|text| text != "Stats"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Quarterly Report"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
    assert!(line_texts.iter().any(|text| text.contains("4,238")));
    // The exact time and offset label depend on the local timezone; check that
    // the date is shown in a human-readable form, not as raw "2026-03-11T09:00:00Z".
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("2026"))
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
    assert_eq!(
        line_texts.iter().filter(|text| *text == "Details").count(),
        1
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
fn pptx_preview_with_no_people_metadata_does_not_show_people_section() {
    let root = temp_path("pptx-no-people");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("deck.pptx");
    write_zip_entries(
        &path,
        &[
            (
                "docProps/core.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dcterms="http://purl.org/dc/terms/">
                      <dcterms:created>2006-08-16T00:00:00Z</dcterms:created>
                      <dcterms:modified>2011-08-01T06:04:30Z</dcterms:modified>
                    </cp:coreProperties>"#,
            ),
            (
                "docProps/app.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>Microsoft Office PowerPoint</Application>
                      <Slides>0</Slides>
                    </Properties>"#,
            ),
            ("ppt/slides/slide1.xml", r#"<?xml version="1.0"?><p:sld/>"#),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("PPTX presentation"));
    assert!(line_texts.iter().any(|text| text == "Details"));
    assert!(line_texts.iter().all(|text| text != "People"));
    assert!(line_texts.iter().all(|text| text != "Dates"));
    assert!(line_texts.iter().all(|text| text != "Stats"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("Microsoft Office"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Slides") && text.contains("1"))
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
fn large_pptx_preview_reads_metadata_from_full_zip_archive() {
    let root = temp_path("large-pptx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("large-deck.pptx");
    let core_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
            xmlns:dc="http://purl.org/dc/elements/1.1/">
          <dc:title>Large Launch Deck</dc:title>
          <dc:creator>Elio</dc:creator>
        </cp:coreProperties>"#;
    let app_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
          <Application>PowerPoint</Application>
          <Slides>0</Slides>
          <Notes>0</Notes>
        </Properties>"#;
    write_large_pptx_with_stale_counts(&path, core_xml, app_xml, 13, 2);

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("PPTX presentation"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Large Launch Deck"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Slides") && text.contains("13"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Notes") && text.contains("2"))
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("No document metadata available"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

fn write_large_pptx_with_stale_counts(
    path: &std::path::Path,
    core_xml: &str,
    app_xml: &str,
    slides: u16,
    notes: u16,
) {
    let file = File::create(path).expect("failed to create pptx");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("ppt/media/image1.bin", options)
        .expect("failed to start media entry");
    zip.write_all(&vec![b'x'; 600 * 1024])
        .expect("failed to write media entry");

    for slide in 1..=slides {
        zip.start_file(format!("ppt/slides/slide{slide}.xml"), options)
            .expect("failed to start slide entry");
        zip.write_all(br#"<?xml version="1.0"?><p:sld/>"#)
            .expect("failed to write slide entry");
    }

    for note in 1..=notes {
        zip.start_file(format!("ppt/notesSlides/notesSlide{note}.xml"), options)
            .expect("failed to start notes entry");
        zip.write_all(br#"<?xml version="1.0"?><p:notes/>"#)
            .expect("failed to write notes entry");
    }

    zip.start_file("docProps/core.xml", options)
        .expect("failed to start core metadata entry");
    zip.write_all(core_xml.as_bytes())
        .expect("failed to write core metadata");
    zip.start_file("docProps/app.xml", options)
        .expect("failed to start app metadata entry");
    zip.write_all(app_xml.as_bytes())
        .expect("failed to write app metadata");
    zip.finish().expect("failed to finish pptx");
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
    // The exact time and offset label depend on the local timezone; check that
    // each date is shown in a human-readable form rather than raw ISO 8601.
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Created") && text.contains("2026"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Modified") && text.contains("2026"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("Apple Pages"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

enum DocTestPropertyValue<'a> {
    Count(u32),
    Text(&'a str),
    Timestamp(u64),
}

fn write_doc_summary_information(
    path: &std::path::Path,
    properties: &[(u32, DocTestPropertyValue<'_>)],
) {
    const DOC_SUMMARY_INFORMATION_STREAM: &str = "/\u{5}SummaryInformation";
    const VT_LPWSTR: u16 = 0x001F;
    const VT_FILETIME: u16 = 0x0040;
    const VT_UI4: u16 = 0x0013;

    fn encode_doc_property_value(value: &DocTestPropertyValue<'_>) -> Vec<u8> {
        match value {
            DocTestPropertyValue::Count(count) => {
                let mut bytes = Vec::with_capacity(8);
                bytes.extend_from_slice(&VT_UI4.to_le_bytes());
                bytes.extend_from_slice(&0u16.to_le_bytes());
                bytes.extend_from_slice(&count.to_le_bytes());
                bytes
            }
            DocTestPropertyValue::Text(text) => {
                let mut bytes = Vec::new();
                let mut units = text.encode_utf16().collect::<Vec<_>>();
                units.push(0);
                bytes.extend_from_slice(&VT_LPWSTR.to_le_bytes());
                bytes.extend_from_slice(&0u16.to_le_bytes());
                bytes.extend_from_slice(&(units.len() as u32).to_le_bytes());
                for unit in units {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                bytes
            }
            DocTestPropertyValue::Timestamp(unix_seconds) => {
                const WINDOWS_TICKS_PER_SECOND: u64 = 10_000_000;
                const WINDOWS_TO_UNIX_EPOCH_SECONDS: u64 = 11_644_473_600;

                let filetime =
                    (unix_seconds + WINDOWS_TO_UNIX_EPOCH_SECONDS) * WINDOWS_TICKS_PER_SECOND;
                let mut bytes = Vec::with_capacity(12);
                bytes.extend_from_slice(&VT_FILETIME.to_le_bytes());
                bytes.extend_from_slice(&0u16.to_le_bytes());
                bytes.extend_from_slice(&filetime.to_le_bytes());
                bytes
            }
        }
    }

    let section_offset = 48usize;
    let table_len = 8 + properties.len() * 8;
    let mut section = vec![0; table_len];
    section[4..8].copy_from_slice(&(properties.len() as u32).to_le_bytes());
    let mut values = Vec::new();

    for (index, (property_id, value)) in properties.iter().enumerate() {
        let encoded = encode_doc_property_value(value);
        let entry_offset = 8 + index * 8;
        section[entry_offset..entry_offset + 4].copy_from_slice(&property_id.to_le_bytes());
        section[entry_offset + 4..entry_offset + 8]
            .copy_from_slice(&((table_len + values.len()) as u32).to_le_bytes());
        values.extend_from_slice(&encoded);
    }

    let mut bytes = vec![0; section_offset];
    bytes[28..32].copy_from_slice(&1u32.to_le_bytes());
    bytes[44..48].copy_from_slice(&(section_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&section);
    bytes.extend_from_slice(&values);

    let mut compound = cfb::create(path).expect("failed to create compound document");
    let mut stream = compound
        .create_stream(DOC_SUMMARY_INFORMATION_STREAM)
        .expect("failed to create summary information stream");
    stream
        .write_all(&bytes)
        .expect("failed to write summary information stream");
}
