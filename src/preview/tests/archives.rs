use super::*;

#[test]
fn torrent_preview_shows_single_file_metadata_and_trackers() {
    let root = temp_path("torrent");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("sample.torrent");
    let bytes = bencode_dict(vec![
        ("announce", bencode_str("https://tracker.test")),
        (
            "announce-list",
            bencode_list(vec![bencode_list(vec![
                bencode_str("https://tracker.test"),
                bencode_str("https://backup.test"),
            ])]),
        ),
        ("comment", bencode_str("test torrent")),
        ("created by", bencode_str("elio")),
        (
            "info",
            bencode_dict(vec![
                ("length", bencode_int(12_345)),
                ("name", bencode_str("file.txt")),
                ("piece length", bencode_int(262_144)),
                ("pieces", bencode_bytes(b"12345678901234567890")),
                ("private", bencode_int(1)),
            ]),
        ),
    ]);
    fs::write(&path, bytes).expect("failed to write torrent");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("BitTorrent file"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Name") && text.contains("file.txt"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Mode") && text.contains("Single-file"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Private")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Trackers") && text.contains("2 across 1 tier"))
    );
    assert!(line_texts.iter().any(|text| text == "Trackers"));
    assert!(line_texts.iter().any(|text| {
        text.contains("Tier 1") && text.contains("tracker.test") && text.contains("backup.test")
    }));
    assert!(line_texts.iter().any(|text| text == "Contents"));
    assert!(line_texts.iter().any(|text| text.contains("file.txt")));
    assert!(!preview.truncated);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn torrent_preview_shows_multifile_contents_tree() {
    let root = temp_path("torrent-multifile");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("series.torrent");
    let bytes = bencode_dict(vec![
        (
            "announce-list",
            bencode_list(vec![
                bencode_list(vec![
                    bencode_str("https://tracker.one"),
                    bencode_str("https://tracker.two"),
                ]),
                bencode_list(vec![bencode_str("https://backup.tld/announce")]),
            ]),
        ),
        ("created by", bencode_str("elio")),
        (
            "info",
            bencode_dict(vec![
                (
                    "files",
                    bencode_list(vec![
                        bencode_dict(vec![
                            ("length", bencode_int(100)),
                            (
                                "path",
                                bencode_list(vec![
                                    bencode_str("season-01"),
                                    bencode_str("ep1.mkv"),
                                ]),
                            ),
                        ]),
                        bencode_dict(vec![
                            ("length", bencode_int(200)),
                            (
                                "path.utf-8",
                                bencode_list(vec![
                                    bencode_str("season-01"),
                                    bencode_str("ep2.mkv"),
                                ]),
                            ),
                        ]),
                    ]),
                ),
                ("name", bencode_str("series")),
                ("piece length", bencode_int(65_536)),
                (
                    "pieces",
                    bencode_bytes(b"1234567890123456789012345678901234567890"),
                ),
                ("private", bencode_int(0)),
            ]),
        ),
    ]);
    fs::write(&path, bytes).expect("failed to write torrent");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Text);
    assert_eq!(preview.detail.as_deref(), Some("BitTorrent file"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Mode") && text.contains("Multi-file"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Files") && text.contains("2"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Trackers") && text.contains("3 across 2 tiers"))
    );
    assert!(line_texts.iter().any(|text| text.contains("Tier 2")));
    assert!(line_texts.iter().any(|text| text.contains("series/")));
    assert!(line_texts.iter().any(|text| text.contains("season-01/")));
    assert!(line_texts.iter().any(|text| text.contains("ep1.mkv")));
    assert!(line_texts.iter().any(|text| text.contains("ep2.mkv")));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Privacy") && text.contains("Public"))
    );
    assert!(!preview.truncated);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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
    let metadata = container::parse_iso_metadata(&sample_iso_descriptors())
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
        Some(640 * container::ISO_SECTOR_SIZE as u64)
    );
    assert!(metadata.bootable);
}

