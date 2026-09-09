use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("elio-create-archive-{label}-{unique}"))
}

#[test]
fn normalizes_zip_names() {
    assert_eq!(
        normalize_archive_output_name("backup").unwrap(),
        ("backup.zip".to_string(), CreateArchiveFormat::Zip)
    );
    assert_eq!(
        normalize_archive_output_name("backup.zip").unwrap(),
        ("backup.zip".to_string(), CreateArchiveFormat::Zip)
    );
    assert_eq!(
        normalize_archive_output_name("backup.tar").unwrap(),
        ("backup.tar".to_string(), CreateArchiveFormat::Tar)
    );
    assert_eq!(
        normalize_archive_output_name("backup.tar.gz").unwrap(),
        ("backup.tar.gz".to_string(), CreateArchiveFormat::TarGzip)
    );
    assert_eq!(
        normalize_archive_output_name("backup.tgz").unwrap(),
        ("backup.tgz".to_string(), CreateArchiveFormat::TarGzip)
    );
    assert_eq!(
        normalize_archive_output_name("backup.tar.xz").unwrap(),
        ("backup.tar.xz".to_string(), CreateArchiveFormat::TarXz)
    );
    assert_eq!(
        normalize_archive_output_name("backup.txz").unwrap(),
        ("backup.txz".to_string(), CreateArchiveFormat::TarXz)
    );
    assert_eq!(
        normalize_archive_output_name("backup.tar.bz2").unwrap(),
        ("backup.tar.bz2".to_string(), CreateArchiveFormat::TarBzip2)
    );
    assert_eq!(
        normalize_archive_output_name("backup.tbz2").unwrap(),
        ("backup.tbz2".to_string(), CreateArchiveFormat::TarBzip2)
    );
    assert_eq!(
        normalize_archive_output_name("backup.tbz").unwrap(),
        ("backup.tbz".to_string(), CreateArchiveFormat::TarBzip2)
    );
    assert_eq!(
        normalize_archive_output_name("backup.7z").unwrap(),
        ("backup.7z".to_string(), CreateArchiveFormat::SevenZip)
    );
    assert_eq!(
        normalize_archive_output_name("backup.z")
            .unwrap_err()
            .to_string(),
        "Supported: ZIP, 7Z, TAR, TAR.GZ, TAR.XZ, and TAR.BZ2"
    );
    assert_eq!(
        normalize_archive_output_name("../backup.zip")
            .unwrap_err()
            .to_string(),
        "Use a filename, not a path"
    );
}

