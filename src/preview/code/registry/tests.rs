use super::*;
use crate::{
    file_info::{CodeBackend, CustomCodeKind, StructuredFormat},
    preview::code::syntax_manifest::CURATED_SYNTAXES,
};

fn assert_registered_language(
    language: Option<RegisteredLanguage>,
    canonical_id: &'static str,
    display_label: &'static str,
    backend: CodeBackend,
    structured_format: Option<StructuredFormat>,
) {
    let language = language.expect("language should resolve");
    assert_eq!(language.canonical_id, canonical_id);
    assert_eq!(language.display_label, display_label);
    assert_eq!(language.backend, backend);
    assert_eq!(language.structured_format, structured_format);
}

#[test]
fn extension_lookup_returns_canonical_language_ids() {
    assert_eq!(
        language_for_extension("js").map(|language| language.canonical_id),
        Some("javascript")
    );
    assert_eq!(
        language_for_extension("sql").map(|language| language.canonical_id),
        Some("sql")
    );
    assert_eq!(
        language_for_extension("tfvars").map(|language| language.canonical_id),
        Some("terraform")
    );
    assert_eq!(
        language_for_extension("groovy").map(|language| language.canonical_id),
        Some("groovy")
    );
    assert_eq!(
        language_for_extension("hs").map(|language| language.canonical_id),
        Some("haskell")
    );
    assert_eq!(
        language_for_extension("csx").map(|language| language.canonical_id),
        Some("cs")
    );
    assert_eq!(
        language_for_extension("kts").map(|language| language.canonical_id),
        Some("kotlin")
    );
    assert_eq!(
        language_for_extension("exs").map(|language| language.canonical_id),
        Some("elixir")
    );
    assert_eq!(
        language_for_extension("f90").map(|language| language.canonical_id),
        Some("fortran")
    );
    assert_eq!(
        language_for_extension("cpy").map(|language| language.canonical_id),
        Some("cobol")
    );
    assert_eq!(
        language_for_extension("cljc").map(|language| language.canonical_id),
        Some("clojure")
    );
    assert_eq!(
        language_for_extension("tsx").map(|language| language.canonical_id),
        Some("tsx")
    );
    assert_eq!(
        language_for_extension("ps1").map(|language| language.canonical_id),
        Some("powershell")
    );
    assert_eq!(
        language_for_extension("json5").map(|language| language.canonical_id),
        Some("json5")
    );
}

#[test]
fn exact_name_lookup_handles_lockfiles_and_env_variants() {
    assert_eq!(
        language_for_exact_name("uv.lock").map(|language| language.canonical_id),
        Some("toml")
    );
    assert_eq!(
        language_for_exact_name("Dockerfile").map(|language| language.canonical_id),
        Some("dockerfile")
    );
    assert_eq!(
        language_for_exact_name(".terraform.lock.hcl").map(|language| language.canonical_id),
        Some("hcl")
    );
    assert_eq!(
        language_for_exact_name("build.gradle").map(|language| language.canonical_id),
        Some("groovy")
    );
    assert_eq!(
        language_for_exact_name("Justfile").map(|language| language.canonical_id),
        Some("just")
    );
    assert_eq!(
        language_for_exact_name("deps.edn").map(|language| language.canonical_id),
        Some("clojure")
    );
    assert_eq!(
        language_for_exact_name(".env.local").map(|language| language.canonical_id),
        Some("dotenv")
    );
}

#[test]
fn shebang_and_modeline_lookups_share_one_source_of_truth() {
    assert_eq!(
        language_for_shebang("bash").map(|language| language.canonical_id),
        Some("bash")
    );
    assert_eq!(
        language_for_shebang("elixir").map(|language| language.canonical_id),
        Some("elixir")
    );
    assert_eq!(
        language_for_shebang("pwsh").map(|language| language.canonical_id),
        Some("powershell")
    );
    assert_eq!(
        language_for_shebang("perl").map(|language| language.canonical_id),
        Some("perl")
    );
    assert_eq!(
        language_for_shebang("rscript").map(|language| language.canonical_id),
        Some("r")
    );
    assert_eq!(
        language_for_shebang("bb").map(|language| language.canonical_id),
        Some("clojure")
    );
    assert_eq!(
        language_for_modeline(" kitty ").map(|language| language.canonical_id),
        Some("kitty")
    );
    assert_eq!(
        language_for_modeline("json5").map(|language| language.canonical_id),
        Some("json5")
    );
    assert_eq!(
        language_for_modeline("csharp").map(|language| language.canonical_id),
        Some("cs")
    );
    assert_eq!(
        language_for_modeline("kts").map(|language| language.canonical_id),
        Some("kotlin")
    );
    assert_eq!(
        language_for_modeline("powershell").map(|language| language.canonical_id),
        Some("powershell")
    );
    assert_eq!(
        language_for_modeline("fortran").map(|language| language.canonical_id),
        Some("fortran")
    );
    assert_eq!(
        language_for_modeline("cobol").map(|language| language.canonical_id),
        Some("cobol")
    );
    assert_eq!(
        language_for_modeline("cljs").map(|language| language.canonical_id),
        Some("clojure")
    );
    assert_eq!(
        language_for_modeline("terraform").map(|language| language.canonical_id),
        Some("terraform")
    );
    assert_eq!(
        language_for_modeline("gradle").map(|language| language.canonical_id),
        Some("groovy")
    );
}

