mod comic;
mod common;
mod external;
mod format;
mod internal;
mod manifest;
mod render;

use self::comic::build_comic_archive_preview;
use self::common::normalize_archive_path;
use self::external::{
    collect_archive_entries_with_bsdtar, collect_archive_listing_with_7z,
    fallback_single_file_archive_entry,
};
use self::format::{
    archive_default_label, archive_format_name, archive_is_empty_label, detect_archive_format,
};
use self::internal::{collect_internal_archive_listing, collect_preferred_archive_entries};
use self::manifest::{parse_zip_manifest, zip_manifest_sections};
use self::render::{ArchiveRenderConfig, render_archive_preview};
use super::*;
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
use zip::ZipArchive;

pub(in crate::preview) fn build_archive_preview(
    path: &Path,
    type_detail: Option<&'static str>,
    comic_page_index: Option<usize>,
) -> Option<PreviewContent> {
    let format = detect_archive_format(path);
    if matches!(format, ArchiveFormat::ComicZip | ArchiveFormat::ComicRar)
        && let Some(preview) =
            build_comic_archive_preview(path, format, type_detail, comic_page_index.unwrap_or(0))
    {
        return Some(preview);
    }
    if let Some(preview) = build_zip_archive_preview(path, format, type_detail) {
        return Some(preview);
    }
    if let Some(preview) = build_tar_archive_preview(path, format, type_detail) {
        return Some(preview);
    }
    build_external_archive_preview(path, format, type_detail)
}

fn build_zip_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    if !matches!(format, ArchiveFormat::Zip | ArchiveFormat::ComicZip) {
        return None;
    }

    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    let total_entries = archive.len();
    let mut entries = Vec::with_capacity(total_entries.min(ARCHIVE_ENTRY_SCAN_LIMIT));
    let mut metadata = ArchiveMetadata {
        format_label: Some(archive_format_name(format).to_string()),
        physical_size: fs::metadata(path).ok().map(|metadata| metadata.len()),
        ..ArchiveMetadata::default()
    };
    let mut manifest = ZipManifestMetadata::default();

    for index in 0..total_entries.min(ARCHIVE_ENTRY_SCAN_LIMIT) {
        let entry = archive.by_index(index).ok()?;
        let is_dir = entry.is_dir();
        let name = entry.name().to_string();
        if let Some(path) = normalize_archive_path(&name, false) {
            entries.push(ArchiveEntry { path, is_dir });
        }
        metadata.unpacked_size = Some(
            metadata
                .unpacked_size
                .unwrap_or(0)
                .saturating_add(entry.size()),
        );
        metadata.compressed_size = Some(
            metadata
                .compressed_size
                .unwrap_or(0)
                .saturating_add(entry.compressed_size()),
        );

        if manifest.is_empty()
            && !is_dir
            && name.eq_ignore_ascii_case("META-INF/MANIFEST.MF")
            && entry.size() <= ZIP_MANIFEST_LIMIT_BYTES
        {
            let mut contents = String::new();
            if entry
                .take(ZIP_MANIFEST_LIMIT_BYTES)
                .read_to_string(&mut contents)
                .is_ok()
            {
                manifest = parse_zip_manifest(&contents);
            }
        }
    }

    let comment = String::from_utf8_lossy(archive.comment());
    let comment = comment.trim();
    if !comment.is_empty() {
        metadata.comment = Some(comment.to_string());
    }

    let detail = type_detail.unwrap_or(archive_default_label(format));
    let scan_truncated = total_entries > ARCHIVE_ENTRY_SCAN_LIMIT;
    let preview = render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata,
        entries: Some(entries),
        total_entries_hint: Some(total_entries),
        empty_label: archive_is_empty_label(format),
        unavailable_label: "Unable to read archive contents",
        extra_sections: zip_manifest_sections(&manifest),
        scan_truncated,
    });
    Some(preview)
}

fn build_tar_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    let (metadata, entries, total_entries, scan_truncated) =
        collect_internal_archive_listing(path, format)?;
    let detail = type_detail.unwrap_or(archive_default_label(format));

    Some(render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata,
        entries: Some(entries),
        total_entries_hint: Some(total_entries),
        empty_label: archive_is_empty_label(format),
        unavailable_label: "Unable to read archive contents",
        extra_sections: Vec::new(),
        scan_truncated,
    }))
}

