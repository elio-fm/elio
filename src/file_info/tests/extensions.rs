use super::*;

#[test]
fn json5_gets_parser_backed_preview_support() {
    let facts = inspect_path(Path::new("settings.json5"), EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(
        facts.preview.structured_format,
        Some(StructuredFormat::Json5)
    );
    assert_code_spec(
        facts.preview,
        Some("json5"),
        CodeBackend::Custom(CustomCodeKind::Jsonc),
    );
}

#[test]
fn supported_video_extensions_keep_specific_video_labels() {
    let cases = [
        ("clip.mp4", "MP4 video"),
        ("clip.mkv", "Matroska video"),
        ("clip.mov", "QuickTime video"),
        ("clip.webm", "WebM video"),
        ("clip.avi", "AVI video"),
    ];

    for (path, label) in cases {
        let facts = inspect_path(Path::new(path), EntryKind::File);
        assert_eq!(facts.builtin_class, FileClass::Video);
        assert_eq!(facts.specific_type_label, Some(label));
    }
}

#[test]
fn font_files_keep_specific_labels() {
    let cases = [
        ("JetBrainsMono.ttf", "TrueType font"),
        ("RedHatText.otf", "OpenType font"),
        ("KaTeX_Main.woff", "WOFF font"),
        ("FiraSans.woff2", "WOFF2 font"),
    ];

    for (path, label) in cases {
        let facts = inspect_path(Path::new(path), EntryKind::File);
        assert_eq!(facts.builtin_class, FileClass::Font);
        assert_eq!(facts.specific_type_label, Some(label));
    }
}
#[test]
fn html_and_css_files_use_code_preview_support() {
    let html = inspect_path(Path::new("index.html"), EntryKind::File);
    let css = inspect_path(Path::new("styles.css"), EntryKind::File);
    let scss = inspect_path(Path::new("styles.scss"), EntryKind::File);
    let sass = inspect_path(Path::new("styles.sass"), EntryKind::File);
    let less = inspect_path(Path::new("styles.less"), EntryKind::File);

    assert_eq!(html.builtin_class, FileClass::Code);
    assert_eq!(html.preview.language_hint, Some("html"));
    assert_code_spec(html.preview, Some("html"), CodeBackend::Syntect);

    assert_eq!(css.builtin_class, FileClass::Code);
    assert_eq!(css.preview.language_hint, Some("css"));
    assert_code_spec(css.preview, Some("css"), CodeBackend::Syntect);

    assert_eq!(scss.builtin_class, FileClass::Code);
    assert_eq!(scss.preview.language_hint, Some("scss"));
    assert_code_spec(scss.preview, Some("scss"), CodeBackend::Syntect);

    assert_eq!(sass.builtin_class, FileClass::Code);
    assert_eq!(sass.preview.language_hint, Some("sass"));
    assert_code_spec(sass.preview, Some("sass"), CodeBackend::Syntect);

    assert_eq!(less.builtin_class, FileClass::Code);
    assert_eq!(less.preview.language_hint, Some("less"));
    assert_code_spec(less.preview, Some("less"), CodeBackend::Syntect);
}

#[test]
fn qml_files_use_code_preview_support() {
    let qml = inspect_path(Path::new("Main.qml"), EntryKind::File);

    assert_eq!(qml.builtin_class, FileClass::Code);
    assert_eq!(qml.specific_type_label, Some("QML source file"));
    assert_eq!(qml.preview.language_hint, Some("qml"));
    assert_code_spec(qml.preview, Some("qml"), CodeBackend::Syntect);
}

#[test]
fn nix_and_cmake_files_use_code_preview_support() {
    let nix = inspect_path(Path::new("flake.nix"), EntryKind::File);
    let cmake = inspect_path(Path::new("toolchain.cmake"), EntryKind::File);
    let cmakelists = inspect_path(Path::new("CMakeLists.txt"), EntryKind::File);
    let hcl = inspect_path(Path::new("terraform.hcl"), EntryKind::File);
    let terraform = inspect_path(Path::new("main.tf"), EntryKind::File);
    let terraform_vars = inspect_path(Path::new("prod.tfvars"), EntryKind::File);
    let terraform_lock = inspect_path(Path::new(".terraform.lock.hcl"), EntryKind::File);

    assert_eq!(nix.builtin_class, FileClass::Config);
    assert_eq!(nix.specific_type_label, Some("Nix expression"));
    assert_eq!(nix.preview.language_hint, Some("nix"));
    assert_code_spec(nix.preview, Some("nix"), CodeBackend::Syntect);

    assert_eq!(cmake.builtin_class, FileClass::Config);
    assert_eq!(cmake.specific_type_label, Some("CMake script"));
    assert_code_spec(cmake.preview, Some("cmake"), CodeBackend::Syntect);

    assert_eq!(cmakelists.builtin_class, FileClass::Config);
    assert_eq!(cmakelists.specific_type_label, Some("CMake project"));
    assert_code_spec(cmakelists.preview, Some("cmake"), CodeBackend::Syntect);

    assert_eq!(hcl.builtin_class, FileClass::Config);
    assert_eq!(hcl.specific_type_label, Some("HCL config"));
    assert_code_spec(hcl.preview, Some("hcl"), CodeBackend::Syntect);

    assert_eq!(terraform.builtin_class, FileClass::Config);
    assert_eq!(terraform.specific_type_label, Some("Terraform module"));
    assert_code_spec(terraform.preview, Some("terraform"), CodeBackend::Syntect);

    assert_eq!(terraform_vars.builtin_class, FileClass::Config);
    assert_eq!(
        terraform_vars.specific_type_label,
        Some("Terraform variables")
    );
    assert_code_spec(
        terraform_vars.preview,
        Some("terraform"),
        CodeBackend::Syntect,
    );

    assert_eq!(terraform_lock.builtin_class, FileClass::Data);
    assert_eq!(
        terraform_lock.specific_type_label,
        Some("Terraform lockfile")
    );
    assert_code_spec(terraform_lock.preview, Some("hcl"), CodeBackend::Syntect);
}

#[test]
fn make_and_c_files_get_targeted_preview_support() {
    let makefile = inspect_path(Path::new("Makefile"), EntryKind::File);
    let c_source = inspect_path(Path::new("main.c"), EntryKind::File);
    let c_header = inspect_path(Path::new("app.h"), EntryKind::File);

    assert_eq!(makefile.builtin_class, FileClass::Config);
    assert_eq!(makefile.specific_type_label, Some("Makefile"));
    assert_eq!(makefile.preview.language_hint, Some("make"));
    assert_code_spec(makefile.preview, Some("make"), CodeBackend::Syntect);

    assert_eq!(c_source.builtin_class, FileClass::Code);
    assert_eq!(c_source.specific_type_label, Some("C source file"));
    assert_eq!(c_source.preview.language_hint, Some("c"));
    assert_code_spec(c_source.preview, Some("c"), CodeBackend::Syntect);

    assert_eq!(c_header.builtin_class, FileClass::Code);
    assert_eq!(c_header.specific_type_label, Some("C header"));
    assert_eq!(c_header.preview.language_hint, Some("c"));
    assert_code_spec(c_header.preview, Some("c"), CodeBackend::Syntect);
}

#[test]
fn js_like_files_use_syntax_highlighting() {
    let js = inspect_path(Path::new("main.js"), EntryKind::File);
    let ts = inspect_path(Path::new("main.ts"), EntryKind::File);
    let tsx = inspect_path(Path::new("App.tsx"), EntryKind::File);

    assert_eq!(js.builtin_class, FileClass::Code);
    assert_code_spec(js.preview, Some("javascript"), CodeBackend::Syntect);

    assert_eq!(ts.builtin_class, FileClass::Code);
    assert_code_spec(ts.preview, Some("typescript"), CodeBackend::Syntect);

    assert_eq!(tsx.builtin_class, FileClass::Code);
    assert_code_spec(tsx.preview, Some("tsx"), CodeBackend::Syntect);
}

#[test]
fn curated_generic_languages_use_syntect_preview_support() {
    let sql = inspect_path(Path::new("schema.sql"), EntryKind::File);
    let diff = inspect_path(Path::new("changes.diff"), EntryKind::File);
    let dockerfile = inspect_path(Path::new("Dockerfile"), EntryKind::File);
    let groovy = inspect_path(Path::new("build.gradle"), EntryKind::File);
    let scala = inspect_path(Path::new("build.sbt"), EntryKind::File);
    let perl = inspect_path(Path::new("script.pl"), EntryKind::File);
    let haskell = inspect_path(Path::new("Main.hs"), EntryKind::File);
    let julia = inspect_path(Path::new("main.jl"), EntryKind::File);
    let r = inspect_path(Path::new("analysis.r"), EntryKind::File);
    let just = inspect_path(Path::new("Justfile"), EntryKind::File);
    let cs = inspect_path(Path::new("Program.cs"), EntryKind::File);
    let csx = inspect_path(Path::new("Program.csx"), EntryKind::File);
    let dart = inspect_path(Path::new("main.dart"), EntryKind::File);
    let fortran = inspect_path(Path::new("solver.f90"), EntryKind::File);
    let fortran_pp = inspect_path(Path::new("solver.fpp"), EntryKind::File);
    let cobol = inspect_path(Path::new("ledger.cbl"), EntryKind::File);
    let cobol_copybook = inspect_path(Path::new("customer.cpy"), EntryKind::File);
    let zig = inspect_path(Path::new("main.zig"), EntryKind::File);
    let swift = inspect_path(Path::new("main.swift"), EntryKind::File);
    let kotlin = inspect_path(Path::new("main.kts"), EntryKind::File);
    let elixir = inspect_path(Path::new("main.ex"), EntryKind::File);
    let elixir_script = inspect_path(Path::new("mix.exs"), EntryKind::File);
    let clojure = inspect_path(Path::new("core.clj"), EntryKind::File);
    let clojurescript = inspect_path(Path::new("app.cljs"), EntryKind::File);
    let clojure_shared = inspect_path(Path::new("shared.cljc"), EntryKind::File);
    let edn = inspect_path(Path::new("config.edn"), EntryKind::File);
    let powershell = inspect_path(Path::new("build.ps1"), EntryKind::File);
    let powershell_module = inspect_path(Path::new("ElioTools.psm1"), EntryKind::File);
    let powershell_data = inspect_path(Path::new("ElioTools.psd1"), EntryKind::File);

    assert_eq!(sql.builtin_class, FileClass::Code);
    assert_eq!(sql.specific_type_label, Some("SQL script"));
    assert_code_spec(sql.preview, Some("sql"), CodeBackend::Syntect);

    assert_eq!(diff.builtin_class, FileClass::Code);
    assert_eq!(diff.specific_type_label, Some("Diff file"));
    assert_code_spec(diff.preview, Some("diff"), CodeBackend::Syntect);

    assert_eq!(dockerfile.builtin_class, FileClass::Config);
    assert_eq!(dockerfile.specific_type_label, Some("Docker build file"));
    assert_code_spec(dockerfile.preview, Some("dockerfile"), CodeBackend::Syntect);

    assert_eq!(groovy.builtin_class, FileClass::Config);
    assert_eq!(groovy.specific_type_label, Some("Gradle build script"));
    assert_code_spec(groovy.preview, Some("groovy"), CodeBackend::Syntect);

    assert_eq!(scala.builtin_class, FileClass::Config);
    assert_eq!(scala.specific_type_label, Some("sbt build definition"));
    assert_code_spec(scala.preview, Some("scala"), CodeBackend::Syntect);

    assert_eq!(perl.builtin_class, FileClass::Code);
    assert_eq!(perl.specific_type_label, Some("Perl script"));
    assert_code_spec(perl.preview, Some("perl"), CodeBackend::Syntect);

    assert_eq!(haskell.builtin_class, FileClass::Code);
    assert_eq!(haskell.specific_type_label, Some("Haskell source file"));
    assert_code_spec(haskell.preview, Some("haskell"), CodeBackend::Syntect);

    assert_eq!(julia.builtin_class, FileClass::Code);
    assert_eq!(julia.specific_type_label, Some("Julia source file"));
    assert_code_spec(julia.preview, Some("julia"), CodeBackend::Syntect);

    assert_eq!(r.builtin_class, FileClass::Code);
    assert_eq!(r.specific_type_label, Some("R script"));
    assert_code_spec(r.preview, Some("r"), CodeBackend::Syntect);

    assert_eq!(just.builtin_class, FileClass::Config);
    assert_eq!(just.specific_type_label, Some("Justfile"));
    assert_code_spec(just.preview, Some("just"), CodeBackend::Syntect);

    assert_eq!(cs.builtin_class, FileClass::Code);
    assert_eq!(cs.specific_type_label, Some("C# source file"));
    assert_code_spec(cs.preview, Some("cs"), CodeBackend::Syntect);

    assert_eq!(csx.builtin_class, FileClass::Code);
    assert_eq!(csx.specific_type_label, Some("C# script"));
    assert_code_spec(csx.preview, Some("cs"), CodeBackend::Syntect);

    assert_eq!(dart.builtin_class, FileClass::Code);
    assert_eq!(dart.specific_type_label, Some("Dart source file"));
    assert_code_spec(dart.preview, Some("dart"), CodeBackend::Syntect);

    assert_eq!(fortran.builtin_class, FileClass::Code);
    assert_eq!(fortran.specific_type_label, Some("Fortran source file"));
    assert_code_spec(fortran.preview, Some("fortran"), CodeBackend::Syntect);

    assert_eq!(fortran_pp.builtin_class, FileClass::Code);
    assert_eq!(
        fortran_pp.specific_type_label,
        Some("Fortran preprocessor source file")
    );
    assert_code_spec(fortran_pp.preview, Some("fortran"), CodeBackend::Syntect);

    assert_eq!(cobol.builtin_class, FileClass::Code);
    assert_eq!(cobol.specific_type_label, Some("COBOL source file"));
    assert_code_spec(cobol.preview, Some("cobol"), CodeBackend::Syntect);

    assert_eq!(cobol_copybook.builtin_class, FileClass::Code);
    assert_eq!(cobol_copybook.specific_type_label, Some("COBOL copybook"));
    assert_code_spec(cobol_copybook.preview, Some("cobol"), CodeBackend::Syntect);

    assert_eq!(zig.builtin_class, FileClass::Code);
    assert_eq!(zig.specific_type_label, Some("Zig source file"));
    assert_code_spec(zig.preview, Some("zig"), CodeBackend::Syntect);

    assert_eq!(swift.builtin_class, FileClass::Code);
    assert_eq!(swift.specific_type_label, Some("Swift source file"));
    assert_code_spec(swift.preview, Some("swift"), CodeBackend::Syntect);

    assert_eq!(kotlin.builtin_class, FileClass::Code);
    assert_eq!(kotlin.specific_type_label, Some("Kotlin script"));
    assert_code_spec(kotlin.preview, Some("kotlin"), CodeBackend::Syntect);

    assert_eq!(elixir.builtin_class, FileClass::Code);
    assert_eq!(elixir.specific_type_label, Some("Elixir source file"));
    assert_code_spec(elixir.preview, Some("elixir"), CodeBackend::Syntect);

    assert_eq!(elixir_script.builtin_class, FileClass::Code);
    assert_eq!(elixir_script.specific_type_label, Some("Elixir script"));
    assert_code_spec(elixir_script.preview, Some("elixir"), CodeBackend::Syntect);

    assert_eq!(clojure.builtin_class, FileClass::Code);
    assert_eq!(clojure.specific_type_label, Some("Clojure source file"));
    assert_code_spec(clojure.preview, Some("clojure"), CodeBackend::Syntect);

    assert_eq!(clojurescript.builtin_class, FileClass::Code);
    assert_eq!(
        clojurescript.specific_type_label,
        Some("ClojureScript source file")
    );
    assert_code_spec(clojurescript.preview, Some("clojure"), CodeBackend::Syntect);

    assert_eq!(clojure_shared.builtin_class, FileClass::Code);
    assert_eq!(
        clojure_shared.specific_type_label,
        Some("Portable Clojure source file")
    );
    assert_code_spec(
        clojure_shared.preview,
        Some("clojure"),
        CodeBackend::Syntect,
    );

    assert_eq!(edn.builtin_class, FileClass::Config);
    assert_eq!(edn.specific_type_label, Some("EDN file"));
    assert_code_spec(edn.preview, Some("clojure"), CodeBackend::Syntect);

    assert_eq!(powershell.builtin_class, FileClass::Code);
    assert_eq!(powershell.specific_type_label, Some("PowerShell script"));
    assert_code_spec(powershell.preview, Some("powershell"), CodeBackend::Syntect);

    assert_eq!(powershell_module.builtin_class, FileClass::Code);
    assert_eq!(
        powershell_module.specific_type_label,
        Some("PowerShell module")
    );
    assert_code_spec(
        powershell_module.preview,
        Some("powershell"),
        CodeBackend::Syntect,
    );

    assert_eq!(powershell_data.builtin_class, FileClass::Config);
    assert_eq!(
        powershell_data.specific_type_label,
        Some("PowerShell data file")
    );
    assert_code_spec(
        powershell_data.preview,
        Some("powershell"),
        CodeBackend::Syntect,
    );
}

#[test]
fn python_family_files_use_syntax_highlighting() {
    let py = inspect_path(Path::new("main.py"), EntryKind::File);
    let pyi = inspect_path(Path::new("types.pyi"), EntryKind::File);

    assert_eq!(py.builtin_class, FileClass::Code);
    assert_eq!(py.preview.language_hint, Some("python"));
    assert_code_spec(py.preview, Some("python"), CodeBackend::Syntect);

    assert_eq!(pyi.builtin_class, FileClass::Code);
    assert_eq!(pyi.preview.language_hint, Some("python"));
    assert_code_spec(pyi.preview, Some("python"), CodeBackend::Syntect);
}

#[test]
fn lua_files_use_syntax_highlighting() {
    let lua = inspect_path(Path::new("init.lua"), EntryKind::File);

    assert_eq!(lua.builtin_class, FileClass::Code);
    assert_eq!(lua.specific_type_label, Some("Lua script"));
    assert_eq!(lua.preview.language_hint, Some("lua"));
    assert_code_spec(lua.preview, Some("lua"), CodeBackend::Syntect);
}

#[test]
fn svg_keeps_image_identity_while_using_markup_preview() {
    let facts = inspect_path(Path::new("icon.svg"), EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Image);
    assert_eq!(facts.specific_type_label, Some("SVG image"));
    assert_eq!(facts.preview.language_hint, Some("xml"));
    assert_code_spec(facts.preview, Some("xml"), CodeBackend::Syntect);
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
    let mobi = inspect_path(Path::new("novel.mobi"), EntryKind::File);
    let azw3 = inspect_path(Path::new("novel.azw3"), EntryKind::File);
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

    assert_eq!(mobi.builtin_class, FileClass::Document);
    assert_eq!(mobi.preview.document_format, Some(DocumentFormat::Mobi));
    assert_eq!(mobi.specific_type_label, Some("MOBI ebook"));

    assert_eq!(azw3.builtin_class, FileClass::Document);
    assert_eq!(azw3.preview.document_format, Some(DocumentFormat::Azw3));
    assert_eq!(azw3.specific_type_label, Some("AZW3 ebook"));

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
    let cbz = inspect_path(Path::new("issue.cbz"), EntryKind::File);
    let cbr = inspect_path(Path::new("issue.cbr"), EntryKind::File);
    let rar = inspect_path(Path::new("release.rar"), EntryKind::File);
    let seven_zip = inspect_path(Path::new("release.7z"), EntryKind::File);

    assert_eq!(tgz.builtin_class, FileClass::Archive);
    assert_eq!(tgz.specific_type_label, Some("TAR.GZ archive"));
    assert_eq!(txz.specific_type_label, Some("TAR.XZ archive"));
    assert_eq!(tbz2.specific_type_label, Some("TAR.BZ2 archive"));
    assert_eq!(zip.specific_type_label, Some("ZIP archive"));
    assert_eq!(cbz.specific_type_label, Some("Comic ZIP archive"));
    assert_eq!(cbr.specific_type_label, Some("Comic RAR archive"));
    assert_eq!(rar.specific_type_label, Some("RAR archive"));
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

#[test]
fn sqlite_extensions_use_sqlite_preview_kind() {
    for filename in &["app.sqlite", "app.sqlite3", "app.db3"] {
        let facts = inspect_path(Path::new(filename), EntryKind::File);
        assert_eq!(facts.builtin_class, FileClass::Data, "{filename}");
        assert_eq!(facts.preview.kind, PreviewKind::Sqlite, "{filename}");
        assert_eq!(
            facts.specific_type_label,
            Some("SQLite database"),
            "{filename}"
        );
    }
}

#[test]
fn db_extension_uses_sqlite_candidate_kind_so_it_stays_light_before_sniff() {
    let facts = inspect_path(Path::new("data.db"), EntryKind::File);
    assert_eq!(facts.builtin_class, FileClass::Data);
    // SqliteCandidate — not Heavy until header sniffing confirms SQLite magic.
    assert_eq!(facts.preview.kind, PreviewKind::SqliteCandidate);
    assert_eq!(facts.specific_type_label, Some("Database file"));
}

#[test]
fn csv_and_tsv_use_csv_preview_kind() {
    let csv = inspect_path(Path::new("data.csv"), EntryKind::File);
    assert_eq!(csv.builtin_class, FileClass::Data);
    assert_eq!(csv.preview.kind, PreviewKind::Csv);
    assert_eq!(csv.specific_type_label, Some("CSV file"));

    let tsv = inspect_path(Path::new("data.tsv"), EntryKind::File);
    assert_eq!(tsv.builtin_class, FileClass::Data);
    assert_eq!(tsv.preview.kind, PreviewKind::Csv);
    assert_eq!(tsv.specific_type_label, Some("TSV file"));
}

#[test]
fn sqlite_sidecar_extensions_get_descriptive_labels() {
    let cases = [
        ("app.sqlite-wal", "SQLite WAL"),
        ("app.sqlite-shm", "SQLite shared memory"),
        ("app.sqlite-journal", "SQLite rollback journal"),
        ("app.db-wal", "SQLite WAL"),
        ("app.db-shm", "SQLite shared memory"),
        ("app.db-journal", "SQLite rollback journal"),
    ];
    for (filename, label) in cases {
        let facts = inspect_path(Path::new(filename), EntryKind::File);
        assert_eq!(facts.builtin_class, FileClass::Data, "{filename}");
        assert_eq!(facts.specific_type_label, Some(label), "{filename}");
    }
}
