use crate::{
    filesystem::{Entry, EntryKind},
    preview::{PreviewKind, build_preview},
};
use ratatui::text::Line;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

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
            .any(|text| text.contains("Pages") && text.contains('1'))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Producer") && text.contains("Elio Test Suite"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-preview-{label}-{unique}"))
}

fn file_entry(path: PathBuf) -> Entry {
    Entry {
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        name_key: path.file_name().unwrap().to_string_lossy().to_lowercase(),
        path,
        kind: EntryKind::File,
        symlink: None,
        size: 0,
        modified: None,
        readonly: false,
    }
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
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
