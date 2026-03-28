use super::super::*;
pub(super) use image::ImageFormat;
use image::{DynamicImage, Rgba, RgbaImage};
pub(super) use std::{fs, thread, time::Duration};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

pub(super) fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-preview-worker-{label}-{unique}"))
}

pub(super) fn write_zip_entries(path: &Path, entries: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (name, contents) in entries {
        zip.start_file(name, options)
            .expect("failed to start zip entry");
        zip.write_all(contents.as_bytes())
            .expect("failed to write zip entry");
    }

    zip.finish().expect("failed to finish zip");
}

pub(super) fn write_binary_zip_entries(path: &Path, entries: &[(&str, &[u8])]) {
    let file = File::create(path).expect("failed to create zip");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (name, contents) in entries {
        zip.start_file(name, options)
            .expect("failed to start zip entry");
        zip.write_all(contents).expect("failed to write zip entry");
    }

    zip.finish().expect("failed to finish zip");
}

pub(super) fn write_test_raster_image(
    path: &Path,
    format: ImageFormat,
    width_px: u32,
    height_px: u32,
) {
    let mut image = RgbaImage::new(width_px, height_px);
    for pixel in image.pixels_mut() {
        *pixel = Rgba([32, 128, 224, 255]);
    }

    DynamicImage::ImageRgba8(image)
        .save_with_format(path, format)
        .expect("failed to write raster test image");
}

