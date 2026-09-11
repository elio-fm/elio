use super::*;

#[test]
fn macho_preview_shows_dynamic_library_metadata() {
    let root = temp_path("macho-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("libelio.dylib");
    fs::write(&path, sample_macho_dylib_bytes()).expect("failed to write macho fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("Dynamic library"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("Mach-O"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Kind") && text.contains("Dynamic library"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Architecture") && text.contains("ARM64"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Load Commands") && text.contains("12"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn fat_macho_preview_lists_architectures_for_universal_binaries() {
    let root = temp_path("fat-macho-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("elio-universal");
    fs::write(&path, sample_macho_fat_bytes()).expect("failed to write fat macho fixture");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("Mach-O universal binary"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("Mach-O (fat)"))
    );
    assert!(line_texts.iter().any(|text| {
        text.contains("Architecture") && text.contains("x86") && text.contains("ARM64")
    }));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Sections") && text.contains("2"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
