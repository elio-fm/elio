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

fn write_temp_file(label: &str, file_name: &str, contents: &str) -> (PathBuf, PathBuf) {
    let root = temp_path(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(file_name);
    fs::write(&path, contents).expect("failed to write temp file");
    (root, path)
}

fn assert_code_spec(
    preview: PreviewSpec,
    code_syntax: Option<&'static str>,
    code_backend: CodeBackend,
) {
    assert_eq!(preview.code_syntax, code_syntax);
    assert_eq!(preview.code_backend, code_backend);
}

#[test]
fn package_lock_uses_one_shared_definition() {
    let facts = inspect_path(Path::new("package-lock.json"), EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Data);
    assert_eq!(
        facts.preview.structured_format,
        Some(StructuredFormat::Json)
    );
    assert_code_spec(
        facts.preview,
        Some("json"),
        CodeBackend::Custom(CustomCodeKind::Json),
    );
}

#[test]
fn lockfile_variants_get_targeted_preview_support() {
    let uv = inspect_path(Path::new("uv.lock"), EntryKind::File);
    let flake = inspect_path(Path::new("flake.lock"), EntryKind::File);
    let gem = inspect_path(Path::new("Gemfile.lock"), EntryKind::File);
    let generic = inspect_path(Path::new("deps.lock"), EntryKind::File);

    assert_eq!(uv.preview.structured_format, Some(StructuredFormat::Toml));
    assert_code_spec(
        uv.preview,
        Some("toml"),
        CodeBackend::Custom(CustomCodeKind::Toml),
    );

    assert_eq!(
        flake.preview.structured_format,
        Some(StructuredFormat::Json)
    );
    assert_code_spec(
        flake.preview,
        Some("json"),
        CodeBackend::Custom(CustomCodeKind::Json),
    );

    assert_eq!(gem.specific_type_label, Some("Lockfile"));
    assert_code_spec(
        gem.preview,
        Some("ini"),
        CodeBackend::Custom(CustomCodeKind::Ini),
    );

    assert_eq!(generic.specific_type_label, Some("Lockfile"));
    assert_code_spec(
        generic.preview,
        Some("ini"),
        CodeBackend::Custom(CustomCodeKind::Ini),
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
    assert_code_spec(
        facts.preview,
        Some("json5"),
        CodeBackend::Custom(CustomCodeKind::Jsonc),
    );
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
fn shell_files_and_dotfiles_get_targeted_preview_support() {
    let shell = inspect_path(Path::new("deploy.sh"), EntryKind::File);
    let bashrc = inspect_path(Path::new(".bashrc"), EntryKind::File);
    let zsh = inspect_path(Path::new("prompt.zsh"), EntryKind::File);
    let fish = inspect_path(Path::new("config.fish"), EntryKind::File);
    let zshrc = inspect_path(Path::new(".zshrc"), EntryKind::File);

    assert_eq!(shell.builtin_class, FileClass::Code);
    assert_eq!(shell.specific_type_label, Some("Shell script"));
    assert_eq!(shell.preview.language_hint, Some("sh"));
    assert_code_spec(shell.preview, Some("sh"), CodeBackend::Syntect);

    assert_eq!(bashrc.builtin_class, FileClass::Config);
    assert_eq!(bashrc.specific_type_label, Some("Bash config"));
    assert_eq!(bashrc.preview.language_hint, Some("bash"));
    assert_code_spec(bashrc.preview, Some("bash"), CodeBackend::Syntect);

    assert_eq!(zsh.builtin_class, FileClass::Code);
    assert_eq!(zsh.specific_type_label, Some("Zsh script"));
    assert_eq!(zsh.preview.language_hint, Some("zsh"));
    assert_code_spec(zsh.preview, Some("zsh"), CodeBackend::Syntect);

    assert_eq!(fish.builtin_class, FileClass::Code);
    assert_eq!(fish.specific_type_label, Some("Fish script"));
    assert_eq!(fish.preview.language_hint, Some("fish"));
    assert_code_spec(fish.preview, Some("fish"), CodeBackend::Syntect);

    assert_eq!(zshrc.builtin_class, FileClass::Config);
    assert_eq!(zshrc.specific_type_label, Some("Zsh config"));
    assert_eq!(zshrc.preview.language_hint, Some("zsh"));
    assert_code_spec(zshrc.preview, Some("zsh"), CodeBackend::Syntect);
}

#[test]
fn extensionless_shebang_scripts_are_classified_as_shell_code() {
    let (root, path) = write_temp_file(
        "extensionless-bash-script",
        "apksigner",
        "#!/bin/bash\n#\n# Copyright (C) 2016 The Android Open Source Project\n#\n# Licensed under the Apache License, Version 2.0 (the \"License\");\n# you may not use this file except in compliance with the License.\n\nexec java -jar apksigner.jar \"$@\"\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Bash script"));
    assert_eq!(facts.preview.language_hint, Some("bash"));
    assert_code_spec(facts.preview, Some("bash"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
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
fn extensionless_elixir_scripts_are_classified_as_code() {
    let (root, path) = write_temp_file(
        "extensionless-elixir-script",
        "mix-task",
        "#!/usr/bin/env elixir\nIO.puts(\"hello\")\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Elixir script"));
    assert_eq!(facts.preview.language_hint, Some("elixir"));
    assert_code_spec(facts.preview, Some("elixir"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shebang_and_exact_name_detection_cover_new_languages() {
    let (perl_root, perl_path) = write_temp_file(
        "extensionless-perl-script",
        "tool",
        "#!/usr/bin/env perl\nprint \"elio\\n\";\n",
    );
    let perl = inspect_path(&perl_path, EntryKind::File);
    assert_eq!(perl.preview.language_hint, Some("perl"));
    assert_eq!(perl.specific_type_label, Some("Perl script"));
    fs::remove_dir_all(perl_root).expect("failed to remove temp root");

    let (r_root, r_path) = write_temp_file(
        "extensionless-r-script",
        "analysis",
        "#!/usr/bin/env Rscript\nprint('elio')\n",
    );
    let r = inspect_path(&r_path, EntryKind::File);
    assert_eq!(r.preview.language_hint, Some("r"));
    assert_eq!(r.specific_type_label, Some("R script"));
    fs::remove_dir_all(r_root).expect("failed to remove temp root");

    let dockerfile = inspect_path(Path::new("Containerfile"), EntryKind::File);
    assert_eq!(dockerfile.preview.language_hint, Some("dockerfile"));
    assert_eq!(dockerfile.specific_type_label, Some("Docker build file"));

    let just = inspect_path(Path::new(".justfile"), EntryKind::File);
    assert_eq!(just.preview.language_hint, Some("just"));
    assert_eq!(just.specific_type_label, Some("Justfile"));

    let deps = inspect_path(Path::new("deps.edn"), EntryKind::File);
    assert_eq!(deps.preview.language_hint, Some("clojure"));
    assert_eq!(deps.specific_type_label, Some("Clojure deps config"));

    let project = inspect_path(Path::new("project.clj"), EntryKind::File);
    assert_eq!(project.preview.language_hint, Some("clojure"));
    assert_eq!(project.specific_type_label, Some("Leiningen project"));
}

#[test]
fn extensionless_powershell_scripts_are_classified_as_code() {
    let (root, path) = write_temp_file(
        "extensionless-powershell-script",
        "elio-tool",
        "#!/usr/bin/env pwsh\nWrite-Host \"hello\"\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("PowerShell script"));
    assert_eq!(facts.preview.language_hint, Some("powershell"));
    assert_code_spec(facts.preview, Some("powershell"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn extensionless_babashka_scripts_are_classified_as_code() {
    let (root, path) = write_temp_file(
        "extensionless-babashka-script",
        "bb-task",
        "#!/usr/bin/env bb\n(println \"hello\")\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Clojure script"));
    assert_eq!(facts.preview.language_hint, Some("clojure"));
    assert_code_spec(facts.preview, Some("clojure"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
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
fn ini_style_conf_is_detected_from_contents() {
    let (root, path) = write_temp_file(
        "ini-conf",
        "settings.conf",
        "[Settings]\ncolor=blue\nenabled=true\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("ini"));
    assert_code_spec(
        facts.preview,
        Some("ini"),
        CodeBackend::Custom(CustomCodeKind::Ini),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn shell_style_conf_is_detected_from_contents() {
    let (root, path) = write_temp_file(
        "shell-conf",
        "module.conf",
        "MAKE=\"make -C src/ KERNELDIR=/lib/modules/${kernelver}/build\"\nAUTOINSTALL=yes\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("sh"));
    assert_code_spec(facts.preview, Some("sh"), CodeBackend::Syntect);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn ambiguous_conf_defaults_to_directive_config() {
    let (root, path) = write_temp_file(
        "directive-conf",
        "custom.conf",
        "font_size 11.5\nforeground #c0c6e2\nmap ctrl+c copy_to_clipboard\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("config"));
    assert_code_spec(
        facts.preview,
        Some("config"),
        CodeBackend::Custom(CustomCodeKind::DirectiveConf),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cfg_files_use_the_same_content_based_detection() {
    let (root, path) = write_temp_file(
        "directive-cfg",
        "custom.cfg",
        "font_size 11.5\nforeground #c0c6e2\nmap ctrl+c copy_to_clipboard\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_code_spec(
        facts.preview,
        Some("config"),
        CodeBackend::Custom(CustomCodeKind::DirectiveConf),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn config_modelines_can_force_directive_conf_without_name_overrides() {
    let (root, path) = write_temp_file(
        "kitty-modeline",
        "settings.conf",
        "# vim:ft=kitty\n[Settings]\ncolor=blue\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("kitty"));
    assert_code_spec(
        facts.preview,
        Some("kitty"),
        CodeBackend::Custom(CustomCodeKind::DirectiveConf),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn unsupported_modelines_are_ignored_for_conf_detection() {
    let (root, path) = write_temp_file(
        "unknown-modeline",
        "settings.conf",
        "# vim:ft=totallyunknown\n[Settings]\ncolor=blue\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Config);
    assert_eq!(facts.preview.language_hint, Some("ini"));
    assert_code_spec(
        facts.preview,
        Some("ini"),
        CodeBackend::Custom(CustomCodeKind::Ini),
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
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
    let cbz = inspect_path(Path::new("issue.cbz"), EntryKind::File);
    let cbr = inspect_path(Path::new("issue.cbr"), EntryKind::File);
    let seven_zip = inspect_path(Path::new("release.7z"), EntryKind::File);

    assert_eq!(tgz.builtin_class, FileClass::Archive);
    assert_eq!(tgz.specific_type_label, Some("TAR.GZ archive"));
    assert_eq!(txz.specific_type_label, Some("TAR.XZ archive"));
    assert_eq!(tbz2.specific_type_label, Some("TAR.BZ2 archive"));
    assert_eq!(zip.specific_type_label, Some("ZIP archive"));
    assert_eq!(cbz.specific_type_label, Some("Comic ZIP archive"));
    assert_eq!(cbr.specific_type_label, Some("Comic RAR archive"));
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
fn license_like_files_detect_specific_and_generic_licenses() {
    let (mit_root, mit_path) = write_temp_file(
        "mit-license",
        "LICENSE",
        "MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND.\n",
    );
    let mit = inspect_path(&mit_path, EntryKind::File);
    assert_eq!(mit.builtin_class, FileClass::License);
    assert_eq!(mit.specific_type_label, Some("MIT License"));
    assert_eq!(mit.preview.kind, PreviewKind::PlainText);
    fs::remove_dir_all(mit_root).expect("failed to remove temp root");

    let (apache_root, apache_path) = write_temp_file(
        "apache-license",
        "LICENSE.md",
        "# SPDX-License-Identifier: Apache-2.0\n\nLicensed under the Apache License, Version 2.0.\n",
    );
    let apache = inspect_path(&apache_path, EntryKind::File);
    assert_eq!(apache.builtin_class, FileClass::License);
    assert_eq!(apache.specific_type_label, Some("Apache License 2.0"));
    assert_eq!(apache.preview.kind, PreviewKind::Markdown);
    fs::remove_dir_all(apache_root).expect("failed to remove temp root");

    let (generic_root, generic_path) = write_temp_file(
        "generic-license",
        "LICENSE.txt",
        "Copyright (c) 2026 Example Corp.\nAll rights reserved.\nThis license governs internal use only.\nNo warranty is provided.\n",
    );
    let generic = inspect_path(&generic_path, EntryKind::File);
    assert_eq!(generic.builtin_class, FileClass::License);
    assert_eq!(generic.specific_type_label, Some("License document"));
    fs::remove_dir_all(generic_root).expect("failed to remove temp root");

    let (copying_root, copying_path) = write_temp_file(
        "copying-lesser",
        "COPYING.LESSER",
        "GNU LESSER GENERAL PUBLIC LICENSE\nVersion 2.1, February 1999\n\nThis library is free software; you can redistribute it and/or\nmodify it under the terms of the GNU Lesser General Public\nLicense as published by the Free Software Foundation; either\nversion 2.1 of the License, or (at your option) any later version.\n",
    );
    let copying = inspect_path(&copying_path, EntryKind::File);
    assert_eq!(copying.builtin_class, FileClass::License);
    assert_eq!(copying.specific_type_label, Some("GNU LGPL 2.1 or later"));
    fs::remove_dir_all(copying_root).expect("failed to remove temp root");

    let (hyphen_root, hyphen_path) = write_temp_file(
        "license-prefix",
        "license-mit",
        "MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND.\n",
    );
    let hyphen = inspect_path(&hyphen_path, EntryKind::File);
    assert_eq!(hyphen.builtin_class, FileClass::License);
    assert_eq!(hyphen.specific_type_label, Some("MIT License"));
    fs::remove_dir_all(hyphen_root).expect("failed to remove temp root");
}

#[test]
fn license_detection_requires_real_markers_not_just_a_filename() {
    let (root, path) = write_temp_file(
        "not-a-license",
        "LICENSE",
        "shopping list\n- apples\n- oranges\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::File);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn license_family_matching_rejects_middle_substrings_without_markers() {
    let (root, path) = write_temp_file(
        "license-middle-substring",
        "my-license-notes.txt",
        "notes about legal cleanup\nnot a real license file\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Document);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn spdx_marked_text_files_can_be_detected_without_license_filenames() {
    let (root, path) = write_temp_file(
        "spdx-text",
        "third-party.txt",
        "SPDX-License-Identifier: MIT\n\nRedistribution notes.\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::License);
    assert_eq!(facts.specific_type_label, Some("MIT License"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn additional_spdx_license_ids_are_classified_explicitly() {
    let cases = [
        ("cc-by-3", "CC-BY-3.0", "Creative Commons Attribution 3.0"),
        (
            "cc-by-at",
            "CC-BY-3.0-AT",
            "Creative Commons Attribution 3.0 Austria",
        ),
        (
            "cc-by-sa-jp",
            "CC-BY-SA-2.1-JP",
            "Creative Commons Attribution-ShareAlike 2.1 Japan",
        ),
        (
            "cc-by-sa-at",
            "CC-BY-SA-3.0-AT",
            "Creative Commons Attribution-ShareAlike 3.0 Austria",
        ),
        ("w3c", "W3C", "W3C Software Notice and License"),
        ("wtfpl", "WTFPL", "WTFPL"),
    ];

    for (label, spdx_id, expected) in cases {
        let (root, path) = write_temp_file(
            label,
            "LICENSE",
            &format!("SPDX-License-Identifier: {spdx_id}\n\nLicense text.\n"),
        );

        let facts = inspect_path(&path, EntryKind::File);

        assert_eq!(facts.builtin_class, FileClass::License);
        assert_eq!(facts.specific_type_label, Some(expected));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn high_signal_license_texts_are_detected_without_canonical_filenames() {
    let cases = [
        (
            "cc-by-at-text",
            "third-party.txt",
            "CREATIVE COMMONS IST KEINE RECHTSANWALTSKANZLEI UND LEISTET KEINE RECHTSBERATUNG.\n\nLizenz\n\nDER GEGENSTAND DIESER LIZENZ WIRD UNTER DEN BEDINGUNGEN DIESER CREATIVE COMMONS PUBLIC LICENSE ZUR VERFÜGUNG GESTELLT.\n\nSofern zwischen Ihnen und dem Lizenzgeber keine anderweitige Vereinbarung getroffen wurde und soweit Wahlfreiheit besteht, findet auf diesen Lizenzvertrag das Recht der Republik Österreich Anwendung.\n",
            "Creative Commons Attribution 3.0 Austria",
        ),
        (
            "cc-by-sa-at-text",
            "third-party.txt",
            "CREATIVE COMMONS IST KEINE RECHTSANWALTSKANZLEI UND LEISTET KEINE RECHTSBERATUNG.\n\nLizenz\n\nUnter \"Lizenzelementen\" werden im Sinne dieser Lizenz die folgenden übergeordneten Lizenzcharakteristika verstanden: \"Namensnennung\", \"Weitergabe unter gleichen Bedingungen\".\n\nSofern zwischen Ihnen und dem Lizenzgeber keine anderweitige Vereinbarung getroffen wurde und soweit Wahlfreiheit besteht, findet auf diesen Lizenzvertrag das Recht der Republik Österreich Anwendung.\n",
            "Creative Commons Attribution-ShareAlike 3.0 Austria",
        ),
        (
            "cc-by-sa-jp-text",
            "third-party.txt",
            "アトリビューション—シェアアライク 2.1\n（帰属—同一条件許諾）\n利用許諾\n",
            "Creative Commons Attribution-ShareAlike 2.1 Japan",
        ),
        (
            "w3c-text",
            "third-party.txt",
            "W3C SOFTWARE NOTICE AND LICENSE\n\nBy obtaining, using and/or copying this work, you (the licensee) agree that you have read, understood, and will comply with the following terms and conditions.\n",
            "W3C Software Notice and License",
        ),
        (
            "wtfpl-text",
            "third-party.txt",
            "DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE\nVersion 2, December 2004\n\nEveryone is permitted to copy and distribute verbatim or modified copies of this license document, and changing it is allowed as long as the name is changed.\n",
            "WTFPL",
        ),
    ];

    for (label, file_name, contents, expected) in cases {
        let (root, path) = write_temp_file(label, file_name, contents);

        let facts = inspect_path(&path, EntryKind::File);

        assert_eq!(facts.builtin_class, FileClass::License);
        assert_eq!(facts.specific_type_label, Some(expected));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}

#[test]
fn standalone_apache_license_text_is_detected_without_canonical_filename() {
    let (root, path) = write_temp_file(
        "apache-third-party",
        "third-party.txt",
        "Apache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/LICENSE-2.0\n\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::License);
    assert_eq!(facts.specific_type_label, Some("Apache License 2.0"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn phase_numbers_do_not_trigger_japanese_cc_license_detection() {
    let (root, path) = write_temp_file(
        "roadmap-phase-numbers",
        "RoadMap2026.txt",
        "Phase 2: The \"Modern Systems\" Language (Month 2)\n\nGetting Started with Rust (LFEL1002) [1.5h]: A quick syntax primer.\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Document);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn diff_like_numeric_text_does_not_trigger_japanese_cc_license_detection() {
    let (root, path) = write_temp_file(
        "diff-like-numbers",
        "undo this.txt",
        "undo this\n\n145 app frame state preview content area some rect\n146 x 2\n147 y 3\n148 width 48\n149 height 20\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Document);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn embedded_license_headers_do_not_turn_shell_wrappers_into_license_files() {
    let (root, path) = write_temp_file(
        "android-shell-wrapper",
        "lld",
        "#!/bin/bash\n#\n# Copyright (C) 2020 The Android Open Source Project\n#\n# Licensed under the Apache License, Version 2.0 (the \"License\");\n# you may not use this file except in compliance with the License.\n# You may obtain a copy of the License at\n#\n#     http://www.apache.org/licenses/LICENSE-2.0\n#\n# Unless required by applicable law or agreed to in writing, software\n# distributed under the License is distributed on an \"AS IS\" BASIS,\n# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.\n\n$(dirname \"$0\")/lld-bin/lld \"$@\"\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Code);
    assert_eq!(facts.specific_type_label, Some("Bash script"));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn notice_files_with_embedded_license_text_are_not_classified_as_licenses() {
    let (root, path) = write_temp_file(
        "android-notice",
        "NOTICE.txt",
        "==============================================================================\nAndroid used by:\n  sdk-repo-linux-build-tools.zip\n\nApache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/\n\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n",
    );

    let facts = inspect_path(&path, EntryKind::File);

    assert_eq!(facts.builtin_class, FileClass::Document);
    assert_eq!(facts.specific_type_label, None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
