use super::*;

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
