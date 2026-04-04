use super::super::ArchiveFormat;
use super::super::build_archive_preview;
use super::{
    ComicArchiveBackend, ComicArchiveSignature, build_comic_archive_preview,
    parse_comic_archive_from_7z_output, parse_zip_comic_archive, sniff_comic_archive_signature,
};
use crate::preview::PreviewKind;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::{
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    env::temp_dir().join(format!("elio-comic-archive-{label}-{unique}"))
}

#[test]
fn sniff_comic_archive_signature_detects_common_formats() {
    let root = temp_path("signature-sniff");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let zip = root.join("issue.cbz");
    fs::write(&zip, b"PK\x03\x04demo").expect("failed to write zip signature");
    assert_eq!(
        sniff_comic_archive_signature(&zip),
        ComicArchiveSignature::Zip
    );

    let rar = root.join("issue.cbr");
    fs::write(&rar, b"Rar!\x1a\x07\x01\x00demo").expect("failed to write rar signature");
    assert_eq!(
        sniff_comic_archive_signature(&rar),
        ComicArchiveSignature::Rar
    );

    let seven_zip = root.join("issue.7z");
    fs::write(&seven_zip, [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, 0, 0])
        .expect("failed to write 7z signature");
    assert_eq!(
        sniff_comic_archive_signature(&seven_zip),
        ComicArchiveSignature::SevenZip
    );

    let unknown = root.join("issue.bin");
    fs::write(&unknown, b"not-an-archive").expect("failed to write unknown file");
    assert_eq!(
        sniff_comic_archive_signature(&unknown),
        ComicArchiveSignature::Unknown
    );

    fs::remove_dir_all(&root).expect("failed to remove temp root");
}

#[test]
fn parses_comic_pages_from_7z_listing_output() {
    let output = r#"
Path = /tmp/berserk.cbz
Type = Rar
Physical Size = 1024

----------
Path = 010.jpg
Folder = -
Size = 10
Packed Size = 10

Path = 002.jpg
Folder = -
Size = 20
Packed Size = 20

Path = notes/readme.txt
Folder = -
Size = 30
Packed Size = 30

Path = 001.jpg
Folder = -
Size = 40
Packed Size = 40
"#;

    let comic = parse_comic_archive_from_7z_output(output, &|| false)
        .expect("7z output should yield comic pages");

    assert_eq!(comic.backend, ComicArchiveBackend::SevenZip);
    assert_eq!(comic.page_entries.len(), 3);
    assert_eq!(comic.page_entries[0].entry_name, "001.jpg");
    assert_eq!(comic.page_entries[1].entry_name, "002.jpg");
    assert_eq!(comic.page_entries[2].entry_name, "010.jpg");
}

#[test]
fn parse_zip_comic_archive_returns_none_when_canceled() {
    let root = temp_path("zip-cancel");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbz");
    let file = File::create(&archive).expect("failed to create comic zip");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("001.jpg", options)
        .expect("failed to start first page");
    zip.write_all(b"page-one")
        .expect("failed to write first page");
    zip.start_file("002.jpg", options)
        .expect("failed to start second page");
    zip.write_all(b"page-two")
        .expect("failed to write second page");
    zip.finish().expect("failed to finish comic zip");

    let canceled = AtomicBool::new(true);
    let parsed = parse_zip_comic_archive(&archive, &|| canceled.load(Ordering::Relaxed));
    assert!(parsed.is_none(), "canceled zip parsing should stop early");

    fs::remove_dir_all(&root).expect("failed to remove temp root");
}

