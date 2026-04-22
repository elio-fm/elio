use super::super::rules::rgb;
use super::*;

const GENERIC_DEV_DIRECTORIES: &[&str] = &[
    "node_modules",
    "tests",
    "test",
    "__tests__",
    "scripts",
    "build",
    "dist",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".astro",
    "assets",
    "coverage",
    "tmp",
    "temp",
    "out",
    "target",
    "bin",
    "lib",
    "vendor",
    "src",
    "config",
    "docs",
];

fn load_built_in_default_theme_asset() -> Theme {
    Theme::apply_config_on(Theme::base_theme(), DEFAULT_THEME_TOML)
        .expect("built-in default theme asset should parse")
}

fn assert_uses_normal_folder_color_for_generic_dev_directories(theme: &Theme, label: &str) {
    let normal_folder_color = theme
        .resolve(Path::new("projects"), EntryKind::Directory)
        .color;

    for directory in GENERIC_DEV_DIRECTORIES {
        let resolved = theme.resolve(Path::new(directory), EntryKind::Directory);
        assert_eq!(
            resolved.class,
            FileClass::Directory,
            "{label}: {directory} should resolve as a directory",
        );
        assert_eq!(
            resolved.color, normal_folder_color,
            "{label}: {directory} should use the normal folder color",
        );
    }
}

#[test]
fn built_in_default_theme_asset_matches_runtime_default_theme() {
    let built_in_asset = load_built_in_default_theme_asset();
    let runtime_default = Theme::default_theme();

    assert_eq!(built_in_asset.palette.bg, runtime_default.palette.bg);
    assert_eq!(
        built_in_asset.palette.selected_bg,
        runtime_default.palette.selected_bg
    );
    assert_eq!(
        built_in_asset.preview.code.keyword,
        runtime_default.preview.code.keyword,
    );
    assert_eq!(
        built_in_asset.preview.code.function,
        runtime_default.preview.code.function,
    );

    for (path, kind) in [
        ("projects", EntryKind::Directory),
        ("Downloads", EntryKind::Directory),
        ("Cargo.toml", EntryKind::File),
        ("Cargo.lock", EntryKind::File),
        ("README.md", EntryKind::File),
        ("main.rs", EntryKind::File),
    ] {
        let built_in = built_in_asset.resolve(Path::new(path), kind);
        let runtime = runtime_default.resolve(Path::new(path), kind);
        assert_eq!(
            built_in.class, runtime.class,
            "{path} should keep its class"
        );
        assert_eq!(built_in.icon, runtime.icon, "{path} should keep its icon");
        assert_eq!(
            built_in.color, runtime.color,
            "{path} should keep its color"
        );
    }
}

#[test]
fn built_in_default_theme_uses_normal_folder_color_for_generic_dev_directories() {
    let theme = load_built_in_default_theme_asset();
    assert_uses_normal_folder_color_for_generic_dev_directories(&theme, "built-in default");
}

