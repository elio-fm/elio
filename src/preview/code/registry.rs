use crate::file_info::{CodeBackend, CustomCodeKind, PreviewSpec, StructuredFormat};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredLanguage {
    pub canonical_id: &'static str,
    pub display_label: &'static str,
    pub backend: CodeBackend,
    pub structured_format: Option<StructuredFormat>,
}

impl RegisteredLanguage {
    pub(crate) const fn preview_spec(self) -> PreviewSpec {
        PreviewSpec::code(self.canonical_id, self.backend, self.structured_format)
    }
}

#[derive(Clone, Copy)]
struct RegistryEntry {
    language: RegisteredLanguage,
    extensions: &'static [&'static str],
    exact_filenames: &'static [&'static str],
    shebang_interpreters: &'static [&'static str],
    modelines: &'static [&'static str],
    markdown_fences: &'static [&'static str],
}

const fn language(
    canonical_id: &'static str,
    display_label: &'static str,
    backend: CodeBackend,
    structured_format: Option<StructuredFormat>,
) -> RegisteredLanguage {
    RegisteredLanguage {
        canonical_id,
        display_label,
        backend,
        structured_format,
    }
}

const LANGUAGES: &[RegistryEntry] = &[
    RegistryEntry {
        language: language(
            "json",
            "JSON",
            CodeBackend::Custom(CustomCodeKind::Json),
            Some(StructuredFormat::Json),
        ),
        extensions: &["json"],
        exact_filenames: &[
            "package.json",
            "package-lock.json",
            "tsconfig.json",
            "deno.json",
            "composer.lock",
            "pipfile.lock",
            "flake.lock",
        ],
        shebang_interpreters: &[],
        modelines: &["json"],
        markdown_fences: &["json"],
    },
    RegistryEntry {
        language: language(
            "jsonc",
            "JSONC",
            CodeBackend::Custom(CustomCodeKind::Jsonc),
            Some(StructuredFormat::Jsonc),
        ),
        extensions: &["jsonc"],
        exact_filenames: &["deno.jsonc"],
        shebang_interpreters: &[],
        modelines: &["jsonc"],
        markdown_fences: &["jsonc"],
    },
    RegistryEntry {
        language: language(
            "json5",
            "JSON5",
            CodeBackend::Custom(CustomCodeKind::Jsonc),
            Some(StructuredFormat::Json5),
        ),
        extensions: &["json5"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["json5"],
        markdown_fences: &["json5"],
    },
    RegistryEntry {
        language: language(
            "toml",
            "TOML",
            CodeBackend::Custom(CustomCodeKind::Toml),
            Some(StructuredFormat::Toml),
        ),
        extensions: &["toml"],
        exact_filenames: &["cargo.lock", "poetry.lock", "uv.lock"],
        shebang_interpreters: &[],
        modelines: &["toml"],
        markdown_fences: &["toml"],
    },
    RegistryEntry {
        language: language(
            "yaml",
            "YAML",
            CodeBackend::Custom(CustomCodeKind::Yaml),
            Some(StructuredFormat::Yaml),
        ),
        extensions: &["yaml", "yml"],
        exact_filenames: &[
            "compose.yml",
            "compose.yaml",
            "docker-compose.yml",
            "docker-compose.yaml",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
        ],
        shebang_interpreters: &[],
        modelines: &["yaml", "yml"],
        markdown_fences: &["yaml", "yml"],
    },
    RegistryEntry {
        language: language(
            "dotenv",
            ".env",
            CodeBackend::Custom(CustomCodeKind::Ini),
            Some(StructuredFormat::Dotenv),
        ),
        extensions: &["env"],
        exact_filenames: &[".env"],
        shebang_interpreters: &[],
        modelines: &["dotenv"],
        markdown_fences: &["dotenv"],
    },
    RegistryEntry {
        language: language(
            "log",
            "Log",
            CodeBackend::Custom(CustomCodeKind::Log),
            Some(StructuredFormat::Log),
        ),
        extensions: &["log"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["log"],
        markdown_fences: &["log"],
    },
    RegistryEntry {
        language: language("ini", "INI", CodeBackend::Custom(CustomCodeKind::Ini), None),
        extensions: &["ini", "keys", "lock"],
        exact_filenames: &["gemfile.lock", "bun.lock"],
        shebang_interpreters: &[],
        modelines: &["ini", "dosini"],
        markdown_fences: &["ini", "dosini"],
    },
    RegistryEntry {
        language: language(
            "desktop",
            "Desktop Entry",
            CodeBackend::Custom(CustomCodeKind::DesktopEntry),
            None,
        ),
        extensions: &["desktop"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["desktop"],
        markdown_fences: &["desktop"],
    },
    RegistryEntry {
        language: language(
            "config",
            "Directive config",
            CodeBackend::Custom(CustomCodeKind::DirectiveConf),
            None,
        ),
        extensions: &["conf", "cfg"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["conf", "cfg", "config"],
        markdown_fences: &["conf", "cfg", "config"],
    },
    RegistryEntry {
        language: language(
            "kitty",
            "Kitty",
            CodeBackend::Custom(CustomCodeKind::DirectiveConf),
            None,
        ),
        extensions: &[],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["kitty"],
        markdown_fences: &["kitty"],
    },
    RegistryEntry {
        language: language(
            "mpv",
            "MPV",
            CodeBackend::Custom(CustomCodeKind::DirectiveConf),
            None,
        ),
        extensions: &[],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["mpv"],
        markdown_fences: &["mpv"],
    },
    RegistryEntry {
        language: language(
            "btop",
            "btop",
            CodeBackend::Custom(CustomCodeKind::DirectiveConf),
            None,
        ),
        extensions: &[],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["btop"],
        markdown_fences: &["btop"],
    },
    RegistryEntry {
        language: language("html", "HTML", CodeBackend::Syntect, None),
        extensions: &["html", "htm", "xhtml"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["html"],
        markdown_fences: &["html"],
    },
    RegistryEntry {
        language: language("xml", "XML", CodeBackend::Syntect, None),
        extensions: &["xml", "xsd", "xsl", "xslt", "svg"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["xml", "svg", "markup"],
        markdown_fences: &["xml", "xhtml", "svg", "markup"],
    },
    RegistryEntry {
        language: language("css", "CSS", CodeBackend::Syntect, None),
        extensions: &["css"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["css"],
        markdown_fences: &["css"],
    },
    RegistryEntry {
        language: language("scss", "SCSS", CodeBackend::Syntect, None),
        extensions: &["scss"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["scss"],
        markdown_fences: &["scss"],
    },
    RegistryEntry {
        language: language("sass", "Sass", CodeBackend::Syntect, None),
        extensions: &["sass"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["sass"],
        markdown_fences: &["sass"],
    },
    RegistryEntry {
        language: language("less", "Less", CodeBackend::Syntect, None),
        extensions: &["less"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["less"],
        markdown_fences: &["less"],
    },
    RegistryEntry {
        language: language("javascript", "JavaScript", CodeBackend::Syntect, None),
        extensions: &["js", "mjs", "cjs"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["javascript"],
        markdown_fences: &["js", "javascript"],
    },
    RegistryEntry {
        language: language("jsx", "JSX", CodeBackend::Syntect, None),
        extensions: &["jsx"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["jsx"],
        markdown_fences: &["jsx"],
    },
    RegistryEntry {
        language: language("typescript", "TypeScript", CodeBackend::Syntect, None),
        extensions: &["ts", "mts", "cts"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["typescript"],
        markdown_fences: &["ts", "typescript"],
    },
    RegistryEntry {
        language: language("tsx", "TSX", CodeBackend::Syntect, None),
        extensions: &["tsx"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["tsx"],
        markdown_fences: &["tsx"],
    },
    RegistryEntry {
        language: language("sql", "SQL", CodeBackend::Syntect, None),
        extensions: &["sql"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["sql"],
        markdown_fences: &["sql"],
    },
    RegistryEntry {
        language: language("diff", "Diff", CodeBackend::Syntect, None),
        extensions: &["diff", "patch"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["diff", "patch"],
        markdown_fences: &["diff", "patch"],
    },
    RegistryEntry {
        language: language("dockerfile", "Dockerfile", CodeBackend::Syntect, None),
        extensions: &[],
        exact_filenames: &["dockerfile", "containerfile"],
        shebang_interpreters: &[],
        modelines: &["dockerfile"],
        markdown_fences: &["dockerfile", "docker"],
    },
    RegistryEntry {
        language: language("hcl", "HCL", CodeBackend::Syntect, None),
        extensions: &["hcl"],
        exact_filenames: &[".terraform.lock.hcl"],
        shebang_interpreters: &[],
        modelines: &["hcl"],
        markdown_fences: &["hcl"],
    },
    RegistryEntry {
        language: language("terraform", "Terraform", CodeBackend::Syntect, None),
        extensions: &["tf", "tfvars", "tfbackend"],
        exact_filenames: &["terraform.rc", ".terraformrc"],
        shebang_interpreters: &[],
        modelines: &["terraform", "tf", "tfvars"],
        markdown_fences: &["terraform", "tf", "tfvars"],
    },
    RegistryEntry {
        language: language("groovy", "Groovy", CodeBackend::Syntect, None),
        extensions: &["groovy", "gvy", "gradle"],
        exact_filenames: &["build.gradle", "settings.gradle", "init.gradle"],
        shebang_interpreters: &["groovy"],
        modelines: &["groovy", "gradle"],
        markdown_fences: &["groovy", "gradle"],
    },
    RegistryEntry {
        language: language("scala", "Scala", CodeBackend::Syntect, None),
        extensions: &["scala", "sbt"],
        exact_filenames: &["build.sbt"],
        shebang_interpreters: &["scala"],
        modelines: &["scala", "sbt"],
        markdown_fences: &["scala", "sbt"],
    },
    RegistryEntry {
        language: language("perl", "Perl", CodeBackend::Syntect, None),
        extensions: &["pl", "pm", "pod", "t"],
        exact_filenames: &["cpanfile"],
        shebang_interpreters: &["perl"],
        modelines: &["perl", "pl", "pm"],
        markdown_fences: &["perl", "pl", "pm"],
    },
    RegistryEntry {
        language: language("haskell", "Haskell", CodeBackend::Syntect, None),
        extensions: &["hs", "lhs"],
        exact_filenames: &[],
        shebang_interpreters: &["runhaskell"],
        modelines: &["haskell", "hs", "lhs"],
        markdown_fences: &["haskell", "hs", "lhs"],
    },
    RegistryEntry {
        language: language("julia", "Julia", CodeBackend::Syntect, None),
        extensions: &["jl"],
        exact_filenames: &[],
        shebang_interpreters: &["julia"],
        modelines: &["julia", "jl"],
        markdown_fences: &["julia", "jl"],
    },
    RegistryEntry {
        language: language("r", "R", CodeBackend::Syntect, None),
        extensions: &["r"],
        exact_filenames: &[".rprofile"],
        shebang_interpreters: &["rscript"],
        modelines: &["r"],
        markdown_fences: &["r", "rscript"],
    },
    RegistryEntry {
        language: language("just", "Just", CodeBackend::Syntect, None),
        extensions: &[],
        exact_filenames: &["justfile", ".justfile"],
        shebang_interpreters: &[],
        modelines: &["just"],
        markdown_fences: &["just"],
    },
    RegistryEntry {
        language: language("rust", "Rust", CodeBackend::Syntect, None),
        extensions: &["rs"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["rust", "rs"],
        markdown_fences: &["rust", "rs"],
    },
    RegistryEntry {
        language: language("go", "Go", CodeBackend::Syntect, None),
        extensions: &["go"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["go", "golang"],
        markdown_fences: &["go", "golang"],
    },
    RegistryEntry {
        language: language("c", "C", CodeBackend::Syntect, None),
        extensions: &["c", "h"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["c", "h"],
        markdown_fences: &["c", "h"],
    },
    RegistryEntry {
        language: language("cpp", "C++", CodeBackend::Syntect, None),
        extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["cpp", "c++", "cc", "cxx", "hpp", "hh", "hxx"],
        markdown_fences: &["cpp", "c++", "cc", "cxx", "hpp", "hh", "hxx"],
    },
    RegistryEntry {
        language: language("cs", "C#", CodeBackend::Syntect, None),
        extensions: &["cs", "csx"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["cs", "csharp", "c#"],
        markdown_fences: &["cs", "csharp", "c#"],
    },
    RegistryEntry {
        language: language("java", "Java", CodeBackend::Syntect, None),
        extensions: &["java"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["java"],
        markdown_fences: &["java"],
    },
    RegistryEntry {
        language: language("dart", "Dart", CodeBackend::Syntect, None),
        extensions: &["dart"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["dart"],
        markdown_fences: &["dart"],
    },
    RegistryEntry {
        language: language("zig", "Zig", CodeBackend::Syntect, None),
        extensions: &["zig"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["zig"],
        markdown_fences: &["zig"],
    },
    RegistryEntry {
        language: language("php", "PHP", CodeBackend::Syntect, None),
        extensions: &["php"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["php"],
        markdown_fences: &["php"],
    },
    RegistryEntry {
        language: language("swift", "Swift", CodeBackend::Syntect, None),
        extensions: &["swift"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["swift"],
        markdown_fences: &["swift"],
    },
    RegistryEntry {
        language: language("kotlin", "Kotlin", CodeBackend::Syntect, None),
        extensions: &["kt", "kts"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["kotlin", "kt", "kts"],
        markdown_fences: &["kotlin", "kt", "kts"],
    },
    RegistryEntry {
        language: language("elixir", "Elixir", CodeBackend::Syntect, None),
        extensions: &["ex", "exs"],
        exact_filenames: &[],
        shebang_interpreters: &["elixir"],
        modelines: &["elixir", "ex", "exs"],
        markdown_fences: &["elixir", "ex", "exs"],
    },
    RegistryEntry {
        language: language("fortran", "Fortran", CodeBackend::Syntect, None),
        extensions: &["f", "for", "f90", "f95", "f03", "f08", "fpp"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["fortran", "f90", "f95", "f03", "f08"],
        markdown_fences: &["fortran", "f90", "f95", "f03", "f08"],
    },
    RegistryEntry {
        language: language("cobol", "COBOL", CodeBackend::Syntect, None),
        extensions: &["cbl", "cob", "cobol", "cpy"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["cobol", "cbl", "cob", "cpy"],
        markdown_fences: &["cobol", "cbl", "cob", "cpy"],
    },
    RegistryEntry {
        language: language("clojure", "Clojure", CodeBackend::Syntect, None),
        extensions: &["clj", "cljs", "cljc", "edn"],
        exact_filenames: &["project.clj", "deps.edn", "bb.edn", "shadow-cljs.edn"],
        shebang_interpreters: &["clojure", "clj", "bb"],
        modelines: &["clojure", "clj", "cljs", "cljc", "edn"],
        markdown_fences: &["clojure", "clj", "cljs", "cljc", "edn"],
    },
    RegistryEntry {
        language: language("ruby", "Ruby", CodeBackend::Syntect, None),
        extensions: &["rb"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["ruby", "rb"],
        markdown_fences: &["ruby", "rb"],
    },
    RegistryEntry {
        language: language("python", "Python", CodeBackend::Syntect, None),
        extensions: &["py", "pyi", "pyw", "pyx"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["python", "py"],
        markdown_fences: &["python", "py"],
    },
    RegistryEntry {
        language: language("lua", "Lua", CodeBackend::Syntect, None),
        extensions: &["lua"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["lua"],
        markdown_fences: &["lua"],
    },
    RegistryEntry {
        language: language("make", "Makefile", CodeBackend::Syntect, None),
        extensions: &["mk", "mak"],
        exact_filenames: &["makefile", "gnumakefile", "bsdmakefile"],
        shebang_interpreters: &[],
        modelines: &["make", "makefile"],
        markdown_fences: &["make", "makefile"],
    },
    RegistryEntry {
        language: language("sh", "Shell", CodeBackend::Syntect, None),
        extensions: &["sh"],
        exact_filenames: &[".profile", ".xprofile", ".xsessionrc", ".envrc"],
        shebang_interpreters: &["sh"],
        modelines: &["sh", "shell"],
        markdown_fences: &["sh", "shell"],
    },
    RegistryEntry {
        language: language("bash", "Bash", CodeBackend::Syntect, None),
        extensions: &["bash"],
        exact_filenames: &[
            ".bashrc",
            ".bash_profile",
            ".bash_login",
            ".bash_logout",
            ".bash_aliases",
            "pkgbuild",
        ],
        shebang_interpreters: &["bash"],
        modelines: &["bash"],
        markdown_fences: &["bash"],
    },
    RegistryEntry {
        language: language("zsh", "Zsh", CodeBackend::Syntect, None),
        extensions: &["zsh"],
        exact_filenames: &[".zshrc", ".zprofile", ".zshenv", ".zlogin", ".zlogout"],
        shebang_interpreters: &["zsh"],
        modelines: &["zsh"],
        markdown_fences: &["zsh"],
    },
    RegistryEntry {
        language: language("ksh", "KornShell", CodeBackend::Syntect, None),
        extensions: &["ksh"],
        exact_filenames: &[".kshrc", ".mkshrc"],
        shebang_interpreters: &["ksh"],
        modelines: &["ksh"],
        markdown_fences: &["ksh"],
    },
    RegistryEntry {
        language: language("fish", "Fish", CodeBackend::Syntect, None),
        extensions: &["fish"],
        exact_filenames: &[],
        shebang_interpreters: &["fish"],
        modelines: &["fish"],
        markdown_fences: &["fish"],
    },
    RegistryEntry {
        language: language("powershell", "PowerShell", CodeBackend::Syntect, None),
        extensions: &["ps1", "psm1", "psd1"],
        exact_filenames: &[],
        shebang_interpreters: &["pwsh", "powershell"],
        modelines: &["powershell", "pwsh", "ps1"],
        markdown_fences: &["powershell", "pwsh", "ps1"],
    },
    RegistryEntry {
        language: language("nix", "Nix", CodeBackend::Syntect, None),
        extensions: &["nix"],
        exact_filenames: &[],
        shebang_interpreters: &[],
        modelines: &["nix"],
        markdown_fences: &["nix"],
    },
    RegistryEntry {
        language: language("cmake", "CMake", CodeBackend::Syntect, None),
        extensions: &["cmake"],
        exact_filenames: &["cmakelists.txt"],
        shebang_interpreters: &[],
        modelines: &["cmake"],
        markdown_fences: &["cmake"],
    },
];

pub(crate) fn language_for_extension(ext: &str) -> Option<RegisteredLanguage> {
    language_for_alias(ext, |entry| entry.extensions)
}

pub(crate) fn language_for_exact_name(name: &str) -> Option<RegisteredLanguage> {
    let normalized = normalize(name);
    if is_env_name(&normalized) {
        return language_for_code_syntax("dotenv");
    }
    LANGUAGES
        .iter()
        .find(|entry| contains(entry.exact_filenames, &normalized))
        .map(|entry| entry.language)
}

pub(crate) fn language_for_shebang(interpreter: &str) -> Option<RegisteredLanguage> {
    language_for_alias(interpreter, |entry| entry.shebang_interpreters)
}

pub(crate) fn language_for_modeline(token: &str) -> Option<RegisteredLanguage> {
    language_for_alias(token, |entry| entry.modelines)
}

pub(crate) fn language_for_markdown_fence(token: &str) -> Option<RegisteredLanguage> {
    language_for_alias(token, |entry| entry.markdown_fences)
}

pub(crate) fn language_for_code_syntax(code_syntax: &str) -> Option<RegisteredLanguage> {
    let normalized = normalize(code_syntax);
    LANGUAGES
        .iter()
        .find(|entry| entry.language.canonical_id == normalized)
        .map(|entry| entry.language)
}

pub(crate) fn display_label_for_code_syntax(code_syntax: &str) -> Option<&'static str> {
    language_for_code_syntax(code_syntax).map(|language| language.display_label)
}

fn language_for_alias(
    value: &str,
    aliases: impl Fn(&RegistryEntry) -> &'static [&'static str],
) -> Option<RegisteredLanguage> {
    let normalized = normalize(value);
    LANGUAGES
        .iter()
        .find(|entry| contains(aliases(entry), &normalized))
        .map(|entry| entry.language)
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn contains(values: &[&str], needle: &str) -> bool {
    values.contains(&needle)
}

fn is_env_name(name: &str) -> bool {
    name == ".env" || name.starts_with(".env.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::code::syntax_manifest::CURATED_SYNTAXES;

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
        let mut registered = LANGUAGES
            .iter()
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
        let mut custom_entries = LANGUAGES
            .iter()
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
}
