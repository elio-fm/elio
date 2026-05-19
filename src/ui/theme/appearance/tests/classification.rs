use super::super::{
    resolve::{builtin_classify_browser_entry, builtin_classify_path},
    rules::rgb,
};
use super::*;
use crate::core::SymlinkInfo;

fn write_temp_file(label: &str, file_name: &str, contents: &str) -> (PathBuf, PathBuf) {
    let root = temp_path(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(file_name);
    fs::write(&path, contents).expect("failed to write temp file");
    (root, path)
}

fn test_entry(name: &str, kind: EntryKind) -> Entry {
    Entry {
        path: PathBuf::from(name),
        name: name.to_string(),
        name_key: name.to_lowercase(),
        kind,
        ..Entry::default()
    }
}

fn symlink_entry(name: &str, kind: EntryKind, target_kind: Option<EntryKind>) -> Entry {
    Entry {
        symlink: Some(SymlinkInfo {
            target: Some(PathBuf::from("target")),
            target_kind,
        }),
        ..test_entry(name, kind)
    }
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
        symlink: None,
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
fn symlinked_browser_directories_use_link_folder_appearance() {
    let entry = symlink_entry("linked", EntryKind::Directory, Some(EntryKind::Directory));
    let directory = resolve_browser_entry(&test_entry("folder", EntryKind::Directory));

    let browser = resolve_browser_entry(&entry);
    let preview = resolve_entry(&entry);

    assert_eq!(browser.class, FileClass::SymlinkDirectory);
    assert_eq!(browser.icon, "");
    assert_eq!(browser.color, directory.color);
    assert_eq!(preview.class, FileClass::SymlinkDirectory);
    assert_eq!(preview.icon, "");
    assert_eq!(preview.color, directory.color);
}

#[test]
fn symlinked_browser_files_keep_file_type_appearance() {
    let entry = symlink_entry("config.toml", EntryKind::File, Some(EntryKind::File));
    let normal_file = test_entry("config.toml", EntryKind::File);

    let resolved = resolve_browser_entry(&entry);
    let normal = resolve_browser_entry(&normal_file);

    assert_eq!(resolved.class, normal.class);
    assert_eq!(resolved.icon, normal.icon);
    assert_eq!(resolved.color, normal.color);
}

#[test]
fn broken_symlinks_use_broken_link_appearance() {
    let entry = symlink_entry("missing", EntryKind::File, None);

    let resolved = resolve_browser_entry(&entry);

    assert_eq!(resolved.class, FileClass::BrokenSymlink);
    assert_eq!(resolved.icon, "󰌺");
    assert_eq!(resolved.color, rgb(0xff, 0x85, 0x85));
}

#[test]
fn symlink_state_classes_take_precedence_over_name_rules() {
    let theme = Theme::from_config_str(
        r##"
[classes.symlink_directory]
icon = "L"

[classes.broken_symlink]
icon = "B"

[directories.linked]
icon = "D"

[files.missing]
icon = "F"
"##,
    )
    .expect("theme should parse symlink state overrides");
    let linked = symlink_entry("linked", EntryKind::Directory, Some(EntryKind::Directory));
    let broken = symlink_entry("missing", EntryKind::File, None);

    let linked = theme.resolve_with_builtin_class(
        &linked.path,
        linked.kind,
        builtin_classify_browser_entry(&linked),
    );
    let broken = theme.resolve_with_builtin_class(
        &broken.path,
        broken.kind,
        builtin_classify_browser_entry(&broken),
    );

    assert_eq!(linked.class, FileClass::SymlinkDirectory);
    assert_eq!(linked.icon, "L");
    assert_eq!(broken.class, FileClass::BrokenSymlink);
    assert_eq!(broken.icon, "B");
}

#[test]
fn symlink_directory_class_inherits_directory_color_when_color_is_omitted() {
    let theme = Theme::from_config_str(
        r##"
[classes.directory]
color = "#778899"

[classes.symlink_directory]
icon = "L"
"##,
    )
    .expect("theme should parse symlink directory class");
    let entry = symlink_entry("linked", EntryKind::Directory, Some(EntryKind::Directory));

    let resolved = theme.resolve_with_builtin_class(
        &entry.path,
        entry.kind,
        builtin_classify_browser_entry(&entry),
    );

    assert_eq!(resolved.class, FileClass::SymlinkDirectory);
    assert_eq!(resolved.icon, "L");
    assert_eq!(resolved.color, rgb(0x77, 0x88, 0x99));
}

#[test]
fn symlink_directory_class_can_override_color_explicitly() {
    let theme = Theme::from_config_str(
        r##"
[classes.directory]
color = "#778899"

[classes.symlink_directory]
color = "#112233"
"##,
    )
    .expect("theme should parse symlink directory class");
    let entry = symlink_entry("linked", EntryKind::Directory, Some(EntryKind::Directory));

    let resolved = theme.resolve_with_builtin_class(
        &entry.path,
        entry.kind,
        builtin_classify_browser_entry(&entry),
    );

    assert_eq!(resolved.class, FileClass::SymlinkDirectory);
    assert_eq!(resolved.color, rgb(0x11, 0x22, 0x33));
}

#[test]
fn broken_symlink_class_inherits_invalid_color_when_color_is_omitted() {
    let theme = Theme::from_config_str(
        r##"
[preview.code]
invalid = "#112233"

[classes.broken_symlink]
icon = "B"
"##,
    )
    .expect("theme should parse broken symlink class");
    let entry = symlink_entry("missing", EntryKind::File, None);

    let resolved = theme.resolve_with_builtin_class(
        &entry.path,
        entry.kind,
        builtin_classify_browser_entry(&entry),
    );

    assert_eq!(resolved.class, FileClass::BrokenSymlink);
    assert_eq!(resolved.icon, "B");
    assert_eq!(resolved.color, rgb(0x11, 0x22, 0x33));
}

#[test]
fn broken_symlink_class_can_override_color_explicitly() {
    let theme = Theme::from_config_str(
        r##"
[preview.code]
invalid = "#112233"

[classes.broken_symlink]
color = "#445566"
"##,
    )
    .expect("theme should parse broken symlink class");
    let entry = symlink_entry("missing", EntryKind::File, None);

    let resolved = theme.resolve_with_builtin_class(
        &entry.path,
        entry.kind,
        builtin_classify_browser_entry(&entry),
    );

    assert_eq!(resolved.class, FileClass::BrokenSymlink);
    assert_eq!(resolved.color, rgb(0x44, 0x55, 0x66));
}

#[test]
fn resolve_browser_entry_preserves_canonical_license_appearance() {
    let (root, path) = write_temp_file(
        "browser-canonical-license",
        "LICENSE.md",
        "# SPDX-License-Identifier: Apache-2.0\n\nFixture license notes.\n",
    );

    let metadata = fs::metadata(&path).expect("metadata should exist");
    let entry = Entry {
        path: path.clone(),
        name: "LICENSE.md".to_string(),
        name_key: "license.md".to_string(),
        kind: EntryKind::File,
        symlink: None,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        readonly: false,
    };

    let browser = resolve_browser_entry(&entry);
    let preview = resolve_entry(&entry);

    assert_eq!(browser.class, FileClass::License);
    assert_eq!(preview.class, FileClass::License);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn resolve_browser_entry_preserves_non_canonical_spdx_license_appearance() {
    let (root, path) = write_temp_file(
        "browser-noncanonical-license",
        "third-party.txt",
        "SPDX-License-Identifier: Apache-2.0\n",
    );

    let metadata = fs::metadata(&path).expect("metadata should exist");
    let entry = Entry {
        path: path.clone(),
        name: "third-party.txt".to_string(),
        name_key: "third-party.txt".to_string(),
        kind: EntryKind::File,
        symlink: None,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        readonly: false,
    };

    let browser = resolve_browser_entry(&entry);
    let preview = resolve_entry(&entry);

    assert_eq!(browser.class, FileClass::License);
    assert_eq!(preview.class, FileClass::License);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn resolve_browser_entry_preserves_standalone_license_text_without_canonical_filename() {
    let (root, path) = write_temp_file(
        "browser-standalone-license",
        "third-party.txt",
        "Apache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/LICENSE-2.0\n\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n",
    );

    let metadata = fs::metadata(&path).expect("metadata should exist");
    let entry = Entry {
        path: path.clone(),
        name: "third-party.txt".to_string(),
        name_key: "third-party.txt".to_string(),
        kind: EntryKind::File,
        symlink: None,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        readonly: false,
    };

    let browser = resolve_browser_entry(&entry);
    let preview = resolve_entry(&entry);

    assert_eq!(browser.class, FileClass::License);
    assert_eq!(preview.class, FileClass::License);

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
    assert_eq!(
        specific_type_label(Path::new("JetBrainsMono.ttf"), EntryKind::File),
        Some("TrueType font")
    );
    assert_eq!(
        specific_type_label(Path::new("RedHatText.otf"), EntryKind::File),
        Some("OpenType font")
    );
    assert_eq!(
        specific_type_label(Path::new("KaTeX_Main.woff"), EntryKind::File),
        Some("WOFF font")
    );
    assert_eq!(
        specific_type_label(Path::new("FiraSans.woff2"), EntryKind::File),
        Some("WOFF2 font")
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
    assert_eq!(
        builtin_classify_path(Path::new("JetBrainsMono.ttf"), EntryKind::File),
        FileClass::Font
    );
    assert_eq!(
        builtin_classify_path(Path::new("RedHatText.otf"), EntryKind::File),
        FileClass::Font
    );
    assert_eq!(
        builtin_classify_path(Path::new("KaTeX_Main.woff"), EntryKind::File),
        FileClass::Font
    );
    assert_eq!(
        builtin_classify_path(Path::new("FiraSans.woff2"), EntryKind::File),
        FileClass::Font
    );
}