#[test]
fn default_theme_assigns_specific_icons_for_common_dev_paths() {
    let theme = Theme::default_theme();

    let ts = theme.resolve(Path::new("main.ts"), EntryKind::File);
    assert_eq!(ts.icon, "");

    let json = theme.resolve(Path::new("data.json"), EntryKind::File);
    assert_eq!(json.class, FileClass::Config);
    assert_eq!(json.icon, "");
    assert_eq!(json.color, rgb(125, 176, 255));

    let package = theme.resolve(Path::new("package.json"), EntryKind::File);
    assert_eq!(package.icon, "󰏗");

    let modules = theme.resolve(Path::new("node_modules"), EntryKind::Directory);
    assert_eq!(modules.icon, "󰏗");

    let docs = theme.resolve(Path::new("docs"), EntryKind::Directory);
    assert_eq!(docs.class, FileClass::Directory);
    assert_eq!(docs.icon, "󱧷");
    assert_eq!(docs.color, rgb(91, 168, 255));

    let bin = theme.resolve(Path::new("bin"), EntryKind::Directory);
    assert_eq!(bin.class, FileClass::Directory);
    assert_eq!(bin.icon, "󱁿");
    assert_eq!(bin.color, rgb(91, 168, 255));

    let lib = theme.resolve(Path::new("lib"), EntryKind::Directory);
    assert_eq!(lib.class, FileClass::Directory);
    assert_eq!(lib.icon, "󰉋");
    assert_eq!(lib.color, rgb(91, 168, 255));

    let target = theme.resolve(Path::new("target"), EntryKind::Directory);
    assert_eq!(target.class, FileClass::Directory);
    assert_eq!(target.icon, "󱧽");
    assert_eq!(target.color, rgb(91, 168, 255));

    let dist = theme.resolve(Path::new("dist"), EntryKind::Directory);
    assert_eq!(dist.class, FileClass::Directory);
    assert_eq!(dist.icon, "󰉋");
    assert_eq!(dist.color, rgb(91, 168, 255));

    let out = theme.resolve(Path::new("out"), EntryKind::Directory);
    assert_eq!(out.class, FileClass::Directory);
    assert_eq!(out.icon, "󰉋");
    assert_eq!(out.color, rgb(91, 168, 255));

    let xml = theme.resolve(Path::new("config.xml"), EntryKind::File);
    assert_eq!(xml.class, FileClass::Code);
    assert_eq!(xml.icon, "󰗀");
    assert_eq!(xml.color, rgb(179, 140, 255));

    let csharp = theme.resolve(Path::new("Program.cs"), EntryKind::File);
    assert_eq!(csharp.class, FileClass::Code);
    assert_eq!(csharp.icon, "󰌛");
    assert_eq!(csharp.color, rgb(104, 179, 120));

    let csharp_script = theme.resolve(Path::new("Program.csx"), EntryKind::File);
    assert_eq!(csharp_script.class, FileClass::Code);
    assert_eq!(csharp_script.icon, "󰌛");
    assert_eq!(csharp_script.color, rgb(104, 179, 120));

    let dart = theme.resolve(Path::new("main.dart"), EntryKind::File);
    assert_eq!(dart.class, FileClass::Code);
    assert_eq!(dart.icon, "");
    assert_eq!(dart.color, rgb(56, 213, 255));

    let fortran = theme.resolve(Path::new("solver.f90"), EntryKind::File);
    assert_eq!(fortran.class, FileClass::Code);
    assert_eq!(fortran.icon, "󱈚");
    assert_eq!(fortran.color, rgb(115, 79, 150));

    let fortran_pp = theme.resolve(Path::new("solver.fpp"), EntryKind::File);
    assert_eq!(fortran_pp.class, FileClass::Code);
    assert_eq!(fortran_pp.icon, "󱈚");
    assert_eq!(fortran_pp.color, rgb(115, 79, 150));

    let cobol = theme.resolve(Path::new("ledger.cbl"), EntryKind::File);
    assert_eq!(cobol.class, FileClass::Code);
    assert_eq!(cobol.icon, "");
    assert_eq!(cobol.color, rgb(0, 92, 165));

    let cobol_copybook = theme.resolve(Path::new("customer.cpy"), EntryKind::File);
    assert_eq!(cobol_copybook.class, FileClass::Code);
    assert_eq!(cobol_copybook.icon, "");
    assert_eq!(cobol_copybook.color, rgb(0, 92, 165));

    let elixir = theme.resolve(Path::new("main.ex"), EntryKind::File);
    assert_eq!(elixir.class, FileClass::Code);
    assert_eq!(elixir.icon, "");
    assert_eq!(elixir.color, rgb(155, 143, 199));

    let elixir_script = theme.resolve(Path::new("mix.exs"), EntryKind::File);
    assert_eq!(elixir_script.class, FileClass::Code);
    assert_eq!(elixir_script.icon, "");
    assert_eq!(elixir_script.color, rgb(155, 143, 199));

    let clojure = theme.resolve(Path::new("core.clj"), EntryKind::File);
    assert_eq!(clojure.class, FileClass::Code);
    assert_eq!(clojure.icon, "");
    assert_eq!(clojure.color, rgb(128, 176, 92));

    let clojurescript = theme.resolve(Path::new("app.cljs"), EntryKind::File);
    assert_eq!(clojurescript.class, FileClass::Code);
    assert_eq!(clojurescript.icon, "");
    assert_eq!(clojurescript.color, rgb(128, 176, 92));

    let clojure_data = theme.resolve(Path::new("deps.edn"), EntryKind::File);
    assert_eq!(clojure_data.class, FileClass::Config);
    assert_eq!(clojure_data.icon, "");
    assert_eq!(clojure_data.color, rgb(128, 176, 92));

    let leiningen = theme.resolve(Path::new("project.clj"), EntryKind::File);
    assert_eq!(leiningen.class, FileClass::Config);
    assert_eq!(leiningen.icon, "");
    assert_eq!(leiningen.color, rgb(128, 176, 92));

    let powershell = theme.resolve(Path::new("build.ps1"), EntryKind::File);
    assert_eq!(powershell.class, FileClass::Code);
    assert_eq!(powershell.icon, "󰨊");
    assert_eq!(powershell.color, rgb(95, 153, 219));

    let powershell_module = theme.resolve(Path::new("ElioTools.psm1"), EntryKind::File);
    assert_eq!(powershell_module.class, FileClass::Code);
    assert_eq!(powershell_module.icon, "󰨊");
    assert_eq!(powershell_module.color, rgb(95, 153, 219));

    let powershell_data = theme.resolve(Path::new("ElioTools.psd1"), EntryKind::File);
    assert_eq!(powershell_data.class, FileClass::Config);
    assert_eq!(powershell_data.icon, "󰨊");
    assert_eq!(powershell_data.color, rgb(95, 153, 219));

    let shell = theme.resolve(Path::new("deploy.sh"), EntryKind::File);
    assert_eq!(shell.class, FileClass::Code);
    assert_eq!(shell.icon, "");
    assert_eq!(shell.color, rgb(214, 222, 240));

    let bash = theme.resolve(Path::new("profile.bash"), EntryKind::File);
    assert_eq!(bash.class, FileClass::Code);
    assert_eq!(bash.icon, "");
    assert_eq!(bash.color, rgb(214, 222, 240));

    let zsh = theme.resolve(Path::new("prompt.zsh"), EntryKind::File);
    assert_eq!(zsh.class, FileClass::Code);
    assert_eq!(zsh.icon, "");
    assert_eq!(zsh.color, rgb(214, 222, 240));

    let fish = theme.resolve(Path::new("config.fish"), EntryKind::File);
    assert_eq!(fish.class, FileClass::Code);
    assert_eq!(fish.icon, "");
    assert_eq!(fish.color, rgb(214, 222, 240));
}