pub(super) fn write_epub_fixture(path: &Path, sections: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("META-INF/container.xml", options)
        .expect("failed to start container entry");
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
            <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
              <rootfiles>
                <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
              </rootfiles>
            </container>"#,
    )
    .expect("failed to write container entry");

    let manifest = sections
        .iter()
        .enumerate()
        .map(|(index, _)| {
            format!(
                r#"<item id="chapter-{id}" href="text/chapter-{id}.xhtml" media-type="application/xhtml+xml"/>"#,
                id = index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let spine = sections
        .iter()
        .enumerate()
        .map(|(index, _)| format!(r#"<itemref idref="chapter-{}"/>"#, index + 1))
        .collect::<Vec<_>>()
        .join("");
    let nav = sections
        .iter()
        .enumerate()
        .map(|(index, (title, _))| {
            format!(
                r#"<li><a href="text/chapter-{id}.xhtml">{title}</a></li>"#,
                id = index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let package = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
              <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                <dc:title>Wheel Book</dc:title>
                <dc:creator>Regueiro</dc:creator>
              </metadata>
              <manifest>
                <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                {manifest}
              </manifest>
              <spine>{spine}</spine>
            </package>"#
    );
    zip.start_file("OPS/package.opf", options)
        .expect("failed to start package entry");
    zip.write_all(package.as_bytes())
        .expect("failed to write package entry");

    let nav_document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
              <body>
                <nav epub:type="toc">
                  <ol>{nav}</ol>
                </nav>
              </body>
            </html>"#
    );
    zip.start_file("OPS/nav.xhtml", options)
        .expect("failed to start nav entry");
    zip.write_all(nav_document.as_bytes())
        .expect("failed to write nav entry");

    for (index, (title, body)) in sections.iter().enumerate() {
        let chapter = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <h1>{title}</h1>
                    {body}
                  </body>
                </html>"#
        );
        zip.start_file(format!("OPS/text/chapter-{}.xhtml", index + 1), options)
            .expect("failed to start chapter entry");
        zip.write_all(chapter.as_bytes())
            .expect("failed to write chapter entry");
    }

    zip.finish().expect("failed to finish epub");
}

pub(super) fn write_fixed_layout_epub_fixture(path: &Path, section_titles: &[&str]) {
    let file = File::create(path).expect("failed to create epub");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("META-INF/container.xml", options)
        .expect("failed to start container entry");
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
            <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
              <rootfiles>
                <rootfile full-path="OPS/package.opf" media-type="application/oebps-package+xml"/>
              </rootfiles>
            </container>"#,
    )
    .expect("failed to write container entry");

    let manifest = section_titles
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let id = index + 1;
            format!(
                r#"<item id="page-{id}" href="xhtml/page-{id}.xhtml" media-type="application/xhtml+xml" properties="svg"/><item id="image-{id}" href="image/page-{id}.jpg" media-type="image/jpeg"/>"#
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let spine = section_titles
        .iter()
        .enumerate()
        .map(|(index, _)| format!(r#"<itemref idref="page-{}"/>"#, index + 1))
        .collect::<Vec<_>>()
        .join("");
    let nav = section_titles
        .iter()
        .enumerate()
        .map(|(index, title)| {
            format!(
                r#"<li><a href="xhtml/page-{id}.xhtml">{title}</a></li>"#,
                id = index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let package = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
              <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                <dc:title>Fixed Layout Book</dc:title>
                <meta property="rendition:layout">pre-paginated</meta>
              </metadata>
              <manifest>
                <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
                {manifest}
              </manifest>
              <spine>{spine}</spine>
            </package>"#
    );
    zip.start_file("OPS/package.opf", options)
        .expect("failed to start package entry");
    zip.write_all(package.as_bytes())
        .expect("failed to write package entry");

    let nav_document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
              <body>
                <nav epub:type="toc">
                  <ol>{nav}</ol>
                </nav>
              </body>
            </html>"#
    );
    zip.start_file("OPS/nav.xhtml", options)
        .expect("failed to start nav entry");
    zip.write_all(nav_document.as_bytes())
        .expect("failed to write nav entry");

    for (index, _) in section_titles.iter().enumerate() {
        let id = index + 1;
        let chapter = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <html xmlns="http://www.w3.org/1999/xhtml">
                  <body>
                    <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                      <image width="1600" height="900" xlink:href="../image/page-{id}.jpg"/>
                    </svg>
                  </body>
                </html>"#
        );
        zip.start_file(format!("OPS/xhtml/page-{id}.xhtml"), options)
            .expect("failed to start chapter entry");
        zip.write_all(chapter.as_bytes())
            .expect("failed to write chapter entry");
        zip.start_file(format!("OPS/image/page-{id}.jpg"), options)
            .expect("failed to start image entry");
        zip.write_all(b"jpeg").expect("failed to write image entry");
    }

    zip.finish().expect("failed to finish epub");
}

pub(super) fn wait_for_background_preview(app: &mut App) {
    for _ in 0..500 {
        if app.process_background_jobs() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for background preview");
}

pub(super) fn wait_for_preview_prefetch(app: &mut App) {
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        let _ = app.process_preview_prefetch_timers();
        if app.pending_preview_prefetch_timer().is_none() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for preview prefetch");
}

pub(super) fn wait_for_preview_header(
    app: &mut App,
    visible_rows: usize,
    width: usize,
    expected: &str,
) {
    let mut last_seen = None;
    for _ in 0..200 {
        let current = app.preview_header_detail_for_width(visible_rows, width);
        if current.as_deref() == Some(expected) {
            return;
        }
        last_seen = current;
        let _ = app.process_background_jobs();
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for preview header {expected:?}; last seen: {last_seen:?}");
}

pub(super) fn wait_for_preview_total_line_count(app: &mut App, expected_total: usize) {
    let mut last_seen = None;
    for _ in 0..200 {
        let current = app
            .preview_state
            .content
            .line_coverage
            .as_ref()
            .and_then(|coverage| coverage.total_lines);
        if current == Some(expected_total) {
            return;
        }
        last_seen = current;
        let _ = app.process_background_jobs();
        thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "timed out waiting for preview total line count {expected_total}; last seen: {last_seen:?}"
    );
}

pub(super) fn wait_for_directory_load(app: &mut App) {
    for _ in 0..200 {
        let _ = app.process_background_jobs();
        if app.directory_runtime.pending_load.is_none() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for directory load");
}

pub(super) fn write_docx_fixture(path: &Path) {
    write_zip_entries(
        path,
        &[
            (
                "docProps/core.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/"
                        xmlns:dcterms="http://purl.org/dc/terms/">
                      <dc:title>Quarterly Report</dc:title>
                      <dc:creator>Regueiro</dc:creator>
                      <dcterms:created>2026-03-11T09:00:00Z</dcterms:created>
                    </cp:coreProperties>"#,
            ),
            (
                "docProps/app.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>LibreOffice</Application>
                      <Pages>12</Pages>
                      <Words>4238</Words>
                    </Properties>"#,
            ),
        ],
    );
}
