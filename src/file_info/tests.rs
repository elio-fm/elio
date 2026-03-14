use super::*;
use crate::app::{EntryKind, FileClass};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-file-info-{label}-{unique}"))
}

#[test]
fn package_lock_uses_one_shared_definition() {
    let facts = inspect_path(Path::new("package-lock.json"), EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Data);
    assert_eq!(
        facts.preview.structured_format,
        Some(StructuredFormat::Json)
    );
    assert_eq!(
        facts.preview.highlight_language,
        Some(HighlightLanguage::Json)
    );
}

#[test]
fn lockfile_variants_get_targeted_preview_support() {
    let uv = inspect_path(Path::new("uv.lock"), EntryKind::File);
    let flake = inspect_path(Path::new("flake.lock"), EntryKind::File);
    let gem = inspect_path(Path::new("Gemfile.lock"), EntryKind::File);
    let generic = inspect_path(Path::new("deps.lock"), EntryKind::File);

    assert_eq!(uv.preview.structured_format, Some(StructuredFormat::Toml));
    assert_eq!(uv.preview.highlight_language, Some(HighlightLanguage::Toml));

    assert_eq!(
        flake.preview.structured_format,
        Some(StructuredFormat::Json)
    );
    assert_eq!(
        flake.preview.highlight_language,
        Some(HighlightLanguage::Json)
    );

    assert_eq!(gem.specific_type_label, Some("Lockfile"));
    assert_eq!(gem.preview.highlight_language, Some(HighlightLanguage::Ini));

    assert_eq!(generic.specific_type_label, Some("Lockfile"));
    assert_eq!(
        generic.preview.highlight_language,
        Some(HighlightLanguage::Ini)
    );
}

#[test]
fn dotenv_variants_are_classified_once() {
    let facts = inspect_path(Path::new(".env.local"), EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.specific_type_label, Some("Environment file"));
    assert_eq!(
        facts.preview.structured_format,
        Some(StructuredFormat::Dotenv)
    );
}