#[test]
fn default_theme_assigns_icons_for_new_language_support() {
    let theme = Theme::default_theme();

    let dockerfile = theme.resolve(Path::new("Dockerfile"), EntryKind::File);
    assert_eq!(dockerfile.class, FileClass::Config);
    assert_eq!(dockerfile.icon, "󰡨");

    let sql = theme.resolve(Path::new("schema.sql"), EntryKind::File);
    assert_eq!(sql.icon, "");

    let diff = theme.resolve(Path::new("changes.diff"), EntryKind::File);
    assert_eq!(diff.class, FileClass::Code);
    assert_eq!(diff.icon, "");

    let terraform = theme.resolve(Path::new("main.tf"), EntryKind::File);
    assert_eq!(terraform.class, FileClass::Config);
    assert_eq!(terraform.icon, "");

    let hcl = theme.resolve(Path::new("terraform.lock.hcl"), EntryKind::File);
    assert_eq!(hcl.class, FileClass::Config);
    assert_eq!(hcl.icon, "");

    let groovy = theme.resolve(Path::new("build.gradle"), EntryKind::File);
    assert_eq!(groovy.class, FileClass::Config);
    assert_eq!(groovy.icon, "");

    let scala = theme.resolve(Path::new("build.sbt"), EntryKind::File);
    assert_eq!(scala.class, FileClass::Config);
    assert_eq!(scala.icon, "");

    let perl = theme.resolve(Path::new("script.pl"), EntryKind::File);
    assert_eq!(perl.class, FileClass::Code);
    assert_eq!(perl.icon, "");

    let haskell = theme.resolve(Path::new("Main.hs"), EntryKind::File);
    assert_eq!(haskell.class, FileClass::Code);
    assert_eq!(haskell.icon, "");

    let julia = theme.resolve(Path::new("main.jl"), EntryKind::File);
    assert_eq!(julia.class, FileClass::Code);
    assert_eq!(julia.icon, "");

    let r = theme.resolve(Path::new("analysis.r"), EntryKind::File);
    assert_eq!(r.class, FileClass::Code);
    assert_eq!(r.icon, "󰟔");

    let just = theme.resolve(Path::new("Justfile"), EntryKind::File);
    assert_eq!(just.class, FileClass::Config);
    assert_eq!(just.icon, "");

    let ziggy = theme.resolve(Path::new("config.ziggy"), EntryKind::File);
    assert_eq!(ziggy.class, FileClass::Config);
    assert_eq!(ziggy.icon, "");

    let fortran = theme.resolve(Path::new("solver.f90"), EntryKind::File);
    assert_eq!(fortran.class, FileClass::Code);
    assert_eq!(fortran.icon, "󱈚");

    let cobol = theme.resolve(Path::new("ledger.cbl"), EntryKind::File);
    assert_eq!(cobol.class, FileClass::Code);
    assert_eq!(cobol.icon, "");

    let qml = theme.resolve(Path::new("Main.qml"), EntryKind::File);
    assert_eq!(qml.class, FileClass::Code);
    assert_eq!(qml.icon, "");
}

