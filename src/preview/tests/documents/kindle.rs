use super::*;

#[test]
fn mobi_and_azw3_previews_use_document_headers() {
    for (file_name, detail) in [("novel.mobi", "MOBI ebook"), ("novel.azw3", "AZW3 ebook")] {
        let root = temp_path(&format!("kindle-{}", file_name.replace('.', "-")));
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join(file_name);
        fs::write(&path, b"synthetic kindle ebook bytes").expect("failed to write ebook fixture");

        let preview = build_preview(&file_entry(path));
        let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.section_label(), "Document");
        assert_eq!(preview.detail.as_deref(), Some(detail));
        assert!(
            line_texts
                .iter()
                .any(|text| text.contains("No document metadata available"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn mobi_preview_reads_exth_metadata() {
    let root = temp_path("mobi-exth-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("handbook.mobi");
    write_synthetic_kindle(
        &path,
        "Fallback Handbook",
        &[
            (100, b"Avery Quill".as_slice()),
            (100, b"Morgan Line".as_slice()),
            (101, b"Elio Press".as_slice()),
            (103, b"Synthetic metadata fixture".as_slice()),
            (105, b"Reference".as_slice()),
            (106, b"2026-03-12T08:00:00Z".as_slice()),
            (113, b"B012345678".as_slice()),
            (204, &201_u32.to_be_bytes()),
            (205, &2_u32.to_be_bytes()),
            (206, &9_u32.to_be_bytes()),
            (207, &1029_u32.to_be_bytes()),
            (501, b"EBOK".as_slice()),
            (503, b"Signal Handbook".as_slice()),
            (524, b"en".as_slice()),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("MOBI ebook"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Title") && text.contains("Signal Handbook"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Author") && text.contains("Avery Quill, Morgan Line"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Subject") && text.contains("Reference"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Application") && text.contains("KindleGen 2.9 build 1029"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Publisher") && text.contains("Elio Press"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Language") && text.contains("en"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Published") && text.contains("2026"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("ASIN") && text.contains("B012345678"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Kindle Type") && text.contains("EBOK"))
    );
    assert!(line_texts.iter().all(|text| !text.contains("Binary")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn azw3_preview_uses_mobi_full_name_when_exth_title_is_missing() {
    let root = temp_path("azw3-full-name-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("handbook.azw3");
    write_synthetic_kindle(
        &path,
        "Fallback Handbook",
        &[(100, b"Avery Quill".as_slice()), (524, b"en".as_slice())],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(preview.detail.as_deref(), Some("AZW3 ebook"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Title") && text.contains("Fallback Handbook"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Author") && text.contains("Avery Quill"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Language") && text.contains("en"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

fn write_synthetic_kindle(path: &std::path::Path, full_name: &str, exth_records: &[(u32, &[u8])]) {
    let mut record0 = vec![0_u8; 16];
    write_be_u16(&mut record0, 0, 2);
    write_be_u32(&mut record0, 4, 12_345);
    write_be_u16(&mut record0, 8, 2);
    write_be_u16(&mut record0, 10, 4096);

    let mut mobi_header = vec![0_u8; 256];
    mobi_header[0..4].copy_from_slice(b"MOBI");
    write_be_u32(&mut mobi_header, 4, 256);
    write_be_u32(&mut mobi_header, 8, 2);
    write_be_u32(&mut mobi_header, 12, 65_001);
    write_be_u32(&mut mobi_header, 20, 6);

    let exth = build_exth(exth_records);
    let full_name_offset = 16 + mobi_header.len() + exth.len();
    write_be_u32(&mut mobi_header, 68, full_name_offset as u32);
    write_be_u32(&mut mobi_header, 72, full_name.len() as u32);
    if !exth_records.is_empty() {
        write_be_u32(&mut mobi_header, 112, 0x40);
    }

    record0.extend_from_slice(&mobi_header);
    record0.extend_from_slice(&exth);
    record0.extend_from_slice(full_name.as_bytes());
    record0.extend_from_slice(&[0, 0]);
    while !record0.len().is_multiple_of(4) {
        record0.push(0);
    }

    let record_count = 2_u16;
    let record0_offset = 78 + usize::from(record_count) * 8;
    let record1_offset = record0_offset + record0.len();
    let mut bytes = vec![0_u8; 78];
    bytes[..13].copy_from_slice(b"Fixture Book\0");
    bytes[60..64].copy_from_slice(b"BOOK");
    bytes[64..68].copy_from_slice(b"MOBI");
    write_be_u16(&mut bytes, 76, record_count);
    write_record_entry(&mut bytes, record0_offset as u32);
    write_record_entry(&mut bytes, record1_offset as u32);
    bytes.extend_from_slice(&record0);
    bytes.extend_from_slice(b"EOF");

    fs::write(path, bytes).expect("failed to write synthetic kindle file");
}

fn build_exth(records: &[(u32, &[u8])]) -> Vec<u8> {
    if records.is_empty() {
        return Vec::new();
    }

    let mut exth = Vec::new();
    exth.extend_from_slice(b"EXTH");
    exth.extend_from_slice(&[0; 4]);
    exth.extend_from_slice(&(records.len() as u32).to_be_bytes());
    for (kind, value) in records {
        exth.extend_from_slice(&kind.to_be_bytes());
        exth.extend_from_slice(&((8 + value.len()) as u32).to_be_bytes());
        exth.extend_from_slice(value);
    }
    let exth_len = exth.len() as u32;
    exth[4..8].copy_from_slice(&exth_len.to_be_bytes());
    while !exth.len().is_multiple_of(4) {
        exth.push(0);
    }
    exth
}

fn write_record_entry(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&offset.to_be_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 0]);
}

fn write_be_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_be_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}
