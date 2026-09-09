use super::*;
use std::{
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("elio-archive-extract-{label}-{unique}"))
}

fn archive_test_password(root: &Path) -> String {
    root.file_name()
        .expect("temp root should have a file name")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn detects_archive_formats() {
    let cases = [
        ("app.zip", ExtractFormat::Zip),
        ("app.tar", ExtractFormat::Tar),
        ("app.7z", ExtractFormat::SevenZip),
        ("app.rar", ExtractFormat::Rar),
        ("app.tar.gz", ExtractFormat::TarGzip),
        ("app.tgz", ExtractFormat::TarGzip),
        ("app.tar.xz", ExtractFormat::TarXz),
        ("app.txz", ExtractFormat::TarXz),
        ("app.tar.bz2", ExtractFormat::TarBzip2),
        ("app.tbz2", ExtractFormat::TarBzip2),
        ("app.tbz", ExtractFormat::TarBzip2),
        ("app.tar.zst", ExtractFormat::TarZstd),
        ("app.tzst", ExtractFormat::TarZstd),
    ];
    for (name, expected) in cases {
        assert_eq!(ExtractFormat::detect(Path::new(name)), Some(expected));
    }
}

#[test]
fn maps_formats_to_backends() {
    assert_eq!(ExtractFormat::Zip.backend(), ExtractBackend::Zip);
    assert_eq!(
        ExtractFormat::Tar.backend(),
        ExtractBackend::Tar(ExtractFormat::Tar)
    );
    assert_eq!(ExtractFormat::SevenZip.backend(), ExtractBackend::SevenZip);
    assert_eq!(
        ExtractFormat::Rar.backend(),
        ExtractBackend::ExternalSevenZip
    );
}

#[test]
fn derives_destination_stems() {
    for name in [
        "app.zip",
        "app.tar",
        "app.7z",
        "app.rar",
        "app.tar.gz",
        "app.tgz",
        "app.tar.xz",
        "app.txz",
        "app.tar.bz2",
        "app.tbz2",
        "app.tbz",
        "app.tar.zst",
        "app.tzst",
    ] {
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new(name)).as_deref(),
            Some("app")
        );
    }
}

