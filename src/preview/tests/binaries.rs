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

#[test]
fn jar_preview_surfaces_manifest_metadata() {
    let root = temp_path("jar-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("app.jar");
    write_zip_entries(
        &path,
        &[
            (
                "META-INF/MANIFEST.MF",
                "Implementation-Title: Elio\nImplementation-Version: 1.2.3\nMain-Class: elio.Main\nCreated-By: OpenJDK\n",
            ),
            ("elio/Main.class", "compiled"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("Java archive"));
    assert!(line_texts.iter().any(|text| text == "Manifest"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Title") && text.contains("Elio"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Version") && text.contains("1.2.3"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Main-Class") && text.contains("elio.Main"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