#[test]
fn markdown_fence_lookup_supports_common_aliases() {
    assert_eq!(
        language_for_markdown_fence("rs").map(|language| language.canonical_id),
        Some("rust")
    );
    assert_eq!(
        language_for_markdown_fence("shell").map(|language| language.canonical_id),
        Some("sh")
    );
    assert_eq!(
        language_for_markdown_fence("c++").map(|language| language.canonical_id),
        Some("cpp")
    );
    assert_eq!(
        language_for_markdown_fence("c#").map(|language| language.canonical_id),
        Some("cs")
    );
    assert_eq!(
        language_for_markdown_fence("exs").map(|language| language.canonical_id),
        Some("elixir")
    );
    assert_eq!(
        language_for_markdown_fence("pwsh").map(|language| language.canonical_id),
        Some("powershell")
    );
    assert_eq!(
        language_for_markdown_fence("f90").map(|language| language.canonical_id),
        Some("fortran")
    );
    assert_eq!(
        language_for_markdown_fence("cob").map(|language| language.canonical_id),
        Some("cobol")
    );
    assert_eq!(
        language_for_markdown_fence("clj").map(|language| language.canonical_id),
        Some("clojure")
    );
    assert_eq!(
        language_for_markdown_fence("docker").map(|language| language.canonical_id),
        Some("dockerfile")
    );
    assert_eq!(
        language_for_markdown_fence("terraform").map(|language| language.canonical_id),
        Some("terraform")
    );
    assert_eq!(
        language_for_markdown_fence("rscript").map(|language| language.canonical_id),
        Some("r")
    );
}

#[test]
fn registry_resolution_preserves_backend_and_structured_metadata() {
    assert_registered_language(
        language_for_extension("yaml"),
        "yaml",
        "YAML",
        CodeBackend::Custom(CustomCodeKind::Yaml),
        Some(StructuredFormat::Yaml),
    );
    assert_registered_language(
        language_for_exact_name(".env.production"),
        "dotenv",
        ".env",
        CodeBackend::Custom(CustomCodeKind::Ini),
        Some(StructuredFormat::Dotenv),
    );
    assert_registered_language(
        language_for_exact_name("Cargo.lock"),
        "toml",
        "TOML",
        CodeBackend::Custom(CustomCodeKind::Toml),
        Some(StructuredFormat::Toml),
    );
    assert_registered_language(
        language_for_shebang("bash"),
        "bash",
        "Bash",
        CodeBackend::Syntect,
        None,
    );
    assert_registered_language(
        language_for_modeline(" c++ "),
        "cpp",
        "C++",
        CodeBackend::Syntect,
        None,
    );
    assert_registered_language(
        language_for_markdown_fence("shell"),
        "sh",
        "Shell",
        CodeBackend::Syntect,
        None,
    );
    assert_registered_language(
        language_for_shebang("pwsh"),
        "powershell",
        "PowerShell",
        CodeBackend::Syntect,
        None,
    );
    assert_registered_language(
        language_for_exact_name("Dockerfile"),
        "dockerfile",
        "Dockerfile",
        CodeBackend::Syntect,
        None,
    );
    assert_registered_language(
        language_for_markdown_fence("terraform"),
        "terraform",
        "Terraform",
        CodeBackend::Syntect,
        None,
    );
}

#[test]
fn preview_specs_round_trip_registry_metadata() {
    let json5 = language_for_code_syntax("json5")
        .expect("json5 should be available")
        .preview_spec();
    assert_eq!(json5.code_syntax, Some("json5"));
    assert_eq!(
        json5.code_backend,
        CodeBackend::Custom(CustomCodeKind::Jsonc)
    );
    assert_eq!(json5.structured_format, Some(StructuredFormat::Json5));

    let bash = language_for_code_syntax("bash")
        .expect("bash should be available")
        .preview_spec();
    assert_eq!(bash.code_syntax, Some("bash"));
    assert_eq!(bash.code_backend, CodeBackend::Syntect);
    assert_eq!(bash.structured_format, None);
}

#[test]
fn syntect_registry_entries_match_curated_support_matrix() {
    let mut registered = data::all_languages()
        .filter(|entry| entry.language.backend == CodeBackend::Syntect)
        .map(|entry| entry.language.canonical_id)
        .collect::<Vec<_>>();
    registered.sort_unstable();

    let mut curated = CURATED_SYNTAXES
        .iter()
        .map(|syntax| syntax.canonical_id)
        .collect::<Vec<_>>();
    curated.sort_unstable();

    assert_eq!(registered, curated);
}

#[test]
fn custom_registry_entries_stay_limited_to_product_specific_renderers() {
    let mut custom_entries = data::all_languages()
        .filter_map(|entry| match entry.language.backend {
            CodeBackend::Custom(kind) => Some((entry.language.canonical_id, kind)),
            CodeBackend::Plain | CodeBackend::Syntect => None,
        })
        .collect::<Vec<_>>();
    custom_entries.sort_unstable_by_key(|(canonical_id, _)| *canonical_id);

    assert_eq!(
        custom_entries,
        vec![
            ("btop", CustomCodeKind::DirectiveConf),
            ("config", CustomCodeKind::DirectiveConf),
            ("desktop", CustomCodeKind::DesktopEntry),
            ("dotenv", CustomCodeKind::Ini),
            ("ini", CustomCodeKind::Ini),
            ("json", CustomCodeKind::Json),
            ("json5", CustomCodeKind::Jsonc),
            ("jsonc", CustomCodeKind::Jsonc),
            ("kitty", CustomCodeKind::DirectiveConf),
            ("log", CustomCodeKind::Log),
            ("mpv", CustomCodeKind::DirectiveConf),
            ("toml", CustomCodeKind::Toml),
            ("yaml", CustomCodeKind::Yaml),
        ]
    );
}
