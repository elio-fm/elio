use super::*;

#[test]
fn pdf_preview_shows_pdfinfo_metadata() {
    if Command::new("pdfinfo").arg("-v").output().is_err() {
        return;
    }

    let root = temp_path("pdf");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("report.pdf");
    fs::write(&path, sample_pdf_bytes()).expect("failed to write pdf fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("PDF document"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Variant") && text.contains("PDF 1.4"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Quarterly Report"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Regueiro")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("Elio"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Pages") && text.contains("1"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Producer") && text.contains("Elio Test Suite"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