#[test]
fn json5_gets_parser_backed_preview_support() {
    let facts = inspect_path(Path::new("settings.json5"), EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(
        facts.preview.structured_format,
        Some(StructuredFormat::Json5)
    );
    assert_eq!(
        facts.preview.highlight_language,
        Some(HighlightLanguage::Jsonc)
    );
}

#[test]
fn html_and_css_files_use_code_preview_support() {
    let html = inspect_path(Path::new("index.html"), EntryKind::File);
    let css = inspect_path(Path::new("styles.css"), EntryKind::File);

    assert_eq!(html.builtin_class, FileClass::Code);
    assert_eq!(html.preview.language_hint, Some("html"));
    assert_eq!(
        html.preview.highlight_language,
        Some(HighlightLanguage::Markup)
    );

    assert_eq!(css.builtin_class, FileClass::Code);
    assert_eq!(css.preview.language_hint, Some("css"));
    assert_eq!(css.preview.highlight_language, Some(HighlightLanguage::Css));
}

#[test]
fn nix_and_cmake_files_use_code_preview_support() {
    let nix = inspect_path(Path::new("flake.nix"), EntryKind::File);
    let cmake = inspect_path(Path::new("toolchain.cmake"), EntryKind::File);
    let cmakelists = inspect_path(Path::new("CMakeLists.txt"), EntryKind::File);

    assert_eq!(nix.builtin_class, FileClass::Config);
    assert_eq!(nix.specific_type_label, Some("Nix expression"));
    assert_eq!(nix.preview.language_hint, Some("nix"));

    assert_eq!(cmake.builtin_class, FileClass::Config);
    assert_eq!(cmake.specific_type_label, Some("CMake script"));
    assert_eq!(
        cmake.preview.highlight_language,
        Some(HighlightLanguage::CMake)
    );

    assert_eq!(cmakelists.builtin_class, FileClass::Config);
    assert_eq!(cmakelists.specific_type_label, Some("CMake project"));
    assert_eq!(
        cmakelists.preview.highlight_language,
        Some(HighlightLanguage::CMake)
    );
}

#[test]
fn make_and_c_files_get_targeted_preview_support() {
    let makefile = inspect_path(Path::new("Makefile"), EntryKind::File);
    let c_source = inspect_path(Path::new("main.c"), EntryKind::File);
    let c_header = inspect_path(Path::new("app.h"), EntryKind::File);

    assert_eq!(makefile.builtin_class, FileClass::Config);
    assert_eq!(makefile.specific_type_label, Some("Makefile"));
    assert_eq!(makefile.preview.language_hint, Some("make"));
    assert_eq!(
        makefile.preview.highlight_language,
        Some(HighlightLanguage::Make)
    );

    assert_eq!(c_source.builtin_class, FileClass::Code);
    assert_eq!(c_source.specific_type_label, Some("C source file"));
    assert_eq!(c_source.preview.language_hint, Some("c"));
    assert_eq!(
        c_source.preview.highlight_language,
        Some(HighlightLanguage::CLike)
    );

    assert_eq!(c_header.builtin_class, FileClass::Code);
    assert_eq!(c_header.specific_type_label, Some("C header"));
    assert_eq!(c_header.preview.language_hint, Some("c"));
    assert_eq!(
        c_header.preview.highlight_language,
        Some(HighlightLanguage::CLike)
    );
}

#[test]
fn shell_files_and_dotfiles_get_targeted_preview_support() {
    let shell = inspect_path(Path::new("deploy.sh"), EntryKind::File);
    let bashrc = inspect_path(Path::new(".bashrc"), EntryKind::File);
    let zsh = inspect_path(Path::new("prompt.zsh"), EntryKind::File);
    let fish = inspect_path(Path::new("config.fish"), EntryKind::File);
    let zshrc = inspect_path(Path::new(".zshrc"), EntryKind::File);

    assert_eq!(shell.builtin_class, FileClass::Code);
    assert_eq!(shell.specific_type_label, Some("Shell script"));
    assert_eq!(shell.preview.language_hint, Some("sh"));
    assert_eq!(
        shell.preview.highlight_language,
        Some(HighlightLanguage::Shell)
    );

    assert_eq!(bashrc.builtin_class, FileClass::Config);
    assert_eq!(bashrc.specific_type_label, Some("Bash config"));
    assert_eq!(bashrc.preview.language_hint, Some("bash"));
    assert_eq!(
        bashrc.preview.highlight_language,
        Some(HighlightLanguage::Shell)
    );

    assert_eq!(zsh.builtin_class, FileClass::Code);
    assert_eq!(zsh.specific_type_label, Some("Zsh script"));
    assert_eq!(zsh.preview.language_hint, Some("zsh"));
    assert_eq!(
        zsh.preview.highlight_language,
        Some(HighlightLanguage::Shell)
    );

    assert_eq!(fish.builtin_class, FileClass::Code);
    assert_eq!(fish.specific_type_label, Some("Fish script"));
    assert_eq!(fish.preview.language_hint, Some("fish"));
    assert_eq!(
        fish.preview.highlight_language,
        Some(HighlightLanguage::Shell)
    );

    assert_eq!(zshrc.builtin_class, FileClass::Config);
    assert_eq!(zshrc.specific_type_label, Some("Zsh config"));
    assert_eq!(zshrc.preview.language_hint, Some("zsh"));
    assert_eq!(
        zshrc.preview.highlight_language,
        Some(HighlightLanguage::Shell)
    );
}

#[test]
fn js_like_files_use_syntax_highlighting() {
    let js = inspect_path(Path::new("main.js"), EntryKind::File);
    let tsx = inspect_path(Path::new("App.tsx"), EntryKind::File);

    assert_eq!(js.builtin_class, FileClass::Code);
    assert_eq!(
        js.preview.highlight_language,
        Some(HighlightLanguage::JsLike)
    );

    assert_eq!(tsx.builtin_class, FileClass::Code);
    assert_eq!(
        tsx.preview.highlight_language,
        Some(HighlightLanguage::JsLike)
    );
}

#[test]
fn python_family_files_use_syntax_highlighting() {
    let py = inspect_path(Path::new("main.py"), EntryKind::File);
    let pyi = inspect_path(Path::new("types.pyi"), EntryKind::File);

    assert_eq!(py.builtin_class, FileClass::Code);
    assert_eq!(py.preview.language_hint, Some("python"));
    assert_eq!(
        py.preview.highlight_language,
        Some(HighlightLanguage::Python)
    );

    assert_eq!(pyi.builtin_class, FileClass::Code);
    assert_eq!(pyi.preview.language_hint, Some("python"));
    assert_eq!(
        pyi.preview.highlight_language,
        Some(HighlightLanguage::Python)
    );
}

#[test]
fn svg_keeps_image_identity_while_using_markup_preview() {
    let facts = inspect_path(Path::new("icon.svg"), EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Image);
    assert_eq!(facts.specific_type_label, Some("SVG image"));
    assert_eq!(facts.preview.language_hint, Some("xml"));
    assert_eq!(
        facts.preview.highlight_language,
        Some(HighlightLanguage::Markup)
    );
}

#[test]
fn extensionless_png_is_detected_from_magic_bytes() {
    let root = temp_path("extensionless-png");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("background");
    fs::write(
        &path,
        [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H',
            b'D', b'R',
        ],
    )
    .expect("failed to write png signature");

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Image);
    assert_eq!(facts.specific_type_label, Some("PNG image"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_svg_is_detected_from_contents() {
    let root = temp_path("extensionless-svg");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("logo");
    fs::write(
        &path,
        r#"<?xml version="1.0"?><svg viewBox="0 0 600 300" xmlns="http://www.w3.org/2000/svg"></svg>"#,
    )
    .expect("failed to write svg contents");

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Image);
    assert_eq!(facts.specific_type_label, Some("SVG image"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn office_and_pages_documents_use_metadata_preview() {
    let doc = inspect_path(Path::new("legacy.doc"), EntryKind::File);
    let docx = inspect_path(Path::new("report.docx"), EntryKind::File);
    let docm = inspect_path(Path::new("report.docm"), EntryKind::File);
    let odt = inspect_path(Path::new("report.odt"), EntryKind::File);
    let ods = inspect_path(Path::new("budget.ods"), EntryKind::File);
    let odp = inspect_path(Path::new("deck.odp"), EntryKind::File);
    let pptx = inspect_path(Path::new("deck.pptx"), EntryKind::File);
    let xlsx = inspect_path(Path::new("budget.xlsx"), EntryKind::File);
    let pages = inspect_path(Path::new("proposal.pages"), EntryKind::File);
    let epub = inspect_path(Path::new("novel.epub"), EntryKind::File);
    let pdf = inspect_path(Path::new("manual.pdf"), EntryKind::File);

    assert_eq!(doc.builtin_class, FileClass::Document);
    assert_eq!(doc.preview.document_format, Some(DocumentFormat::Doc));
    assert_eq!(doc.specific_type_label, Some("DOC document"));

    assert_eq!(docx.builtin_class, FileClass::Document);
    assert_eq!(docx.preview.document_format, Some(DocumentFormat::Docx));
    assert_eq!(docx.specific_type_label, Some("DOCX document"));

    assert_eq!(docm.builtin_class, FileClass::Document);
    assert_eq!(docm.preview.document_format, Some(DocumentFormat::Docm));
    assert_eq!(docm.specific_type_label, Some("DOCM document"));

    assert_eq!(odt.builtin_class, FileClass::Document);
    assert_eq!(odt.preview.document_format, Some(DocumentFormat::Odt));
    assert_eq!(odt.specific_type_label, Some("ODT document"));

    assert_eq!(ods.builtin_class, FileClass::Document);
    assert_eq!(ods.preview.document_format, Some(DocumentFormat::Ods));
    assert_eq!(ods.specific_type_label, Some("ODS spreadsheet"));

    assert_eq!(odp.builtin_class, FileClass::Document);
    assert_eq!(odp.preview.document_format, Some(DocumentFormat::Odp));
    assert_eq!(odp.specific_type_label, Some("ODP presentation"));

    assert_eq!(pptx.builtin_class, FileClass::Document);
    assert_eq!(pptx.preview.document_format, Some(DocumentFormat::Pptx));
    assert_eq!(pptx.specific_type_label, Some("PPTX presentation"));

    assert_eq!(xlsx.builtin_class, FileClass::Document);
    assert_eq!(xlsx.preview.document_format, Some(DocumentFormat::Xlsx));
    assert_eq!(xlsx.specific_type_label, Some("XLSX spreadsheet"));

    assert_eq!(pages.builtin_class, FileClass::Document);
    assert_eq!(pages.preview.document_format, Some(DocumentFormat::Pages));
    assert_eq!(pages.specific_type_label, Some("Pages document"));

    assert_eq!(epub.builtin_class, FileClass::Document);
    assert_eq!(epub.preview.document_format, Some(DocumentFormat::Epub));
    assert_eq!(epub.specific_type_label, Some("EPUB ebook"));

    assert_eq!(pdf.builtin_class, FileClass::Document);
    assert_eq!(pdf.preview.document_format, Some(DocumentFormat::Pdf));
    assert_eq!(pdf.specific_type_label, Some("PDF document"));
}

#[test]
fn archive_suffixes_keep_specific_labels_for_common_multi_part_formats() {
    let tgz = inspect_path(Path::new("release.tar.gz"), EntryKind::File);
    let txz = inspect_path(Path::new("release.tar.xz"), EntryKind::File);
    let tbz2 = inspect_path(Path::new("release.tar.bz2"), EntryKind::File);
    let zip = inspect_path(Path::new("release.zip"), EntryKind::File);
    let seven_zip = inspect_path(Path::new("release.7z"), EntryKind::File);

    assert_eq!(tgz.builtin_class, FileClass::Archive);
    assert_eq!(tgz.specific_type_label, Some("TAR.GZ archive"));
    assert_eq!(txz.specific_type_label, Some("TAR.XZ archive"));
    assert_eq!(tbz2.specific_type_label, Some("TAR.BZ2 archive"));
    assert_eq!(zip.specific_type_label, Some("ZIP archive"));
    assert_eq!(seven_zip.specific_type_label, Some("7z archive"));
}

#[test]
fn compressed_disk_images_get_specific_labels() {
    let raw_xz = inspect_path(Path::new("fedora.aarch64.raw.xz"), EntryKind::File);
    let iso_zst = inspect_path(Path::new("installer.iso.zst"), EntryKind::File);
    let qcow2_gz = inspect_path(Path::new("vm.qcow2.gz"), EntryKind::File);
    let vmdk_bz2 = inspect_path(Path::new("appliance.vmdk.bz2"), EntryKind::File);

    assert_eq!(raw_xz.builtin_class, FileClass::Archive);
    assert_eq!(
        raw_xz.specific_type_label,
        Some("XZ-compressed raw disk image")
    );
    assert_eq!(
        iso_zst.specific_type_label,
        Some("Zstandard-compressed ISO disk image")
    );
    assert_eq!(
        qcow2_gz.specific_type_label,
        Some("Gzip-compressed QCOW2 disk image")
    );
    assert_eq!(
        vmdk_bz2.specific_type_label,
        Some("Bzip2-compressed VMDK disk image")
    );
}

#[test]
fn common_disk_image_extensions_keep_specific_labels_without_archive_mode() {
    let raw = inspect_path(Path::new("disk.raw"), EntryKind::File);
    let img = inspect_path(Path::new("disk.img"), EntryKind::File);
    let qcow2 = inspect_path(Path::new("vm.qcow2"), EntryKind::File);
    let vhdx = inspect_path(Path::new("backup.vhdx"), EntryKind::File);

    assert_eq!(raw.builtin_class, FileClass::File);
    assert_eq!(raw.specific_type_label, Some("Raw disk image"));
    assert_eq!(img.builtin_class, FileClass::File);
    assert_eq!(img.specific_type_label, Some("Disk image"));
    assert_eq!(qcow2.builtin_class, FileClass::File);
    assert_eq!(qcow2.specific_type_label, Some("QCOW2 disk image"));
    assert_eq!(vhdx.builtin_class, FileClass::File);
    assert_eq!(vhdx.specific_type_label, Some("VHDX disk image"));
}

#[test]
fn executable_and_library_extensions_keep_specific_labels() {
    let dll = inspect_path(Path::new("plugin.dll"), EntryKind::File);
    let sys = inspect_path(Path::new("driver.sys"), EntryKind::File);
    let so = inspect_path(Path::new("libelio.so"), EntryKind::File);
    let dylib = inspect_path(Path::new("libelio.dylib"), EntryKind::File);
    let object = inspect_path(Path::new("main.o"), EntryKind::File);

    assert_eq!(dll.specific_type_label, Some("Windows DLL"));
    assert_eq!(sys.specific_type_label, Some("Windows system driver"));
    assert_eq!(so.specific_type_label, Some("Shared library"));
    assert_eq!(dylib.specific_type_label, Some("Dynamic library"));
    assert_eq!(object.specific_type_label, Some("Object file"));
}
