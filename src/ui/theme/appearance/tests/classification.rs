use super::super::{resolve::builtin_classify_path, rules::rgb};
use super::*;

fn write_temp_file(label: &str, file_name: &str, contents: &str) -> (PathBuf, PathBuf) {
    let root = temp_path(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(file_name);
    fs::write(&path, contents).expect("failed to write temp file");
    (root, path)
}

#[test]
fn generic_lock_files_use_file_lock_icon() {
    let theme = Theme::default_theme();
    let resolved = theme.resolve(Path::new("custom.lock"), EntryKind::File);
    assert_eq!(resolved.class, FileClass::Data);
    assert_eq!(resolved.icon, "󰈡");
    assert_eq!(resolved.color, rgb(89, 222, 148));

    let cargo = theme.resolve(Path::new("Cargo.lock"), EntryKind::File);
    assert_eq!(cargo.icon, "󰈡");

    let package_lock = theme.resolve(Path::new("package-lock.json"), EntryKind::File);
    assert_eq!(package_lock.icon, "󰈡");

    let poetry = theme.resolve(Path::new("poetry.lock"), EntryKind::File);
    assert_eq!(poetry.icon, "󰈡");
}

#[test]
fn detected_license_files_use_license_class_appearance() {
    let theme = Theme::default_theme();
    let (root, path) = write_temp_file(
        "license-appearance",
        "LICENSE.md",
        "# SPDX-License-Identifier: Apache-2.0\n\nFixture license notes.\n",
    );

    let resolved = theme.resolve(&path, EntryKind::File);

    assert_eq!(resolved.class, FileClass::License);
    assert_eq!(resolved.icon, "󰿃");
    assert_eq!(resolved.color, rgb(245, 216, 91));
    assert_eq!(
        specific_type_label(&path, EntryKind::File),
        Some("Apache License 2.0")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn filename_alone_does_not_force_license_appearance() {
    let theme = Theme::default_theme();
    let (root, path) = write_temp_file(
        "license-false-positive",
        "LICENSE",
        "shopping list\n- apples\n- oranges\n",
    );

    let resolved = theme.resolve(&path, EntryKind::File);

    assert_eq!(resolved.class, FileClass::File);
    assert_ne!(resolved.icon, "󰿃");
    assert_eq!(specific_type_label(&path, EntryKind::File), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn resolve_entry_cache_respects_entry_metadata_when_builtin_class_changes() {
    let (root, path) = write_temp_file(
        "appearance-cache",
        "third-party.txt",
        "SPDX-License-Identifier: Apache-2.0\n",
    );

    let metadata = fs::metadata(&path).expect("metadata should exist");
    let mut entry = Entry {
        path: path.clone(),
        name: "third-party.txt".to_string(),
        name_key: "third-party.txt".to_string(),
        kind: EntryKind::File,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        readonly: false,
    };

    let initial = resolve_entry(&entry);
    assert_eq!(initial.class, FileClass::License);

    fs::write(&path, "shopping list\n- apples\n- oranges\n").expect("failed to rewrite file");
    let metadata = fs::metadata(&path).expect("updated metadata should exist");
    entry.size = metadata.len();
    entry.modified = metadata.modified().ok();

    let updated = resolve_entry(&entry);
    assert_eq!(updated.class, FileClass::Document);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn type_labels_cover_supported_special_files() {
    assert_eq!(
        specific_type_label(Path::new("cover.xcf"), EntryKind::File),
        Some("GIMP image")
    );
    assert_eq!(
        specific_type_label(Path::new("disk.iso"), EntryKind::File),
        Some("ISO disk image")
    );
    assert_eq!(
        specific_type_label(Path::new("package.rpm"), EntryKind::File),
        Some("RPM package")
    );
    assert_eq!(
        specific_type_label(Path::new("ubuntu.torrent"), EntryKind::File),
        Some("BitTorrent file")
    );
    assert_eq!(
        specific_type_label(Path::new("signatures.hash"), EntryKind::File),
        Some("Hash file")
    );
    assert_eq!(
        specific_type_label(Path::new("release.sha1"), EntryKind::File),
        Some("SHA-1 checksum")
    );
    assert_eq!(
        specific_type_label(Path::new("release.sha256"), EntryKind::File),
        Some("SHA-256 checksum")
    );
    assert_eq!(
        specific_type_label(Path::new("release.sha512"), EntryKind::File),
        Some("SHA-512 checksum")
    );
    assert_eq!(
        specific_type_label(Path::new("release.md5"), EntryKind::File),
        Some("MD5 checksum")
    );
    assert_eq!(
        specific_type_label(Path::new("server.log"), EntryKind::File),
        Some("Log file")
    );
    assert_eq!(
        specific_type_label(Path::new("movie.srt"), EntryKind::File),
        Some("SubRip subtitles")
    );
    assert_eq!(
        specific_type_label(Path::new("bindings.keys"), EntryKind::File),
        Some("Keys file")
    );
    assert_eq!(
        specific_type_label(Path::new("identity.p12"), EntryKind::File),
        Some("PKCS#12 certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("identity.pfx"), EntryKind::File),
        Some("PKCS#12 certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("fullchain.pem"), EntryKind::File),
        Some("PEM certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("server.crt"), EntryKind::File),
        Some("Certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("server.cer"), EntryKind::File),
        Some("Certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("server.csr"), EntryKind::File),
        Some("Certificate signing request")
    );
    assert_eq!(
        specific_type_label(Path::new("id_ed25519.key"), EntryKind::File),
        Some("Private key")
    );
    assert_eq!(
        specific_type_label(Path::new("package.deb"), EntryKind::File),
        Some("Debian package")
    );
    assert_eq!(
        specific_type_label(Path::new("app.apk"), EntryKind::File),
        Some("Android package")
    );
    assert_eq!(
        specific_type_label(Path::new("bundle.aab"), EntryKind::File),
        Some("Android App Bundle")
    );
    assert_eq!(
        specific_type_label(Path::new("deck.apkg"), EntryKind::File),
        Some("Anki package")
    );
    assert_eq!(
        specific_type_label(Path::new("archive.zst"), EntryKind::File),
        Some("Zstandard archive")
    );
    assert_eq!(
        specific_type_label(Path::new("theme.zest"), EntryKind::File),
        Some("Zest archive")
    );
    assert_eq!(
        specific_type_label(Path::new("Elio.AppImage"), EntryKind::File),
        Some("AppImage bundle")
    );
    assert_eq!(
        specific_type_label(Path::new("PKGBUILD"), EntryKind::File),
        Some("Arch build script")
    );
    assert_eq!(
        specific_type_label(Path::new("setup.exe"), EntryKind::File),
        Some("Windows executable")
    );
    assert_eq!(
        specific_type_label(Path::new("app.jar"), EntryKind::File),
        Some("Java archive")
    );
}

#[test]
fn builtin_classification_covers_new_special_file_types() {
    assert_eq!(
        builtin_classify_path(Path::new("cover.xcf"), EntryKind::File),
        FileClass::Image
    );
    assert_eq!(
        builtin_classify_path(Path::new("favicon.ico"), EntryKind::File),
        FileClass::Image
    );
    assert_eq!(
        builtin_classify_path(Path::new("disk.iso"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("package.rpm"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("package.deb"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("app.apk"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("bundle.aab"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("deck.apkg"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("archive.zst"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("app.jar"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("archive.zest"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("Elio.AppImage"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("ubuntu.torrent"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("signatures.hash"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("release.sha1"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("release.sha256"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("release.sha512"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("release.md5"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("server.log"), EntryKind::File),
        FileClass::Document
    );
    assert_eq!(
        builtin_classify_path(Path::new("movie.srt"), EntryKind::File),
        FileClass::Document
    );
    assert_eq!(
        builtin_classify_path(Path::new("bindings.keys"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("identity.p12"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("identity.pfx"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("fullchain.pem"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("server.crt"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("server.cer"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("server.csr"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("id_ed25519.key"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("PKGBUILD"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("setup.exe"), EntryKind::File),
        FileClass::File
    );
}
