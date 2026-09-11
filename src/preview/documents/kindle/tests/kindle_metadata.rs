use super::*;

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
    assert!(line_texts.iter().all(|text| text != "Metadata"));
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