fn build_external_archive_preview(
    path: &Path,
    format: ArchiveFormat,
    type_detail: Option<&'static str>,
) -> Option<PreviewContent> {
    // Common ZIP and TAR previews are handled internally above. This path is for
    // recovery and uncommon archive types, where 7z provides the broadest coverage
    // and bsdtar remains a final generic fallback.
    let detail = type_detail.unwrap_or(archive_default_label(format));
    if let Some(entries) = collect_preferred_archive_entries(path, format) {
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata: ArchiveMetadata {
                format_label: Some(archive_format_name(format).to_string()),
                ..ArchiveMetadata::default()
            },
            entries: Some(entries),
            total_entries_hint: None,
            empty_label: archive_is_empty_label(format),
            unavailable_label: "Unable to read archive contents",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }

    if let Some((metadata, mut entries)) = collect_archive_listing_with_7z(path) {
        if entries.is_empty()
            && let Some(entry) = fallback_single_file_archive_entry(path, format)
        {
            entries.push(entry);
        }
        return Some(render_archive_preview(ArchiveRenderConfig {
            detail: detail.to_string(),
            metadata,
            entries: Some(entries),
            total_entries_hint: None,
            empty_label: archive_is_empty_label(format),
            unavailable_label: "Unable to read archive contents",
            extra_sections: Vec::new(),
            scan_truncated: false,
        }));
    }

    let entries = collect_archive_entries_with_bsdtar(path)?;

    Some(render_archive_preview(ArchiveRenderConfig {
        detail: detail.to_string(),
        metadata: ArchiveMetadata {
            format_label: Some(archive_format_name(format).to_string()),
            ..ArchiveMetadata::default()
        },
        entries: Some(entries),
        total_entries_hint: None,
        empty_label: archive_is_empty_label(format),
        unavailable_label: "Unable to read archive contents",
        extra_sections: Vec::new(),
        scan_truncated: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::comic::{
        ComicArchiveBackend, build_comic_archive_preview, parse_comic_archive_from_7z_output,
    };
    use super::external::parse_7z_listing;
    use super::manifest::parse_zip_manifest;
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::{
        env, fs,
        path::PathBuf,
        process::Command,
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

        let comic =
            parse_comic_archive_from_7z_output(output).expect("7z output should yield comic pages");

        assert_eq!(comic.backend, ComicArchiveBackend::SevenZip);
        assert_eq!(comic.page_entries.len(), 3);
        assert_eq!(comic.page_entries[0].entry_name, "001.jpg");
        assert_eq!(comic.page_entries[1].entry_name, "002.jpg");
        assert_eq!(comic.page_entries[2].entry_name, "010.jpg");
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

        let preview = build_archive_preview(&archive, Some("Comic RAR archive"), Some(0))
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
    fn parse_7z_listing_collects_external_fallback_metadata_and_entries() {
        let output = r#"
Path = app.AppImage
Type = SquashFS
Physical Size = 12345
Comment = portable build

----------
Path = AppRun
Folder = -
Size = 12
Packed Size = 10

Path = usr/bin/elio
Folder = -
Size = 52
Packed Size = 20

Path = usr/share/icons
Folder = +
Size = 0
Packed Size = 0
"#;

        let (metadata, entries) =
            parse_7z_listing(output).expect("7z listing should parse archive metadata");

        assert_eq!(metadata.format_label.as_deref(), Some("SquashFS"));
        assert_eq!(metadata.physical_size, Some(12_345));
        assert_eq!(metadata.comment.as_deref(), Some("portable build"));
        assert_eq!(metadata.unpacked_size, Some(64));
        assert_eq!(metadata.compressed_size, Some(30));
        assert_eq!(entries.len(), 3);
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "AppRun" && !entry.is_dir)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "usr/bin/elio" && !entry.is_dir)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.path == "usr/share/icons" && entry.is_dir)
        );
    }

    #[test]
    fn parse_zip_manifest_supports_bundle_fallback_and_continuations() {
        let manifest = parse_zip_manifest(concat!(
            "Bundle-Name: Elio Runtime\n",
            "Bundle-Version: 2.0.0\n",
            "Main-Class: io.elio.Main\n",
            "Automatic-Module-Name: io.elio.\n",
            " core\n",
        ));

        assert_eq!(manifest.title.as_deref(), Some("Elio Runtime"));
        assert_eq!(manifest.version.as_deref(), Some("2.0.0"));
        assert_eq!(manifest.main_class.as_deref(), Some("io.elio.Main"));
        assert_eq!(manifest.automatic_module.as_deref(), Some("io.elio.core"));
        assert!(manifest.created_by.is_none());
    }
}
