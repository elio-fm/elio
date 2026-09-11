use super::*;

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
