use super::*;

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
fn zip_preview_uses_license_icon_for_canonical_license_entries() {
    let root = temp_path("zip-license-icons");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("licenses.zip");
    write_zip_entries(
        &path,
        &[
            ("LICENSE", "MIT License\n"),
            ("LICENSE-MIT", "MIT License\n"),
            ("COPYING.LESSER", "GNU Lesser General Public License\n"),
            ("UNLICENSE", "The Unlicense\n"),
            ("license/readme.txt", "not a license directory\n"),
        ],
    );

    let preview = build_preview(&file_entry(path));
    let license_icon =
        theme::resolve_path_with_class(Path::new("LICENSE"), EntryKind::File, FileClass::License)
            .icon;
    let directory_icon = theme::resolve_path(Path::new("license"), EntryKind::Directory).icon;

    for name in ["LICENSE", "LICENSE-MIT", "COPYING.LESSER", "UNLICENSE"] {
        let line = preview
            .lines
            .iter()
            .find(|line| line_text(line).contains(name))
            .unwrap_or_else(|| panic!("archive preview should show {name}"));
        assert_eq!(line.spans[1].content.trim(), license_icon);
    }

    let directory_line = preview
        .lines
        .iter()
        .find(|line| line_text(line).contains("license/"))
        .expect("archive preview should show the license directory");
    assert_eq!(directory_line.spans[1].content.trim(), directory_icon);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn encrypted_seven_zip_preview_stays_responsive_with_password_notice() {
    let root = temp_path("encrypted-7z-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("secret.7z");
    let mut writer = sevenz_rust2::ArchiveWriter::create(&path).expect("failed to create 7z");
    writer.set_content_methods(vec![
        sevenz_rust2::encoder_options::AesEncoderOptions::new(sevenz_rust2::Password::new(
            "secret",
        ))
        .into(),
        sevenz_rust2::encoder_options::Lzma2Options::default().into(),
    ]);
    writer
        .push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("dir/file.txt"),
            Some(&b"hello"[..]),
        )
        .expect("failed to write 7z entry");
    writer.finish().expect("failed to finish 7z");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("7z archive"));
    assert!(line_texts.iter().any(|text| text == "Details"));
    assert!(line_texts.iter().any(|text| text == "Contents"));
    assert!(
        line_texts
            .iter()
            .any(|text| text.contains("Password-protected"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn corrupt_seven_zip_preview_does_not_claim_password_required() {
    let root = temp_path("corrupt-7z-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("broken.7z");
    fs::write(&path, b"not a seven zip archive").expect("failed to write broken 7z");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("7z archive"));
    assert!(
        line_texts.iter().any(|text| text.contains("Unavailable")),
        "corrupt 7z should stay in archive preview: {line_texts:?}"
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("require a password")),
        "corrupt 7z must not be mislabeled as password-protected: {line_texts:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn corrupt_zip_preview_stays_archive_when_contents_are_unavailable() {
    let root = temp_path("corrupt-zip-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("broken.zip");
    fs::write(&path, b"not a zip archive").expect("failed to write broken zip");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("ZIP archive"));
    assert!(line_texts.iter().any(|text| text == "Details"));
    assert!(line_texts.iter().any(|text| text == "Contents"));
    assert!(
        line_texts.iter().any(|text| text.contains("Unavailable")),
        "known archive failures should not fall through to binary preview: {line_texts:?}"
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("Binary or unsupported file")),
        "known archive failures should not render binary fallback: {line_texts:?}"
    );
    assert_eq!(preview.status_note.as_deref(), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn unreadable_rar_preview_stays_archive_when_contents_are_unavailable() {
    let root = temp_path("unreadable-rar-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("broken.rar");
    fs::write(&path, b"not a rar archive").expect("failed to write broken rar");

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("RAR archive"));
    assert!(
        line_texts.iter().any(|text| text.contains("Unavailable")),
        "unreadable RAR should stay in archive preview: {line_texts:?}"
    );
    assert!(
        line_texts
            .iter()
            .all(|text| !text.contains("Binary or unsupported file")),
        "unreadable RAR should not render binary fallback: {line_texts:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn rar_preview_uses_external_listing_when_available() {
    let root = temp_path("rar-preview");
    fs::create_dir_all(root.join("docs")).expect("failed to create docs dir");
    fs::create_dir_all(root.join("src")).expect("failed to create src dir");
    fs::write(root.join("docs/readme.txt"), "hello").expect("failed to write readme");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("failed to write source");

    let path = root.join("bundle.rar");
    let status = Command::new("7z")
        .current_dir(&root)
        .arg("a")
        .arg("-t7z")
        .arg(&path)
        .arg("docs")
        .arg("src")
        .status();
    let Ok(status) = status else {
        fs::remove_dir_all(&root).expect("failed to remove temp root");
        return;
    };
    if !status.success() {
        fs::remove_dir_all(&root).expect("failed to remove temp root");
        return;
    }

    let preview = build_preview(&file_entry(path));
    let line_texts: Vec<_> = preview.lines.iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("RAR archive"));
    assert!(line_texts.iter().any(|text| text == "Details"));
    assert!(line_texts.iter().any(|text| text == "Contents"));
    assert!(line_texts.iter().any(|text| text.contains("docs/")));
    assert!(line_texts.iter().any(|text| text.contains("src/")));
    assert!(line_texts.iter().any(|text| text.contains("readme.txt")));
    assert!(line_texts.iter().any(|text| text.contains("main.rs")));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn rar_loading_preview_is_silent() {
    let root = temp_path("rar-loading-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("bundle.rar");
    fs::write(&path, b"not-a-real-rar").expect("failed to write rar fixture");

    let preview = loading_preview_for(&file_entry(path), &PreviewRequestOptions::Default);

    assert_eq!(preview.kind, PreviewKind::Archive);
    assert_eq!(preview.detail.as_deref(), Some("RAR archive"));
    assert!(preview.lines.is_empty());

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
