use super::*;

#[test]
fn pe_preview_shows_windows_executable_metadata() {
    let root = temp_path("pe-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("setup.exe");
    fs::write(&path, sample_pe_exe_bytes()).expect("failed to write pe fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("Windows executable"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("PE/COFF"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Architecture") && text.contains("x86_64"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Bits") && text.contains("64-bit"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Subsystem") && text.contains("Console"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Entry Point") && text.contains("0x1230"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn dos_mz_preview_falls_back_to_legacy_executable_metadata() {
    let root = temp_path("dos-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("legacy.bin");
    fs::write(&path, sample_dos_mz_bytes()).expect("failed to write dos fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("DOS executable"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("MZ"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Bits") && text.contains("16-bit"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
