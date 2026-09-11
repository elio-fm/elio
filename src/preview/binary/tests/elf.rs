use super::*;

#[test]
fn elf_preview_detects_binaries_without_extension() {
    let root = temp_path("elf-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("app-bin");
    fs::write(&path, sample_elf_shared_object_bytes()).expect("failed to write elf fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("ELF shared object"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Architecture") && text.contains("AArch64"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("ABI") && text.contains("Linux"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Entry Point") && text.contains("0x401000"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Sections") && text.contains("18"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