#[test]
fn word_processing_documents_get_blue_document_icons() {
    let theme = Theme::default_theme();

    let docx = theme.resolve(Path::new("report.docx"), EntryKind::File);
    assert_eq!(docx.class, FileClass::Document);
    assert_eq!(docx.icon, "󰈬");
    assert_eq!(docx.color, rgb(88, 142, 255));

    let odt = theme.resolve(Path::new("notes.odt"), EntryKind::File);
    assert_eq!(odt.class, FileClass::Document);
    assert_eq!(odt.icon, "󰈬");
    assert_eq!(odt.color, rgb(88, 142, 255));

    let markdown_file = theme.resolve(Path::new("notes.md"), EntryKind::File);
    assert_eq!(markdown_file.class, FileClass::Document);
    assert_eq!(markdown_file.icon, "");
    assert_eq!(markdown_file.color, rgb(211, 170, 124));

    let markdown = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(markdown.class, FileClass::Document);
    assert_eq!(markdown.icon, "");
    assert_eq!(markdown.color, rgb(211, 170, 124));

    let authors = theme.resolve(Path::new("AUTHORS"), EntryKind::File);
    assert_eq!(authors.class, FileClass::Document);
    assert_eq!(authors.icon, "󰭘");
    assert_eq!(authors.color, rgb(155, 143, 199));

    let authors_markdown = theme.resolve(Path::new("AUTHORS.md"), EntryKind::File);
    assert_eq!(authors_markdown.class, FileClass::Document);
    assert_eq!(authors_markdown.icon, "󰭘");
    assert_eq!(authors_markdown.color, rgb(155, 143, 199));

    let authors_text = theme.resolve(Path::new("AUTHORS.txt"), EntryKind::File);
    assert_eq!(authors_text.class, FileClass::Document);
    assert_eq!(authors_text.icon, "󰭘");
    assert_eq!(authors_text.color, rgb(155, 143, 199));

    let contributors = theme.resolve(Path::new("CONTRIBUTORS"), EntryKind::File);
    assert_eq!(contributors.class, FileClass::Document);
    assert_eq!(contributors.icon, "󰭘");
    assert_eq!(contributors.color, rgb(155, 143, 199));

    let contributors_markdown = theme.resolve(Path::new("CONTRIBUTORS.md"), EntryKind::File);
    assert_eq!(contributors_markdown.class, FileClass::Document);
    assert_eq!(contributors_markdown.icon, "󰭘");
    assert_eq!(contributors_markdown.color, rgb(155, 143, 199));

    let text = theme.resolve(Path::new("notes.txt"), EntryKind::File);
    assert_eq!(text.class, FileClass::Document);
    assert_eq!(text.icon, "");
    assert_eq!(text.color, rgb(174, 184, 199));

    let epub = theme.resolve(Path::new("novel.epub"), EntryKind::File);
    assert_eq!(epub.class, FileClass::Document);
    assert_eq!(epub.icon, "󱗖");
    assert_eq!(epub.color, rgb(211, 170, 124));

    let mobi = theme.resolve(Path::new("novel.mobi"), EntryKind::File);
    assert_eq!(mobi.class, FileClass::Document);
    assert_eq!(mobi.icon, "󱗖");
    assert_eq!(mobi.color, rgb(211, 170, 124));

    let azw3 = theme.resolve(Path::new("novel.azw3"), EntryKind::File);
    assert_eq!(azw3.class, FileClass::Document);
    assert_eq!(azw3.icon, "󱗖");
    assert_eq!(azw3.color, rgb(211, 170, 124));

    let comic = theme.resolve(Path::new("issue.cbz"), EntryKind::File);
    assert_eq!(comic.class, FileClass::Archive);
    assert_eq!(comic.icon, "󱗖");
    assert_eq!(comic.color, rgb(211, 170, 124));

    let documents_dir = theme.resolve(Path::new("Documents"), EntryKind::Directory);
    assert_eq!(documents_dir.class, FileClass::Directory);
    assert_eq!(documents_dir.icon, "󰲃");
    assert_eq!(documents_dir.color, rgb(141, 223, 109));

    let archive = theme.resolve(Path::new("bundle.zip"), EntryKind::File);
    assert_eq!(archive.class, FileClass::Archive);
    assert_eq!(archive.color, rgb(207, 111, 63));

    let video = theme.resolve(Path::new("clip.mp4"), EntryKind::File);
    assert_eq!(video.class, FileClass::Video);
    assert_eq!(video.icon, "");
    assert_eq!(video.color, rgb(255, 134, 216));

    let videos_dir = theme.resolve(Path::new("Videos"), EntryKind::Directory);
    assert_eq!(videos_dir.class, FileClass::Directory);
    assert_eq!(videos_dir.icon, "󰕧");
    assert_eq!(videos_dir.color, rgb(255, 134, 216));
}

