use super::*;

#[test]
fn iso_binary_preview_keeps_specific_type_detail() {
    let root = temp_path("iso");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("disk.iso");
    fs::write(&path, [0x00, 0x81, 0xFE, 0xFF]).expect("failed to write iso");

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Binary);
    assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn iso_metadata_parser_reads_primary_volume_descriptor() {
    let metadata = archive_preview::parse_iso_metadata(&sample_iso_descriptors())
        .expect("sample descriptors should parse");

    assert_eq!(metadata.system_id.as_deref(), Some("ELIO_SYS"));
    assert_eq!(metadata.volume_id.as_deref(), Some("ELIO_INSTALL"));
    assert_eq!(metadata.publisher_id.as_deref(), Some("Elio Publisher"));
    assert_eq!(metadata.preparer_id.as_deref(), Some("Elio Builder"));
    assert_eq!(metadata.application_id.as_deref(), Some("Elio Image Tool"));
    assert_eq!(metadata.created_at.as_deref(), Some("2026-03-11 09:00:00"));
    assert_eq!(metadata.modified_at.as_deref(), Some("2026-03-11 10:15:00"));
    assert_eq!(
        metadata.effective_at.as_deref(),
        Some("2026-03-12 00:00:00")
    );
    assert_eq!(
        metadata.total_size,
        Some(640 * archive_preview::ISO_SECTOR_SIZE as u64)
    );
    assert!(metadata.bootable);
}

#[test]
fn iso_entry_normalization_reconstructs_parents_and_strips_versions() {
    let entries = archive_preview::normalize_archive_entries(
        ["/docs/readme.txt;1", "./EFI/BOOT/", "boot.catalog;1"],
        true,
    );

    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "docs" && entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "docs/readme.txt" && !entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "EFI" && entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "EFI/BOOT" && entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "boot.catalog" && !entry.is_dir)
    );
}

#[test]
fn iso_preview_renders_metadata_and_tree() {
    let preview = archive_preview::render_iso_preview(
        archive_preview::IsoMetadata {
            volume_id: Some("ELIO_INSTALL".to_string()),
            system_id: Some("ELIO_SYS".to_string()),
            total_size: Some(640 * archive_preview::ISO_SECTOR_SIZE as u64),
            bootable: true,
            created_at: Some("2026-03-11 09:00:00".to_string()),
            ..archive_preview::IsoMetadata::default()
        },
        archive_preview::normalize_archive_entries(
            ["boot/", "boot/grub/", "boot/grub/grub.cfg", "README.txt"],
            true,
        ),
    );
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let header = preview
        .header_detail(0, 20)
        .expect("iso preview should expose header detail");

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));
    assert!(header.contains("ISO disk image"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("ELIO_INSTALL"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text == "Contents" || text.ends_with("Contents"))
    );
    assert!(line_texts.iter().any(|text| text.contains("boot/")));
    assert!(line_texts.iter().any(|text| text.contains("grub.cfg")));
    assert!(line_texts.iter().any(|text| text.contains("README.txt")));
}

#[test]
fn iso_preview_reports_tree_truncation() {
    let items = (0..(PREVIEW_RENDER_LINE_LIMIT + 80))
        .map(|index| format!("dir/file-{index:03}.txt"))
        .collect::<Vec<_>>();
    let preview = archive_preview::render_iso_preview(
        archive_preview::IsoMetadata {
            volume_id: Some("BIG_IMAGE".to_string()),
            ..archive_preview::IsoMetadata::default()
        },
        archive_preview::normalize_archive_entries(items.iter().map(String::as_str), true),
    );
    let header = preview
        .header_detail(0, 20)
        .expect("iso preview header should include truncation");

    assert!(preview.truncated);
    assert!(header.contains("showing first"));
}

#[test]
fn iso_preview_lists_contents_when_bsdtar_can_read_image() {
    let root = temp_path("iso-listing");
    let image_root = root.join("image-root");
    fs::create_dir_all(image_root.join("docs")).expect("failed to create image tree");
    fs::write(image_root.join("docs/readme.txt"), "hello").expect("failed to write image file");
    let path = root.join("sample.iso");

    let created = Command::new("bsdtar")
        .arg("-cf")
        .arg(&path)
        .arg("-C")
        .arg(&image_root)
        .arg(".")
        .status();
    if !created.as_ref().is_ok_and(|status| status.success()) {
        fs::remove_dir_all(root).expect("failed to remove temp root");
        return;
    }

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("ISO disk image"));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("docs/"))
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("readme.txt"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