#[test]
fn creates_zip_with_top_level_names() {
    let root = temp_path("top-level");
    fs::create_dir_all(root.join("src/nested")).unwrap();
    fs::write(root.join("src/lib.rs"), "lib").unwrap();
    fs::write(root.join("src/nested/mod.rs"), "mod").unwrap();
    fs::write(root.join("README.md"), "readme").unwrap();

    let plan = plan_create_zip_archive(
        &root,
        vec![root.join("README.md"), root.join("src")],
        "archive.zip",
    )
    .unwrap();
    create_zip_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.zip")).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut names = (0..archive.len())
        .map(|index| archive.by_index(index).unwrap().name().to_string())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        vec![
            "README.md",
            "src/",
            "src/lib.rs",
            "src/nested/",
            "src/nested/mod.rs"
        ]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn creates_seven_zip_with_top_level_names() {
    let root = temp_path("seven-zip-top-level");
    fs::create_dir_all(root.join("src/nested")).unwrap();
    fs::write(root.join("src/lib.rs"), "lib").unwrap();
    fs::write(root.join("src/nested/mod.rs"), "mod").unwrap();
    fs::write(root.join("README.md"), "readme").unwrap();

    let plan = plan_create_archive(
        &root,
        vec![root.join("README.md"), root.join("src")],
        "archive.7z",
        CreateArchiveOptions::default(),
    )
    .unwrap();
    create_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.7z")).unwrap();
    let mut archive = sevenz_rust2::ArchiveReader::new(file, SevenZipPassword::empty()).unwrap();
    let mut names = Vec::new();
    archive
        .for_each_entries(|entry, _reader| {
            names.push(entry.name().to_string());
            Ok(true)
        })
        .unwrap();
    names.sort();
    assert_eq!(
        names,
        vec![
            "README.md",
            "src",
            "src/lib.rs",
            "src/nested",
            "src/nested/mod.rs"
        ]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn creates_encrypted_seven_zip() {
    let root = temp_path("seven-zip-password");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("secret.txt"), "secret").unwrap();
    let password = root.display().to_string();
    let options = CreateArchiveOptions {
        format: CreateArchiveFormat::SevenZip,
        encryption: ArchiveEncryption::Password(ArchivePassword::new(&password)),
    };
    let plan =
        plan_create_archive(&root, vec![root.join("secret.txt")], "secret.7z", options).unwrap();
    create_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("secret.7z")).unwrap();
    let mut archive =
        sevenz_rust2::ArchiveReader::new(file, SevenZipPassword::new(&password)).unwrap();
    let mut contents = String::new();
    archive
        .for_each_entries(|entry, reader| {
            assert_eq!(entry.name(), "secret.txt");
            reader.read_to_string(&mut contents).unwrap();
            Ok(true)
        })
        .unwrap();
    assert_eq!(contents, "secret");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn creates_tar_with_top_level_names() {
    let root = temp_path("tar-top-level");
    fs::create_dir_all(root.join("src/nested")).unwrap();
    fs::write(root.join("src/lib.rs"), "lib").unwrap();
    fs::write(root.join("src/nested/mod.rs"), "mod").unwrap();
    fs::write(root.join("README.md"), "readme").unwrap();

    let plan = plan_create_archive(
        &root,
        vec![root.join("README.md"), root.join("src")],
        "archive.tar",
        CreateArchiveOptions::default(),
    )
    .unwrap();
    create_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.tar")).unwrap();
    let mut archive = tar::Archive::new(file);
    let mut names = archive
        .entries()
        .unwrap()
        .map(|entry| entry.unwrap().path().unwrap().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        vec![
            "README.md",
            "src",
            "src/lib.rs",
            "src/nested",
            "src/nested/mod.rs"
        ]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn creates_tar_with_symlink_entries() {
    use std::os::unix::fs::symlink;

    let root = temp_path("tar-symlink");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("target.txt"), "target").unwrap();
    symlink("target.txt", root.join("link.txt")).unwrap();
    symlink("missing.txt", root.join("broken.txt")).unwrap();

    let plan = plan_create_archive(
        &root,
        vec![root.join("link.txt"), root.join("broken.txt")],
        "archive.tar",
        CreateArchiveOptions::default(),
    )
    .unwrap();
    create_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.tar")).unwrap();
    let mut archive = tar::Archive::new(file);
    let mut entries = archive
        .entries()
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.path().unwrap().to_string_lossy().to_string(),
                entry.header().entry_type(),
                entry
                    .link_name()
                    .unwrap()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        entries,
        vec![
            (
                "broken.txt".to_string(),
                tar::EntryType::Symlink,
                "missing.txt".to_string()
            ),
            (
                "link.txt".to_string(),
                tar::EntryType::Symlink,
                "target.txt".to_string()
            )
        ]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn creates_tar_with_portable_internal_absolute_symlink() {
    use std::os::unix::fs::symlink;

    let root = temp_path("tar-absolute-internal-symlink");
    let folder = root.join("folder");
    fs::create_dir_all(folder.join("nested")).unwrap();
    fs::write(folder.join("target.txt"), "target").unwrap();
    symlink(folder.join("target.txt"), folder.join("nested/link.txt")).unwrap();

    let plan = plan_create_archive(
        &root,
        vec![folder.clone()],
        "archive.tar",
        CreateArchiveOptions::default(),
    )
    .unwrap();
    create_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.tar")).unwrap();
    let mut archive = tar::Archive::new(file);
    let link_target = archive
        .entries()
        .unwrap()
        .map(|entry| entry.unwrap())
        .find_map(|entry| {
            (entry.path().unwrap().as_ref() == Path::new("folder/nested/link.txt"))
                .then(|| entry.link_name().unwrap().unwrap().into_owned())
        })
        .unwrap();
    assert_eq!(link_target, Path::new("../target.txt"));
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn creates_zip_with_portable_internal_absolute_symlink() {
    use std::io::Read;
    use std::os::unix::fs::symlink;

    let root = temp_path("zip-absolute-internal-symlink");
    let folder = root.join("folder");
    fs::create_dir_all(folder.join("nested")).unwrap();
    fs::write(folder.join("target.txt"), "target").unwrap();
    symlink(folder.join("target.txt"), folder.join("nested/link.txt")).unwrap();

    let plan = plan_create_zip_archive(&root, vec![folder], "archive.zip").unwrap();
    create_zip_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.zip")).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut entry = archive.by_name("folder/nested/link.txt").unwrap();
    assert_eq!(entry.unix_mode().unwrap() & 0o170000, 0o120000);
    let mut target = String::new();
    entry.read_to_string(&mut target).unwrap();
    assert_eq!(target, "../target.txt");
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn creates_seven_zip_with_portable_internal_absolute_symlink() {
    use std::os::unix::fs::symlink;

    let root = temp_path("seven-zip-absolute-internal-symlink");
    let folder = root.join("folder");
    fs::create_dir_all(folder.join("nested")).unwrap();
    fs::write(folder.join("target.txt"), "target").unwrap();
    symlink(folder.join("target.txt"), folder.join("nested/link.txt")).unwrap();

    let plan = plan_create_archive(
        &root,
        vec![folder],
        "archive.7z",
        CreateArchiveOptions::default(),
    )
    .unwrap();
    create_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.7z")).unwrap();
    let mut archive = sevenz_rust2::ArchiveReader::new(file, SevenZipPassword::empty()).unwrap();
    let mut target = String::new();
    let mut attrs = 0;
    archive
        .for_each_entries(|entry, reader| {
            if entry.name() == "folder/nested/link.txt" {
                attrs = entry.windows_attributes;
                reader.read_to_string(&mut target).unwrap();
            }
            Ok(true)
        })
        .unwrap();
    assert_eq!((attrs >> 16) & 0o170000, 0o120000);
    assert_eq!(target, "../target.txt");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn creates_tar_gzip_archive() {
    let root = temp_path("tar-gzip");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("README.md"), "readme").unwrap();

    let plan = plan_create_archive(
        &root,
        vec![root.join("README.md")],
        "archive.tgz",
        CreateArchiveOptions::default(),
    )
    .unwrap();
    create_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.tgz")).unwrap();
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    assert_tar_readme(&mut archive);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn creates_tar_xz_archive() {
    let root = temp_path("tar-xz");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("README.md"), "readme").unwrap();

    let plan = plan_create_archive(
        &root,
        vec![root.join("README.md")],
        "archive.txz",
        CreateArchiveOptions::default(),
    )
    .unwrap();
    create_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.txz")).unwrap();
    let decoder = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    assert_tar_readme(&mut archive);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn creates_tar_bzip2_archive() {
    let root = temp_path("tar-bzip2");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("README.md"), "readme").unwrap();

    let plan = plan_create_archive(
        &root,
        vec![root.join("README.md")],
        "archive.tbz2",
        CreateArchiveOptions::default(),
    )
    .unwrap();
    create_archive(&plan, |_| {}, || false).unwrap();

    let file = File::open(root.join("archive.tbz2")).unwrap();
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    assert_tar_readme(&mut archive);
    let _ = fs::remove_dir_all(root);
}

fn assert_tar_readme<R: std::io::Read>(archive: &mut tar::Archive<R>) {
    let mut entry = archive.entries().unwrap().next().unwrap().unwrap();
    assert_eq!(entry.path().unwrap().to_string_lossy(), "README.md");
    let mut contents = String::new();
    use std::io::Read;
    entry.read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "readme");
}

#[test]
fn rejects_tar_passwords() {
    let root = temp_path("tar-password");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("README.md"), "readme").unwrap();
    let password = root.file_name().unwrap().to_string_lossy().into_owned();
    let error = plan_create_archive(
        &root,
        vec![root.join("README.md")],
        "archive.tar",
        CreateArchiveOptions {
            format: CreateArchiveFormat::Zip,
            encryption: ArchiveEncryption::Password(ArchivePassword::new(&password)),
        },
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "Password not supported for this format");
    let _ = fs::remove_file(root.join("README.md"));
}

#[test]
fn rejects_output_inside_selected_folder() {
    let root = temp_path("self");
    fs::create_dir_all(root.join("src")).unwrap();
    let error = plan_create_zip_archive(&root.join("src"), vec![root.join("src")], "archive.zip")
        .unwrap_err();
    assert_eq!(error.to_string(), "Output is inside selected folder");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_duplicate_top_level_names() {
    let root = temp_path("duplicate");
    fs::create_dir_all(root.join("left")).unwrap();
    fs::create_dir_all(root.join("right")).unwrap();
    fs::write(root.join("left/item.txt"), "left").unwrap();
    fs::write(root.join("right/item.txt"), "right").unwrap();
    let error = plan_create_zip_archive(
        &root,
        vec![root.join("left/item.txt"), root.join("right/item.txt")],
        "archive.zip",
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Archive would contain duplicate item.txt"
    );
    let _ = fs::remove_dir_all(root);
}
