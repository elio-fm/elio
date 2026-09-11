use super::*;

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