#[test]
fn iso_entry_normalization_reconstructs_parents_and_strips_versions() {
    let entries = container::normalize_archive_entries(
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
    let preview = container::render_iso_preview(
        container::IsoMetadata {
            volume_id: Some("ELIO_INSTALL".to_string()),
            system_id: Some("ELIO_SYS".to_string()),
            total_size: Some(640 * container::ISO_SECTOR_SIZE as u64),
            bootable: true,
            created_at: Some("2026-03-11 09:00:00".to_string()),
            ..container::IsoMetadata::default()
        },
        container::normalize_archive_entries(
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
    let preview = container::render_iso_preview(
        container::IsoMetadata {
            volume_id: Some("BIG_IMAGE".to_string()),
            ..container::IsoMetadata::default()
        },
        container::normalize_archive_entries(items.iter().map(String::as_str), true),
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

#[test]
fn zip_preview_renders_archive_details_and_tree() {
    let root = temp_path("zip-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bundle.zip");
    write_zip_entries(
        &path,
        &[
            ("docs/readme.txt", "hello"),
            ("src/main.rs", "fn main() {}\n"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let header = preview
        .header_detail(0, 20)
        .expect("zip preview should expose header detail");

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("ZIP archive"));
    assert!(header.contains("ZIP archive"));
    assert!(line_texts.iter().any(|text| text.trim() == "Details"));
    assert!(!line_texts.iter().any(|text| text.trim() == "Archive"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Entries") && text.contains("4 total"))
    );
    assert!(line_texts.iter().any(|text| text.contains("docs/")));
    assert!(line_texts.iter().any(|text| text.contains("src/")));
    assert!(line_texts.iter().any(|text| text.contains("readme.txt")));
    assert!(line_texts.iter().any(|text| text.contains("main.rs")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_uses_comic_info_and_compact_contents() {
    let root = temp_path("comic-zip-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("issue.cbz");
    let source_cover = root.join("cover.jpg");
    write_test_raster_image(&source_cover, ImageFormat::Jpeg, 160, 240);
    let cover_bytes = fs::read(&source_cover).expect("failed to read cover image");

    let file = File::create(&path).expect("failed to create comic zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("001-cover.jpg", options)
        .expect("failed to start cover entry");
    zip.write_all(&cover_bytes)
        .expect("failed to write cover entry");
    zip.start_file("002-page.jpg", options)
        .expect("failed to start page entry");
    zip.write_all(&cover_bytes)
        .expect("failed to write page entry");
    zip.start_file("notes/readme.txt", options)
        .expect("failed to start text entry");
    zip.write_all(b"hello").expect("failed to write text entry");
    zip.start_file("ComicInfo.xml", options)
        .expect("failed to start comic info entry");
    zip.write_all(
        br#"<?xml version="1.0" encoding="utf-8"?>
            <ComicInfo>
              <Title>Bright Landing</Title>
              <Series>Orbital Stories</Series>
              <Number>4</Number>
              <Year>2026</Year>
              <Writer>Regueiro</Writer>
              <Publisher>Elio Press</Publisher>
              <Genre>Science Fiction</Genre>
            </ComicInfo>"#,
    )
    .expect("failed to write comic info entry");
    zip.finish().expect("failed to finish comic zip");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic zip should expose a page visual");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic ZIP archive"));
    assert_eq!(visual.kind, PreviewVisualKind::PageImage);
    assert_eq!(visual.layout, PreviewVisualLayout::FullHeight);
    let position = preview
        .navigation_position
        .as_ref()
        .expect("comic zip should expose page navigation");
    assert_eq!(position.label, "Page");
    assert_eq!(position.index, 0);
    assert_eq!(position.count, 2);
    assert!(visual.path.exists());
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Title") && text.contains("Bright Landing"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("Orbital Stories"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Number") && text.contains("4"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Writer") && text.contains("Regueiro"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Publisher") && text.contains("Elio Press"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Genre") && text.contains("Science Fiction"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Pages") && text.contains("2"))
    );
    assert!(line_texts.iter().any(|text| text.trim() == "Contents"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Extras") && text.contains("ComicInfo.xml"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Extras") && text.contains("notes/readme.txt"))
    );
    assert!(
        !line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("ZIP"))
    );
    assert!(!line_texts.iter().any(|text| text.contains("Packed")));
    assert!(!line_texts.iter().any(|text| text.contains("Archive Size")));
    assert!(!line_texts.iter().any(|text| text.contains("001-cover.jpg")));
    assert!(!line_texts.iter().any(|text| text.contains("002-page.jpg")));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_uses_natural_page_order_and_page_selection() {
    let root = temp_path("comic-zip-pages");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("issue.cbz");
    write_zip_binary_entries(
        &path,
        &[
            ("10.jpg", b"page-ten"),
            ("2.jpg", b"page-two"),
            ("1.jpg", b"page-one"),
        ],
    );

    let first_preview = build_preview(&file_entry(path.clone()));
    let first_visual = first_preview
        .preview_visual
        .as_ref()
        .expect("first page should be extracted");
    let second_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::ComicPage(1),
    );
    let second_visual = second_preview
        .preview_visual
        .as_ref()
        .expect("second page should be extracted");
    let third_preview = build_preview_with_options(
        &file_entry(path.clone()),
        &PreviewRequestOptions::ComicPage(2),
    );
    let third_visual = third_preview
        .preview_visual
        .as_ref()
        .expect("third page should be extracted");

    assert_eq!(
        fs::read(&first_visual.path).expect("failed to read first page"),
        b"page-one"
    );
    assert_eq!(
        fs::read(&second_visual.path).expect("failed to read second page"),
        b"page-two"
    );
    assert_eq!(
        fs::read(&third_visual.path).expect("failed to read third page"),
        b"page-ten"
    );
    assert_eq!(
        second_preview
            .navigation_position
            .as_ref()
            .map(|position| position.index),
        Some(1)
    );
    assert_eq!(
        third_preview
            .navigation_position
            .as_ref()
            .map(|position| position.count),
        Some(3)
    );
    let second_line_texts: Vec<_> = second_preview.lines.iter().map(line_text).collect();
    assert!(
        second_line_texts
            .iter()
            .any(|text| text.contains("Pages") && text.contains("3"))
    );
    assert!(!second_line_texts.iter().any(|text| text.contains("2.jpg")));

    let _ = fs::remove_file(first_visual.path.clone());
    let _ = fs::remove_file(second_visual.path.clone());
    let _ = fs::remove_file(third_visual.path.clone());
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cbr_file_with_zip_content_renders_as_comic_preview() {
    let root = temp_path("comic-rar-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("issue.cbr");
    let source_cover = root.join("cover.jpg");
    write_test_raster_image(&source_cover, ImageFormat::Jpeg, 160, 240);
    let cover_bytes = fs::read(&source_cover).expect("failed to read cover image");
    write_zip_binary_entries(
        &path,
        &[
            ("001-cover.jpg", &cover_bytes),
            ("002-page.jpg", &cover_bytes),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic rar should expose a page visual");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic RAR archive"));
    assert_eq!(visual.kind, PreviewVisualKind::PageImage);
    assert_eq!(visual.layout, PreviewVisualLayout::FullHeight);
    assert_eq!(
        preview.navigation_position.as_ref().map(|position| (
            position.label,
            position.index,
            position.count
        )),
        Some(("Page", 0, 2))
    );
    assert!(visual.path.exists());
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Pages") && text.contains("2"))
    );

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tar_preview_lists_inner_archive_contents() {
    let root = temp_path("tar-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bundle.tar");
    write_tar_entries(
        &path,
        &[
            ("docs/readme.txt", "hello"),
            ("src/main.rs", "fn main() {}\n"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("TAR archive"));
    assert!(line_texts.iter().any(|text| text.contains("docs/")));
    assert!(line_texts.iter().any(|text| text.contains("src/")));
    assert!(line_texts.iter().any(|text| text.contains("readme.txt")));
    assert!(line_texts.iter().any(|text| text.contains("main.rs")));
    assert!(!line_texts.iter().any(|text| text.contains("bundle.tar")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tar_gz_preview_lists_inner_archive_contents() {
    let root = temp_path("tar-gz-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bundle.tar.gz");
    write_tar_gz_entries(
        &path,
        &[
            ("docs/readme.txt", "hello"),
            ("src/main.rs", "fn main() {}\n"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("TAR.GZ archive"));
    assert!(line_texts.iter().any(|text| text.contains("docs/")));
    assert!(line_texts.iter().any(|text| text.contains("src/")));
    assert!(line_texts.iter().any(|text| text.contains("readme.txt")));
    assert!(line_texts.iter().any(|text| text.contains("main.rs")));
    assert!(!line_texts.iter().any(|text| text.contains("bundle.tar")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tgz_preview_keeps_tar_gz_label_and_contents_tree() {
    let root = temp_path("tgz-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bundle.tgz");
    write_tar_gz_entries(
        &path,
        &[("assets/logo.txt", "logo"), ("bin/elio", "#!/bin/sh\n")],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("TAR.GZ archive"));
    assert!(line_texts.iter().any(|text| text.contains("assets/")));
    assert!(line_texts.iter().any(|text| text.contains("bin/")));
    assert!(line_texts.iter().any(|text| text.contains("logo.txt")));
    assert!(line_texts.iter().any(|text| text.contains("elio")));
    assert!(!line_texts.iter().any(|text| text.contains("bundle.tar")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn raw_xz_preview_uses_compressed_disk_image_label() {
    let root = temp_path("raw-xz-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("fedora.aarch64.raw.xz");
    if !write_xz_compressed_file(&path, b"raw-disk-image") {
        fs::remove_dir_all(root).expect("failed to remove temp root");
        return;
    }

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    // On systems without 7z or bsdtar support for raw XZ images, the preview
    // falls back to Binary. Skip the remaining assertions in that case.
    if preview.kind == PreviewKind::Binary {
        fs::remove_dir_all(root).expect("failed to remove temp root");
        return;
    }

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(
        preview.detail.as_deref(),
        Some("XZ-compressed raw disk image")
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Format") && (text.contains("XZ") || text.contains("xz")))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("fedora.aarch64.raw"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
