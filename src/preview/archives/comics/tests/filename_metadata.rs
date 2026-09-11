use super::*;

#[test]
fn comic_zip_preview_derives_metadata_from_structured_names() {
    let root = temp_path("comic-zip-derived-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("Moon Ledger v01 (2005) (Digital) (Fixture Group) (ED).cbz");
    write_zip_binary_entries(
        &path,
        &[
            (
                "Moon Ledger - c001 (v01) - p000 [Elio Press] [Digital] [Fixture Group].jpg",
                b"chapter-one",
            ),
            (
                "Moon Ledger - c007 (v01) - p195 [Elio Press] [Digital] [Fixture Group].png",
                b"chapter-seven",
            ),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic zip should expose a page visual");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic ZIP archive"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("Moon Ledger"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("1"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Year") && text.contains("2005"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Publisher") && text.contains("Elio Press"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Source") && text.contains("Digital"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Chapters") && text.contains("1-7"))
    );
    assert!(!line_texts.iter().any(|text| text.contains("Pages")));
    assert!(!line_texts.iter().any(|text| text.contains("Root")));
    assert!(!line_texts.iter().any(|text| text.trim() == "Contents"));
    assert!(!line_texts.iter().any(|text| text.contains("Extras")));
    assert!(!line_texts.iter().any(|text| text.contains("p000")));
    assert!(!line_texts.iter().any(|text| text.contains("Fixture Group")));
    assert!(!line_texts.iter().any(|text| text.contains("ED")));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_derives_tome_volume_and_digital_source_from_filenames() {
    let root = temp_path("comic-zip-tome-derived-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("Skyline Saga T01 (Mira) (2018) [Digital-1699] [Fixture FR].cbz");
    write_zip_binary_entries(
        &path,
        &[("0001_0000.jpg", b"cover"), ("0002_0001.jpg", b"page")],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic zip should expose a page visual");

    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("Skyline Saga"))
    );
    assert!(
        !line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("T01"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("1"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Year") && text.contains("2018"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Source") && text.contains("Digital"))
    );

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_derives_metadata_from_bracketed_names_without_group_noise() {
    let root = temp_path("comic-zip-bracketed-derived-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("[ScanGroup] Harbor Case v01 [Digital].cbz");
    write_zip_binary_entries(&path, &[("001.jpg", b"cover"), ("002.jpg", b"page-one")]);

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic zip should expose a page visual");

    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("Harbor Case"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("1"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Source") && text.contains("Digital"))
    );
    assert!(!line_texts.iter().any(|text| text.contains("ScanGroup")));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_derives_series_from_collection_folder() {
    let root = temp_path("comic-zip-folder-metadata");
    let collection = root.join("[FixtureGroup] Harbor Case");
    fs::create_dir_all(&collection).expect("failed to create collection folder");
    let path = collection.join("Volume 01.cbz");
    write_zip_binary_entries(&path, &[("000.jpg", b"cover"), ("001.png", b"page-one")]);

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic zip should expose a page visual");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic ZIP archive"));
    assert_eq!(line_texts.first().map(String::as_str), Some("Details"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("Harbor Case"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("1"))
    );
    assert!(!line_texts.iter().any(|text| text.contains("FixtureGroup")));
    assert!(!line_texts.iter().any(|text| text.contains("Pages")));
    assert!(!line_texts.iter().any(|text| text.contains("Root")));
    assert!(!line_texts.iter().any(|text| text.contains("000.jpg")));

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_avoids_generic_derived_metadata() {
    let root = temp_path("comic-zip-generic-derived-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("issue.cbz");
    write_zip_binary_entries(
        &path,
        &[
            ("c001-p001 [Digital].jpg", b"cover"),
            ("c001-p002.jpg", b"page-one"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();
    let visual = preview
        .preview_visual
        .clone()
        .expect("comic zip should expose a page visual");

    assert!(line_texts.is_empty());

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
