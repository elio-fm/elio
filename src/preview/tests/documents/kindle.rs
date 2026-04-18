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
fn mobi_and_azw3_previews_attach_exth_cover_visual() {
    for (file_name, detail) in [
        ("covered.mobi", "MOBI ebook"),
        ("covered.azw3", "AZW3 ebook"),
    ] {
        let root = temp_path(&format!("kindle-cover-{}", file_name.replace('.', "-")));
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join(file_name);
        let cover_bytes = raster_image_bytes(&root, ImageFormat::Png, 24, 36);
        let cover_offset = 0_u32.to_be_bytes();
        write_synthetic_kindle_with_resources(
            &path,
            "Covered Handbook",
            &[
                (201, cover_offset.as_slice()),
                (503, b"Covered Handbook".as_slice()),
            ],
            &[cover_bytes.as_slice()],
        );

        let preview = build_preview(&file_entry(path));
        let visual = preview
            .preview_visual
            .clone()
            .expect("kindle preview should attach the cover image");
        let dimensions = image::ImageReader::open(&visual.path)
            .expect("cover cache should open")
            .with_guessed_format()
            .expect("cover cache format should be detected")
            .into_dimensions()
            .expect("cover cache dimensions should decode");

        assert_eq!(preview.kind, PreviewKind::Document);
        assert_eq!(preview.detail.as_deref(), Some(detail));
        assert_eq!(visual.kind, PreviewVisualKind::Cover);
        assert_eq!(visual.layout, PreviewVisualLayout::LargeInline);
        assert_eq!(visual.size, cover_bytes.len() as u64);
        assert_eq!(dimensions, (24, 36));
        assert!(visual.path.extension().is_some_and(|ext| ext == "png"));

        let _ = fs::remove_file(visual.path);
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn kindle_preview_does_not_guess_cover_without_exth_pointer() {
    let root = temp_path("kindle-cover-no-pointer");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("covered.mobi");
    let cover_bytes = raster_image_bytes(&root, ImageFormat::Jpeg, 24, 36);
    write_synthetic_kindle_with_resources(
        &path,
        "Covered Handbook",
        &[(503, b"Covered Handbook".as_slice())],
        &[cover_bytes.as_slice()],
    );

    let preview = build_preview(&file_entry(path));

    assert_eq!(preview.kind, PreviewKind::Document);
    assert!(preview.preview_visual.is_none());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn kindle_preview_falls_back_to_thumbnail_when_cover_record_is_not_an_image() {
    let root = temp_path("kindle-cover-thumbnail-fallback");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("covered.azw3");
    let thumbnail_bytes = raster_image_bytes(&root, ImageFormat::Png, 12, 18);
    let cover_offset = 0_u32.to_be_bytes();
    let thumb_offset = 1_u32.to_be_bytes();
    write_synthetic_kindle_with_resources(
        &path,
        "Covered Handbook",
        &[
            (201, cover_offset.as_slice()),
            (202, thumb_offset.as_slice()),
            (503, b"Covered Handbook".as_slice()),
        ],
        &[b"not an image".as_slice(), thumbnail_bytes.as_slice()],
    );

    let preview = build_preview(&file_entry(path));
    let visual = preview
        .preview_visual
        .clone()
        .expect("kindle preview should fall back to thumbnail image");
    let dimensions = image::ImageReader::open(&visual.path)
        .expect("thumbnail cache should open")
        .with_guessed_format()
        .expect("thumbnail cache format should be detected")
        .into_dimensions()
        .expect("thumbnail cache dimensions should decode");

    assert_eq!(preview.kind, PreviewKind::Document);
    assert_eq!(visual.kind, PreviewVisualKind::Cover);
    assert_eq!(visual.layout, PreviewVisualLayout::LargeInline);
    assert_eq!(visual.size, thumbnail_bytes.len() as u64);
    assert_eq!(dimensions, (12, 18));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn kindle_preview_skips_cover_for_encrypted_books() {
    let root = temp_path("kindle-cover-encrypted");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("encrypted.azw3");
    let cover_bytes = raster_image_bytes(&root, ImageFormat::Png, 24, 36);
    let cover_offset = 0_u32.to_be_bytes();
    write_synthetic_kindle_with_options(
        &path,
        "Encrypted Handbook",
        &[
            (201, cover_offset.as_slice()),
            (503, b"Encrypted Handbook".as_slice()),
        ],
        &[cover_bytes.as_slice()],
        2,
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Document);
    assert!(preview.preview_visual.is_none());
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Encryption") && text.contains("Mobipocket encryption"))
    );

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

fn write_synthetic_kindle(path: &std::path::Path, full_name: &str, exth_records: &[(u32, &[u8])]) {
    write_synthetic_kindle_with_resources(path, full_name, exth_records, &[]);
}

fn write_synthetic_kindle_with_resources(
    path: &std::path::Path,
    full_name: &str,
    exth_records: &[(u32, &[u8])],
    resource_records: &[&[u8]],
) {
    write_synthetic_kindle_with_options(path, full_name, exth_records, resource_records, 0);
}

fn write_synthetic_kindle_with_options(
    path: &std::path::Path,
    full_name: &str,
    exth_records: &[(u32, &[u8])],
    resource_records: &[&[u8]],
    encryption: u16,
) {
    let mut record0 = vec![0_u8; 16];
    write_be_u16(&mut record0, 0, 2);
    write_be_u32(&mut record0, 4, 12_345);
    write_be_u16(&mut record0, 8, 2);
    write_be_u16(&mut record0, 10, 4096);
    write_be_u16(&mut record0, 12, encryption);

    let mut mobi_header = vec![0_u8; 256];
    mobi_header[0..4].copy_from_slice(b"MOBI");
    write_be_u32(&mut mobi_header, 4, 256);
    write_be_u32(&mut mobi_header, 8, 2);
    write_be_u32(&mut mobi_header, 12, 65_001);
    write_be_u32(&mut mobi_header, 20, 6);

    let exth = build_exth(exth_records);
    let full_name_offset = 16 + mobi_header.len() + exth.len();
    write_be_u32(&mut mobi_header, 68, full_name_offset as u32);
    write_be_u32(&mut mobi_header, 72, full_name.len() as u32);
    if !resource_records.is_empty() {
        write_be_u32(&mut mobi_header, 92, 1);
    }
    if !exth_records.is_empty() {
        write_be_u32(&mut mobi_header, 112, 0x40);
    }

    record0.extend_from_slice(&mobi_header);
    record0.extend_from_slice(&exth);
    record0.extend_from_slice(full_name.as_bytes());
    record0.extend_from_slice(&[0, 0]);
    while !record0.len().is_multiple_of(4) {
        record0.push(0);
    }

    let mut records = vec![record0];
    records.extend(resource_records.iter().map(|record| record.to_vec()));
    records.push(b"EOF".to_vec());

    let record_count = records.len() as u16;
    let record0_offset = 78 + usize::from(record_count) * 8;
    let mut bytes = vec![0_u8; 78];
    bytes[..13].copy_from_slice(b"Fixture Book\0");
    bytes[60..64].copy_from_slice(b"BOOK");
    bytes[64..68].copy_from_slice(b"MOBI");
    write_be_u16(&mut bytes, 76, record_count);
    let mut offset = record0_offset;
    for record in &records {
        write_record_entry(&mut bytes, offset as u32);
        offset += record.len();
    }
    for record in records {
        bytes.extend_from_slice(&record);
    }

    fs::write(path, bytes).expect("failed to write synthetic kindle file");
}

fn raster_image_bytes(
    root: &std::path::Path,
    format: ImageFormat,
    width_px: u32,
    height_px: u32,
) -> Vec<u8> {
    let extension = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        _ => "img",
    };
    let path = root.join(format!("cover.{extension}"));
    write_test_raster_image(&path, format, width_px, height_px);
    fs::read(path).expect("failed to read raster test image")
}

fn build_exth(records: &[(u32, &[u8])]) -> Vec<u8> {
    if records.is_empty() {
        return Vec::new();
    }

    let mut exth = Vec::new();
    exth.extend_from_slice(b"EXTH");
    exth.extend_from_slice(&[0; 4]);
    exth.extend_from_slice(&(records.len() as u32).to_be_bytes());
    for (kind, value) in records {
        exth.extend_from_slice(&kind.to_be_bytes());
        exth.extend_from_slice(&((8 + value.len()) as u32).to_be_bytes());
        exth.extend_from_slice(value);
    }
    let exth_len = exth.len() as u32;
    exth[4..8].copy_from_slice(&exth_len.to_be_bytes());
    while !exth.len().is_multiple_of(4) {
        exth.push(0);
    }
    exth
}

fn write_record_entry(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&offset.to_be_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 0]);
}

fn write_be_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_be_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}
