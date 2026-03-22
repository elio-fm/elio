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

fn sample_pdf_bytes() -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Resources << >> /Contents 4 0 R >>"
            .to_string(),
        "<< /Length 0 >>\nstream\n\nendstream".to_string(),
        "<< /Title (Quarterly Report) /Author (Regueiro) /Creator (Elio) /Producer (Elio Test Suite) /CreationDate (D:20260311120000Z) /ModDate (D:20260311123000Z) >>".to_string(),
    ];

    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }

    let xref_offset = bytes.len();
    bytes.extend(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    bytes.extend(b"0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    bytes
}
