use super::super::*;
use std::{
    env,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

pub(super) fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = env::temp_dir().join(format!("elio-events-{label}-{unique}"));
    // Pre-canonicalize so that both sides of assert_eq!(app.cwd, root) use the
    // same path form. On Windows env::temp_dir() returns 8.3 short names while
    // navigate_to() resolves to \\?\ paths; on macOS /var is a symlink to
    // /private/var. Creating the directory first makes canonicalize() succeed.
    std::fs::create_dir_all(&path).ok();
    path.canonicalize().unwrap_or(path)
}

pub(super) fn wait_for_directory_load(app: &mut App) {
    for _ in 0..100 {
        let _ = app.process_background_jobs();
        if app.directory_runtime.pending_load.is_none() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for directory load");
}

pub(super) fn wait_for_background_preview(app: &mut App) {
    for _ in 0..200 {
        if app.process_background_jobs() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for background preview");
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
