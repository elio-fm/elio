use super::*;

#[test]
fn comic_zip_preview_uses_comic_info_without_archive_noise() {
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
    assert!(!line_texts.iter().any(|text| text.contains("Pages")));
    assert!(!line_texts.iter().any(|text| text.contains("Root")));
    assert!(!line_texts.iter().any(|text| text.trim() == "Contents"));
    assert!(!line_texts.iter().any(|text| text.contains("Extras")));
    assert!(
        !line_texts
            .iter()
            .any(|text| text.contains("Format") && text.contains("ZIP"))
    );
    assert!(!line_texts.iter().any(|text| text.contains("Packed")));
    assert!(!line_texts.iter().any(|text| text.contains("Archive Size")));
    assert!(!line_texts.iter().any(|text| text.contains("001-cover.jpg")));
    assert!(!line_texts.iter().any(|text| text.contains("002-page.jpg")));
    assert!(
        !line_texts
            .iter()
            .any(|text| text.contains("notes/readme.txt"))
    );

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_reads_comic_book_info_zip_comment() {
    let root = temp_path("comic-zip-comment-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("comment.cbz");
    let file = File::create(&path).expect("failed to create comic zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("001.jpg", options)
        .expect("failed to start page entry");
    zip.write_all(b"page").expect("failed to write page entry");
    zip.set_comment(
        r#"{"appID":"FixtureReader/1","ComicBookInfo/1.0":{"series":"Aurora Riders","title":"First Light","issue":"1","publisher":"Elio Press","publicationYear":1958,"genre":"Sci-Fi","credits":[{"role":"Writer","person":"Lee Maven"}]}}"#,
    )
    .expect("failed to set comic zip comment");
    zip.finish().expect("failed to finish comic zip");

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
            .any(|text| text.contains("Title") && text.contains("First Light"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("Aurora Riders"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Number") && text.contains("1"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Year") && text.contains("1958"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Publisher") && text.contains("Elio Press"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Writer") && text.contains("Lee Maven"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Genre") && text.contains("Sci-Fi"))
    );

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_reads_comet_xml_metadata() {
    let root = temp_path("comic-zip-comet-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("comet.cbz");
    let file = File::create(&path).expect("failed to create comic zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("001.jpg", options)
        .expect("failed to start page entry");
    zip.write_all(b"page").expect("failed to write page entry");
    zip.start_file("CoMet.xml", options)
        .expect("failed to start comet entry");
    zip.write_all(
        br#"<?xml version="1.0" encoding="utf-8"?>
            <comet>
              <title>Moon Orbit</title>
              <series>Orbital Stories</series>
              <issue>5</issue>
              <volume>2</volume>
              <publisher>Elio Press</publisher>
              <date>1994-11-05</date>
              <genre>Science Fiction</genre>
            </comet>"#,
    )
    .expect("failed to write comet entry");
    zip.finish().expect("failed to finish comic zip");

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
            .any(|text| text.contains("Title") && text.contains("Moon Orbit"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("Orbital Stories"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Number") && text.contains("5"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("2"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Year") && text.contains("1994"))
    );

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_reads_metron_info_metadata() {
    let root = temp_path("comic-zip-metron-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("metron.cbz");
    let file = File::create(&path).expect("failed to create comic zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("001.jpg", options)
        .expect("failed to start page entry");
    zip.write_all(b"page").expect("failed to write page entry");
    zip.start_file("MetronInfo.xml", options)
        .expect("failed to start metron info entry");
    zip.write_all(
        br#"<?xml version="1.0" encoding="utf-8"?>
            <MetronInfo>
              <Publisher>
                <Name>Elio Press</Name>
              </Publisher>
              <Series>
                <Name>Signal Hammer</Name>
                <Volume>1</Volume>
              </Series>
              <Number>12</Number>
              <Stories>
                <Story>The End</Story>
              </Stories>
              <CoverDate>2017-06-21</CoverDate>
              <Genres>
                <Genre>Superhero</Genre>
              </Genres>
              <Credits>
                <Credit>
                  <Creator>Avery Quill</Creator>
                  <Roles>
                    <Role>Writer</Role>
                  </Roles>
                </Credit>
                <Credit>
                  <Creator>Morgan Line</Creator>
                  <Roles>
                    <Role>Artist</Role>
                  </Roles>
                </Credit>
              </Credits>
            </MetronInfo>"#,
    )
    .expect("failed to write metron info entry");
    zip.finish().expect("failed to finish comic zip");

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
            .any(|text| text.contains("Title") && text.contains("The End"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("Signal Hammer"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Number") && text.contains("12"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("1"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Year") && text.contains("2017"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Publisher") && text.contains("Elio Press"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Writer") && text.contains("Avery Quill"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Penciller") && text.contains("Morgan Line"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Genre") && text.contains("Superhero"))
    );

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn comic_zip_preview_reads_acbf_metadata() {
    let root = temp_path("comic-zip-acbf-metadata");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("acbf.cbz");
    let file = File::create(&path).expect("failed to create comic zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("page01.jpg", options)
        .expect("failed to start page entry");
    zip.write_all(b"page").expect("failed to write page entry");
    zip.start_file("metadata/book.acbf", options)
        .expect("failed to start acbf entry");
    zip.write_all(
        br#"<?xml version="1.0" encoding="utf-8"?>
            <ACBF xmlns="http://www.acbf.info/xml/acbf/1.1">
              <meta-data>
                <book-info>
                  <author activity="Writer">
                    <first-name>Rhea</first-name>
                    <last-name>Quinn</last-name>
                  </author>
                  <author activity="Artist">
                    <nickname>Northline Studio</nickname>
                  </author>
                  <book-title>Northline Relay</book-title>
                  <genre>Science Fiction</genre>
                  <sequence title="Futuristic Tales" volume="1">3</sequence>
                </book-info>
                <publish-info>
                  <publisher>Elio Press</publisher>
                  <publish-date value="2007-02-01">February 2007</publish-date>
                </publish-info>
              </meta-data>
            </ACBF>"#,
    )
    .expect("failed to write acbf entry");
    zip.finish().expect("failed to finish comic zip");

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
            .any(|text| text.contains("Title") && text.contains("Northline Relay"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Series") && text.contains("Futuristic Tales"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Number") && text.contains("3"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Volume") && text.contains("1"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Year") && text.contains("2007"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Publisher") && text.contains("Elio Press"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Writer") && text.contains("Rhea Quinn"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Penciller") && text.contains("Northline Studio"))
    );
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Genre") && text.contains("Science Fiction"))
    );

    let _ = fs::remove_file(visual.path);
    fs::remove_dir_all(root).expect("failed to remove temp root");
}