#[test]
fn spreadsheets_and_presentations_get_family_specific_icons() {
    let theme = Theme::default_theme();

    let xlsx = theme.resolve(Path::new("budget.xlsx"), EntryKind::File);
    assert_eq!(xlsx.class, FileClass::Document);
    assert_eq!(xlsx.icon, "󱎏");
    assert_eq!(xlsx.color, rgb(78, 178, 116));

    let ods = theme.resolve(Path::new("budget.ods"), EntryKind::File);
    assert_eq!(ods.class, FileClass::Document);
    assert_eq!(ods.icon, "󱎏");
    assert_eq!(ods.color, rgb(78, 178, 116));

    let pptx = theme.resolve(Path::new("deck.pptx"), EntryKind::File);
    assert_eq!(pptx.class, FileClass::Document);
    assert_eq!(pptx.icon, "󱎐");
    assert_eq!(pptx.color, rgb(232, 139, 63));

    let odp = theme.resolve(Path::new("deck.odp"), EntryKind::File);
    assert_eq!(odp.class, FileClass::Document);
    assert_eq!(odp.icon, "󱎐");
    assert_eq!(odp.color, rgb(232, 139, 63));
}

#[test]
fn default_theme_uses_toml_icon_for_toml_files() {
    let theme = Theme::default_theme();

    let cargo = theme.resolve(Path::new("Cargo.toml"), EntryKind::File);
    assert_eq!(cargo.class, FileClass::Config);
    assert_eq!(cargo.icon, "");

    let pyproject = theme.resolve(Path::new("pyproject.toml"), EntryKind::File);
    assert_eq!(pyproject.class, FileClass::Config);
    assert_eq!(pyproject.icon, "");

    let rust_toolchain = theme.resolve(Path::new("rust-toolchain.toml"), EntryKind::File);
    assert_eq!(rust_toolchain.class, FileClass::Config);
    assert_eq!(rust_toolchain.icon, "");
}
