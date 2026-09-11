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
fn kindle_loading_preview_uses_empty_body() {
    for (file_name, detail) in [("novel.mobi", "MOBI ebook"), ("novel.azw3", "AZW3 ebook")] {
        let root = temp_path(&format!("kindle-loading-{}", file_name.replace('.', "-")));
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join(file_name);
        fs::write(&path, b"still-loading").expect("failed to write ebook fixture");

        let preview = loading_preview_for(&file_entry(path), &PreviewRequestOptions::Default);

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some(detail));
        assert!(preview.lines.is_empty());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
