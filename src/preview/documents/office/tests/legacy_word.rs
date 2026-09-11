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