#[test]
fn build_comic_archive_preview_falls_back_to_7z_for_mislabeled_cbz() {
    let root = temp_path("mislabeled-cbz");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let first = root.join("001.png");
    let second = root.join("010.png");
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255])));
    image
        .save_with_format(&first, ImageFormat::Png)
        .expect("failed to write first image");
    image
        .save_with_format(&second, ImageFormat::Png)
        .expect("failed to write second image");

    let archive = root.join("broken.cbz");
    let status = Command::new("7z")
        .current_dir(&root)
        .arg("a")
        .arg("-t7z")
        .arg(&archive)
        .arg("001.png")
        .arg("010.png")
        .status();
    let Ok(status) = status else {
        fs::remove_dir_all(&root).expect("failed to remove temp root");
        return;
    };
    if !status.success() {
        fs::remove_dir_all(&root).expect("failed to remove temp root");
        return;
    }

    let preview = build_comic_archive_preview(
        &archive,
        ArchiveFormat::ComicZip,
        Some("Comic ZIP archive"),
        0,
        &|| false,
    )
    .expect("mislabeled cbz should still build comic preview");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic ZIP archive"));
    assert_eq!(
        preview
            .navigation_position
            .as_ref()
            .map(|position| position.count),
        Some(2)
    );
    let visual = preview
        .preview_visual
        .as_ref()
        .expect("comic preview should expose a page visual");
    let dimensions = image::ImageReader::open(&visual.path)
        .expect("extracted page should open")
        .with_guessed_format()
        .expect("page format should be detected")
        .into_dimensions()
        .expect("page dimensions should be readable");
    assert_eq!(dimensions, (1, 1));

    fs::remove_dir_all(&root).expect("failed to remove temp root");
}

#[test]
fn build_archive_preview_detects_cbr_as_comic_when_7z_backend_is_needed() {
    let root = temp_path("cbr-7z-backend");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let first = root.join("001.png");
    let second = root.join("010.png");
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255])));
    image
        .save_with_format(&first, ImageFormat::Png)
        .expect("failed to write first image");
    image
        .save_with_format(&second, ImageFormat::Png)
        .expect("failed to write second image");

    let archive = root.join("issue.cbr");
    let status = Command::new("7z")
        .current_dir(&root)
        .arg("a")
        .arg("-t7z")
        .arg(&archive)
        .arg("001.png")
        .arg("010.png")
        .status();
    let Ok(status) = status else {
        fs::remove_dir_all(&root).expect("failed to remove temp root");
        return;
    };
    if !status.success() {
        fs::remove_dir_all(&root).expect("failed to remove temp root");
        return;
    }

    let preview = build_archive_preview(&archive, Some("Comic RAR archive"), Some(0), &|| false)
        .expect("cbr should build comic preview");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic RAR archive"));
    assert_eq!(
        preview
            .navigation_position
            .as_ref()
            .map(|position| position.count),
        Some(2)
    );
    assert!(preview.preview_visual.is_some());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn build_comic_archive_preview_shows_status_note_when_cbr_cannot_be_opened() {
    // Write a file with a RAR5 magic header but invalid/truncated body.
    // All backends (ZIP, 7z, unrar) reject it, so the code must choose a
    // status note based on whether a RAR-capable extractor is installed:
    //   • no extractor  → "RAR preview requires unrar or a 7z build with RAR support"
    //   • extractor present but file unreadable → "Unable to read RAR archive …"
    // Both messages contain "RAR", which is the common denominator we assert.
    let root = temp_path("cbr-unreadable");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let archive = root.join("issue.cbr");
    let rar5_header = b"Rar!\x1a\x07\x01\x00\x00\x00\x00";
    fs::write(&archive, rar5_header).expect("failed to write fake rar header");

    let preview = build_comic_archive_preview(
        &archive,
        ArchiveFormat::ComicRar,
        Some("Comic RAR archive"),
        0,
        &|| false,
    )
    .expect("unreadable cbr should return a status preview, not None");

    assert_eq!(preview.kind, PreviewKind::Comic);
    assert_eq!(preview.detail.as_deref(), Some("Comic RAR archive"));
    assert!(
        preview.navigation_position.is_none(),
        "no pages should be navigable when no backend could open the archive"
    );
    let status = preview.status_note.as_deref().unwrap_or("");
    assert!(
        status.contains("RAR"),
        "status note should mention RAR, got: {status:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