#[test]
fn unique_destination_uses_paste_style_suffix() {
    let root = temp_path("unique-destination");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("app_1")).unwrap();
    assert_eq!(unique_destination(&root, "app"), root.join("app_2"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_escaping_paths() {
    let dest = Path::new("/tmp/out");
    assert!(checked_output_path(dest, Path::new("../evil")).is_err());
    assert!(checked_output_path(dest, Path::new("/evil")).is_err());
    assert_eq!(
        checked_output_path(dest, Path::new("ok/file.txt")).unwrap(),
        dest.join("ok/file.txt")
    );
}

#[test]
fn parses_external_7z_listing_paths() {
    let output = r#"
Path = sample.rar
Type = Rar5
----------
Path = dir
Folder = +

Path = dir/file.txt
Folder = -
Size = 5
"#;

    assert_eq!(
        parse_external_seven_zip_entries(output),
        vec!["dir".to_string(), "dir/file.txt".to_string()]
    );
}

#[test]
fn rejects_unsafe_external_7z_listing_paths() {
    let dest = Path::new("/tmp/out");

    assert!(validate_external_entry_path(dest, "ok/file.txt").is_ok());
    let error = validate_external_entry_path(dest, "../evil.txt").unwrap_err();
    assert!(matches!(error, ExtractError::UnsafeArchivePath));
    let error = validate_external_entry_path(dest, "/evil.txt").unwrap_err();
    assert!(matches!(error, ExtractError::UnsafeArchivePath));
}

#[test]
fn extracts_zip_archive() {
    let root = temp_path("zip");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.zip");
    {
        let file = File::create(&archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.add_directory("dir/", options).unwrap();
        zip.start_file("dir/file.txt", options).unwrap();
        zip.write_all(b"hello").unwrap();
        zip.finish().unwrap();
    }
    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();
    assert_eq!(summary.completed, 2);
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn extracts_safe_zip_symlink() {
    let root = temp_path("zip-safe-link");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.zip");
    {
        let file = File::create(&archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("target.txt", options).unwrap();
        zip.write_all(b"hello").unwrap();
        let link_options = options.unix_permissions(0o777);
        zip.add_symlink("link.txt", "target.txt", link_options)
            .unwrap();
        zip.finish().unwrap();
    }
    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();
    assert_eq!(summary.completed, 2);
    assert_eq!(summary.skipped_links, 0);
    let link_path = root.join("sample/link.txt");
    assert!(
        fs::symlink_metadata(&link_path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_link(link_path).unwrap(), Path::new("target.txt"));
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn skips_unsafe_zip_symlinks() {
    let root = temp_path("zip-unsafe-link");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.zip");
    {
        let file = File::create(&archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        let link_options = options.unix_permissions(0o777);
        zip.add_symlink("absolute", "/etc/passwd", link_options)
            .unwrap();
        zip.add_symlink("escape", "../escape", link_options)
            .unwrap();
        zip.finish().unwrap();
    }
    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();
    assert_eq!(summary.completed, 2);
    assert_eq!(summary.skipped_links, 2);
    assert!(!root.join("sample/absolute").exists());
    assert!(!root.join("sample/escape").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn corrupt_zip_does_not_request_password() {
    let root = temp_path("zip-corrupt");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.zip");
    fs::write(&archive_path, b"not a zip").unwrap();

    let plan = plan_extract(&archive_path).unwrap();
    let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

    assert!(matches!(error, ExtractError::Other(_)));
    assert!(!plan.dest_dir.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn encrypted_zip_requires_password() {
    let root = temp_path("zip-encrypted-required");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.zip");
    let password = archive_test_password(&root);
    write_encrypted_zip(&archive_path, &password, ZipEncryption::Deprecated);

    let plan = plan_extract(&archive_path).unwrap();
    let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

    assert!(
        matches!(error, ExtractError::PasswordRequired),
        "expected password required, got {error:?}"
    );
    assert!(!plan.dest_dir.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn encrypted_zip_rejects_wrong_password() {
    let root = temp_path("zip-encrypted-wrong");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.zip");
    let password = archive_test_password(&root);
    write_encrypted_zip(&archive_path, &password, ZipEncryption::Deprecated);

    let plan = plan_extract(&archive_path).unwrap();
    let error = extract_archive_with_password(
        &plan,
        Some(&ArchivePassword::new(format!("{password}-wrong"))),
        |_| {},
        || false,
    )
    .unwrap_err();

    assert!(
        matches!(error, ExtractError::BadPassword),
        "expected bad password, got {error:?}"
    );
    assert!(!plan.dest_dir.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn encrypted_zip_extracts_with_password() {
    let root = temp_path("zip-encrypted-ok");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.zip");
    let password = archive_test_password(&root);
    write_encrypted_zip(&archive_path, &password, ZipEncryption::Deprecated);

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(
        &plan,
        Some(&ArchivePassword::new(password)),
        |_| {},
        || false,
    )
    .unwrap();

    assert_eq!(summary.completed, 1);
    assert_eq!(summary.total, Some(1));
    assert_eq!(
        fs::read_to_string(root.join("sample/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn aes_encrypted_zip_extracts_with_password() {
    let root = temp_path("zip-aes-encrypted-ok");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.zip");
    let password = archive_test_password(&root);
    write_encrypted_zip(&archive_path, &password, ZipEncryption::Aes256);

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(
        &plan,
        Some(&ArchivePassword::new(password)),
        |_| {},
        || false,
    )
    .unwrap();

    assert_eq!(summary.completed, 1);
    assert_eq!(summary.total, Some(1));
    assert_eq!(
        fs::read_to_string(root.join("sample/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extracts_tar_archive() {
    let root = temp_path("tar");
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source");
    fs::create_dir_all(source.join("dir")).unwrap();
    fs::write(source.join("dir/file.txt"), "hello").unwrap();
    let archive_path = root.join("sample.tar");
    {
        let file = File::create(&archive_path).unwrap();
        let mut tar = tar::Builder::new(file);
        tar.append_dir("dir", source.join("dir")).unwrap();
        tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
            .unwrap();
        tar.finish().unwrap();
    }
    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();
    assert_eq!(summary.completed, 2);
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn extracts_safe_tar_symlink() {
    let root = temp_path("tar-safe-link");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.tar");
    {
        let file = File::create(&archive_path).unwrap();
        let mut tar = tar::Builder::new(file);
        fs::write(root.join("target.txt"), "hello").unwrap();
        tar.append_path_with_name(root.join("target.txt"), "target.txt")
            .unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        tar.append_link(&mut header, "link.txt", "target.txt")
            .unwrap();
        tar.finish().unwrap();
    }

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(summary.skipped_links, 0);
    assert_eq!(
        fs::read_link(root.join("sample/link.txt")).unwrap(),
        Path::new("target.txt")
    );
    assert_eq!(
        fs::read_to_string(root.join("sample/target.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn skips_unsafe_tar_symlinks() {
    let root = temp_path("tar-unsafe-link");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.tar");
    {
        let file = File::create(&archive_path).unwrap();
        let mut tar = tar::Builder::new(file);
        for (path, target) in [
            ("absolute", "/tmp/outside"),
            ("parent", "../outside"),
            ("nested/parent", "../../outside"),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            tar.append_link(&mut header, path, target).unwrap();
        }
        tar.finish().unwrap();
    }

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

    assert_eq!(summary.completed, 3);
    assert_eq!(summary.skipped_links, 3);
    assert!(!root.join("sample/absolute").exists());
    assert!(!root.join("sample/parent").exists());
    assert!(!root.join("sample/nested/parent").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn tar_symlink_parent_does_not_redirect_later_entries() {
    let root = temp_path("tar-link-parent");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.tar");
    {
        let file = File::create(&archive_path).unwrap();
        let mut tar = tar::Builder::new(file);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        tar.append_link(&mut header, "dir", "../outside").unwrap();
        fs::write(root.join("file.txt"), "safe").unwrap();
        tar.append_path_with_name(root.join("file.txt"), "dir/file.txt")
            .unwrap();
        tar.finish().unwrap();
    }

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(summary.skipped_links, 1);
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "safe"
    );
    assert!(!root.join("outside/file.txt").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extracts_tar_gzip_archive() {
    let root = temp_path("tgz");
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source");
    fs::create_dir_all(source.join("dir")).unwrap();
    fs::write(source.join("dir/file.txt"), "hello").unwrap();
    let archive_path = root.join("sample.tar.gz");
    {
        let file = File::create(&archive_path).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.append_dir("dir", source.join("dir")).unwrap();
        tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
            .unwrap();
        tar.finish().unwrap();
    }
    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();
    assert_eq!(summary.completed, 2);
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extracts_tar_xz_archive() {
    let root = temp_path("txz");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.tar.xz");
    write_compressed_tar(&archive_path, |file| {
        Ok(xz2::write::XzEncoder::new(file, 6))
    });

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extracts_tar_bzip2_archive() {
    let root = temp_path("tbz2");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.tar.bz2");
    write_compressed_tar(&archive_path, |file| {
        Ok(bzip2::write::BzEncoder::new(
            file,
            bzip2::Compression::default(),
        ))
    });

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extracts_tar_zstd_archive() {
    let root = temp_path("tzst");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.tar.zst");
    write_zstd_tar(&archive_path);

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extracts_seven_zip_archive() {
    let root = temp_path("7z");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.7z");
    write_seven_zip(
        &archive_path,
        &[("dir", None), ("dir/file.txt", Some(b"hello"))],
    );

    let plan = plan_extract(&archive_path).unwrap();
    let mut progress = Vec::new();
    let summary =
        extract_archive_with_password(&plan, None, |update| progress.push(update), || false)
            .unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(summary.total, Some(2));
    assert_eq!(
        progress
            .last()
            .map(|update| (update.completed, update.total)),
        Some((2, Some(2)))
    );
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn extracts_safe_seven_zip_symlink() {
    let root = temp_path("7z-safe-link");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.7z");
    write_seven_zip_with_symlinks(
        &archive_path,
        &[
            ("target.txt", Some(b"hello"), false),
            ("link.txt", Some(b"target.txt"), true),
        ],
    );

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(summary.skipped_links, 0);
    let link_path = root.join("sample/link.txt");
    assert!(
        fs::symlink_metadata(&link_path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_link(link_path).unwrap(), Path::new("target.txt"));
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn skips_unsafe_seven_zip_symlinks() {
    let root = temp_path("7z-unsafe-link");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.7z");
    write_seven_zip_with_symlinks(
        &archive_path,
        &[
            ("absolute", Some(b"/etc/passwd"), true),
            ("escape", Some(b"../escape"), true),
        ],
    );

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(summary.skipped_links, 2);
    assert!(!root.join("sample/absolute").exists());
    assert!(!root.join("sample/escape").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extracts_rar_with_external_seven_zip_fallback() {
    if !external_seven_zip_available() {
        return;
    }
    let root = temp_path("rar-external-7z");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.rar");
    write_seven_zip(
        &archive_path,
        &[("dir", None), ("dir/file.txt", Some(b"hello"))],
    );

    let plan = plan_extract(&archive_path).unwrap();
    assert_eq!(plan.backend, ExtractBackend::ExternalSevenZip);
    let mut progress = Vec::new();
    let summary =
        extract_archive_with_password(&plan, None, |update| progress.push(update), || false)
            .unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(summary.total, Some(2));
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn external_seven_zip_rejects_unsafe_paths_before_extracting() {
    if !external_seven_zip_available() {
        return;
    }
    let root = temp_path("rar-external-7z-slip");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.rar");
    write_seven_zip(&archive_path, &[("../evil.txt", Some(b"bad"))]);

    let plan = plan_extract(&archive_path).unwrap();
    let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

    assert!(matches!(error, ExtractError::UnsafeArchivePath));
    assert!(!root.join("evil.txt").exists());
    assert!(!plan.dest_dir.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn external_seven_zip_command_handles_large_failure_output() {
    let mut command = Command::new("sh");
    command.arg("-c").arg(
        "i=0; while [ $i -lt 5000 ]; do printf 'ERROR: Wrong password\\n' >&2; i=$((i+1)); done; exit 2",
    );

    let error = run_external_seven_zip_command(command, true, &mut || false).unwrap_err();

    assert!(matches!(error, ExtractError::BadPassword));
}

#[test]
fn encrypted_rar_requires_password_before_staging() {
    if !external_seven_zip_available() {
        return;
    }
    let root = temp_path("rar-encrypted-required");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.rar");
    let password = archive_test_password(&root);
    write_encrypted_seven_zip(
        &archive_path,
        &password,
        &[("dir", None), ("dir/file.txt", Some(b"hello"))],
    );

    let plan = plan_extract(&archive_path).unwrap();
    let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

    assert!(matches!(error, ExtractError::PasswordRequired));
    assert!(!plan.dest_dir.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn encrypted_rar_rejects_wrong_password_before_staging() {
    if !external_seven_zip_available() {
        return;
    }
    let root = temp_path("rar-encrypted-wrong");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.rar");
    let password = archive_test_password(&root);
    write_encrypted_seven_zip(
        &archive_path,
        &password,
        &[("dir", None), ("dir/file.txt", Some(b"hello"))],
    );

    let plan = plan_extract(&archive_path).unwrap();
    let error = extract_archive_with_password(
        &plan,
        Some(&ArchivePassword::new(format!("{password}-wrong"))),
        |_| {},
        || false,
    )
    .unwrap_err();

    assert!(matches!(error, ExtractError::BadPassword));
    assert!(!plan.dest_dir.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn encrypted_rar_extracts_with_password() {
    if !external_seven_zip_available() {
        return;
    }
    let root = temp_path("rar-encrypted-ok");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.rar");
    let password = archive_test_password(&root);
    write_encrypted_seven_zip(
        &archive_path,
        &password,
        &[("dir", None), ("dir/file.txt", Some(b"hello"))],
    );

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(
        &plan,
        Some(&ArchivePassword::new(password)),
        |_| {},
        || false,
    )
    .unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(summary.total, Some(2));
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn archive_password_debug_is_redacted() {
    let password = std::any::type_name::<ArchivePassword>();
    let rendered = format!("{:?}", ArchivePassword::new(password));

    assert_eq!(rendered, "ArchivePassword(<redacted>)");
    assert!(!rendered.contains(password));
}

#[test]
fn encrypted_seven_zip_requires_password() {
    let root = temp_path("7z-encrypted-required");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.7z");
    let password = archive_test_password(&root);
    write_encrypted_seven_zip(
        &archive_path,
        &password,
        &[("dir", None), ("dir/file.txt", Some(b"hello"))],
    );

    let plan = plan_extract(&archive_path).unwrap();
    let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

    assert!(matches!(error, ExtractError::PasswordRequired));
    assert!(!plan.dest_dir.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn encrypted_seven_zip_rejects_wrong_password() {
    let root = temp_path("7z-encrypted-wrong");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.7z");
    let password = archive_test_password(&root);
    let wrong_password = format!("{password}-wrong");
    write_encrypted_seven_zip(
        &archive_path,
        &password,
        &[("dir", None), ("dir/file.txt", Some(b"hello"))],
    );

    let plan = plan_extract(&archive_path).unwrap();
    let error = extract_archive_with_password(
        &plan,
        Some(&ArchivePassword::new(wrong_password)),
        |_| {},
        || false,
    )
    .unwrap_err();

    assert!(matches!(error, ExtractError::BadPassword));
    assert!(!plan.dest_dir.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn encrypted_seven_zip_extracts_with_password() {
    let root = temp_path("7z-encrypted-ok");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.7z");
    let password = archive_test_password(&root);
    write_encrypted_seven_zip(
        &archive_path,
        &password,
        &[("dir", None), ("dir/file.txt", Some(b"hello"))],
    );

    let plan = plan_extract(&archive_path).unwrap();
    let summary = extract_archive_with_password(
        &plan,
        Some(&ArchivePassword::new(password)),
        |_| {},
        || false,
    )
    .unwrap();

    assert_eq!(summary.completed, 2);
    assert_eq!(summary.total, Some(2));
    assert_eq!(
        fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
        "hello"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_escaping_seven_zip_paths() {
    let root = temp_path("7z-slip");
    fs::create_dir_all(&root).unwrap();
    let archive_path = root.join("sample.7z");
    write_seven_zip(&archive_path, &[("../evil.txt", Some(b"bad"))]);

    let plan = plan_extract(&archive_path).unwrap();
    let err = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

    assert!(err.to_string().contains("escapes the destination"));
    assert!(!root.join("evil.txt").exists());
    let _ = fs::remove_dir_all(root);
}

fn write_compressed_tar<W, D>(archive_path: &Path, encoder: D)
where
    W: Write,
    D: FnOnce(File) -> std::result::Result<W, std::io::Error>,
{
    let root = archive_path.parent().unwrap();
    let source = root.join("source");
    fs::create_dir_all(source.join("dir")).unwrap();
    fs::write(source.join("dir/file.txt"), "hello").unwrap();

    let file = File::create(archive_path).unwrap();
    let writer = encoder(file).unwrap();
    let mut tar = tar::Builder::new(writer);
    tar.append_dir("dir", source.join("dir")).unwrap();
    tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
        .unwrap();
    tar.finish().unwrap();
}

fn write_zstd_tar(archive_path: &Path) {
    let root = archive_path.parent().unwrap();
    let source = root.join("source");
    fs::create_dir_all(source.join("dir")).unwrap();
    fs::write(source.join("dir/file.txt"), "hello").unwrap();

    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        tar.append_dir("dir", source.join("dir")).unwrap();
        tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
            .unwrap();
        tar.finish().unwrap();
    }
    let compressed = zstd::stream::encode_all(tar_bytes.as_slice(), 0).unwrap();
    fs::write(archive_path, compressed).unwrap();
}

fn external_seven_zip_available() -> bool {
    available_external_seven_zip().is_ok()
}

enum ZipEncryption {
    Deprecated,
    Aes256,
}

fn write_encrypted_zip(archive_path: &Path, password: &str, encryption: ZipEncryption) {
    use zip::unstable::write::FileOptionsExt;

    let file = File::create(archive_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = match encryption {
        ZipEncryption::Deprecated => zip::write::SimpleFileOptions::default()
            .with_deprecated_encryption(password.as_bytes())
            .unwrap(),
        ZipEncryption::Aes256 => zip::write::SimpleFileOptions::default()
            .with_aes_encryption(zip::AesMode::Aes256, password),
    };
    zip.start_file("file.txt", options).unwrap();
    zip.write_all(b"hello").unwrap();
    zip.finish().unwrap();
}

fn write_seven_zip(archive_path: &Path, entries: &[(&str, Option<&[u8]>)]) {
    write_seven_zip_with_symlinks(
        archive_path,
        &entries
            .iter()
            .map(|(name, contents)| (*name, *contents, false))
            .collect::<Vec<_>>(),
    );
}

fn write_seven_zip_with_symlinks(archive_path: &Path, entries: &[(&str, Option<&[u8]>, bool)]) {
    let mut writer = sevenz_rust2::ArchiveWriter::create(archive_path).unwrap();
    for (name, contents, symlink) in entries {
        let mut entry = if contents.is_some() {
            sevenz_rust2::ArchiveEntry::new_file(name)
        } else {
            sevenz_rust2::ArchiveEntry::new_directory(name)
        };
        if *symlink {
            entry.has_windows_attributes = true;
            entry.windows_attributes = (0o120777 << 16) | 0x8020;
        }
        match contents {
            Some(contents) => {
                writer.push_archive_entry(entry, Some(*contents)).unwrap();
            }
            None => {
                writer.push_archive_entry::<&[u8]>(entry, None).unwrap();
            }
        }
    }
    writer.finish().unwrap();
}

fn write_encrypted_seven_zip(
    archive_path: &Path,
    password: &str,
    entries: &[(&str, Option<&[u8]>)],
) {
    let mut writer = sevenz_rust2::ArchiveWriter::create(archive_path).unwrap();
    writer.set_content_methods(vec![
        sevenz_rust2::encoder_options::AesEncoderOptions::new(sevenz_rust2::Password::new(
            password,
        ))
        .into(),
        sevenz_rust2::encoder_options::Lzma2Options::default().into(),
    ]);
    for (name, contents) in entries {
        let entry = if contents.is_some() {
            sevenz_rust2::ArchiveEntry::new_file(name)
        } else {
            sevenz_rust2::ArchiveEntry::new_directory(name)
        };
        match contents {
            Some(contents) => {
                writer.push_archive_entry(entry, Some(*contents)).unwrap();
            }
            None => {
                writer.push_archive_entry::<&[u8]>(entry, None).unwrap();
            }
        }
    }
    writer.finish().unwrap();
}
