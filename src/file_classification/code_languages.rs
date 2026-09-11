use crate::file_classification::{CodeBackend, CustomCodeKind, PreviewSpec, StructuredFormat};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CodeLanguage {
    pub canonical_id: &'static str,
    pub display_label: &'static str,
    pub backend: CodeBackend,
    pub structured_format: Option<StructuredFormat>,
}

impl CodeLanguage {
    pub(crate) const fn preview_spec(self) -> PreviewSpec {
        PreviewSpec::code(self.canonical_id, self.backend, self.structured_format)
    }
}

#[derive(Clone, Copy)]
pub(super) struct SupportedLanguage {
    pub(super) language: CodeLanguage,
    pub(super) extensions: &'static [&'static str],
    pub(super) exact_filenames: &'static [&'static str],
    pub(super) shebang_interpreters: &'static [&'static str],
    pub(super) modelines: &'static [&'static str],
    pub(super) markdown_fences: &'static [&'static str],
}

pub(super) const fn language(
    canonical_id: &'static str,
    display_label: &'static str,
    backend: CodeBackend,
    structured_format: Option<StructuredFormat>,
) -> CodeLanguage {
    CodeLanguage {
        canonical_id,
        display_label,
        backend,
        structured_format,
    }
}

pub(super) const fn supported(
    language: CodeLanguage,
    extensions: &'static [&'static str],
    exact_filenames: &'static [&'static str],
    shebang_interpreters: &'static [&'static str],
    modelines: &'static [&'static str],
    markdown_fences: &'static [&'static str],
) -> SupportedLanguage {
    SupportedLanguage {
        language,
        extensions,
        exact_filenames,
        shebang_interpreters,
        modelines,
        markdown_fences,
    }
}

pub(crate) fn language_for_extension(ext: &str) -> Option<CodeLanguage> {
    language_for_alias(ext, |entry| entry.extensions)
}

pub(crate) fn language_for_exact_name(name: &str) -> Option<CodeLanguage> {
    let normalized = normalize(name);
    if is_env_name(&normalized) {
        return language_for_code_syntax("dotenv");
    }
    LANGUAGES
        .iter()
        .find(|entry| contains(entry.exact_filenames, &normalized))
        .map(|entry| entry.language)
}

pub(crate) fn language_for_shebang(interpreter: &str) -> Option<CodeLanguage> {
    language_for_alias(interpreter, |entry| entry.shebang_interpreters)
}

pub(crate) fn language_for_modeline(token: &str) -> Option<CodeLanguage> {
    language_for_alias(token, |entry| entry.modelines)
}

pub(crate) fn language_for_markdown_fence(token: &str) -> Option<CodeLanguage> {
    language_for_alias(token, |entry| entry.markdown_fences)
}

pub(crate) fn language_for_code_syntax(code_syntax: &str) -> Option<CodeLanguage> {
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
    aliases: impl Fn(&SupportedLanguage) -> &'static [&'static str],
) -> Option<CodeLanguage> {
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

pub(super) const LANGUAGES: &[SupportedLanguage] = &[
    // Structured data and configuration
    supported(
        language(
            "json",
            "JSON",
            CodeBackend::Custom(CustomCodeKind::Json),
            Some(StructuredFormat::Json),
        ),
        &["json"],
        &[
            "package.json",
            "package-lock.json",
            "tsconfig.json",
            "deno.json",
            "composer.lock",
            "pipfile.lock",
            "flake.lock",
        ],
        &[],
        &["json"],
        &["json"],
    ),
    supported(
        language(
            "jsonc",
            "JSONC",
            CodeBackend::Custom(CustomCodeKind::Jsonc),
            Some(StructuredFormat::Jsonc),
        ),
        &["jsonc"],
        &["deno.jsonc"],
        &[],
        &["jsonc"],
        &["jsonc"],
    ),
    supported(
        language(
            "json5",
            "JSON5",
            CodeBackend::Custom(CustomCodeKind::Jsonc),
            Some(StructuredFormat::Json5),
        ),
        &["json5"],
        &[],
        &[],
        &["json5"],
        &["json5"],
    ),
    supported(
        language(
            "toml",
            "TOML",
            CodeBackend::Custom(CustomCodeKind::Toml),
            Some(StructuredFormat::Toml),
        ),
        &["toml"],
        &["cargo.lock", "poetry.lock", "uv.lock"],
        &[],
        &["toml"],
        &["toml"],
    ),
    supported(
        language(
            "yaml",
            "YAML",
            CodeBackend::Custom(CustomCodeKind::Yaml),
            Some(StructuredFormat::Yaml),
        ),
        &["yaml", "yml"],
        &[
            "compose.yml",
            "compose.yaml",
            "docker-compose.yml",
            "docker-compose.yaml",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
        ],
        &[],
        &["yaml", "yml"],
        &["yaml", "yml"],
    ),
    supported(
        language(
            "dotenv",
            ".env",
            CodeBackend::Custom(CustomCodeKind::Ini),
            Some(StructuredFormat::Dotenv),
        ),
        &["env"],
        &[".env"],
        &[],
        &["dotenv"],
        &["dotenv"],
    ),
    supported(
        language(
            "log",
            "Log",
            CodeBackend::Custom(CustomCodeKind::Log),
            Some(StructuredFormat::Log),
        ),
        &["log"],
        &[],
        &[],
        &["log"],
        &["log"],
    ),
    supported(
        language("ini", "INI", CodeBackend::Custom(CustomCodeKind::Ini), None),
        &["ini", "keys", "lock"],
        &["gemfile.lock", "bun.lock"],
        &[],
        &["ini", "dosini"],
        &["ini", "dosini"],
    ),
    supported(
        language(
            "desktop",
            "Desktop Entry",
            CodeBackend::Custom(CustomCodeKind::DesktopEntry),
            None,
        ),
        &["desktop"],
        &[],
        &[],
        &["desktop"],
        &["desktop"],
    ),
    supported(
        language(
            "config",
            "Directive config",
            CodeBackend::Custom(CustomCodeKind::DirectiveConf),
            None,
        ),
        &["conf", "cfg"],
        &[],
        &[],
        &["conf", "cfg", "config"],
        &["conf", "cfg", "config"],
    ),
    supported(
        language(
            "kitty",
            "Kitty",
            CodeBackend::Custom(CustomCodeKind::DirectiveConf),
            None,
        ),
        &[],
        &[],
        &[],
        &["kitty"],
        &["kitty"],
    ),
    supported(
        language(
            "mpv",
            "MPV",
            CodeBackend::Custom(CustomCodeKind::DirectiveConf),
            None,
        ),
        &[],
        &[],
        &[],
        &["mpv"],
        &["mpv"],
    ),
    supported(
        language(
            "btop",
            "btop",
            CodeBackend::Custom(CustomCodeKind::DirectiveConf),
            None,
        ),
        &[],
        &[],
        &[],
        &["btop"],
        &["btop"],
    ),
    // Web and interface languages
    supported(
        language("html", "HTML", CodeBackend::Syntect, None),
        &["html", "htm", "xhtml"],
        &[],
        &[],
        &["html"],
        &["html"],
    ),
    supported(
        language("xml", "XML", CodeBackend::Syntect, None),
        &["xml", "xsd", "xsl", "xslt", "svg"],
        &[],
        &[],
        &["xml", "svg", "markup"],
        &["xml", "xhtml", "svg", "markup"],
    ),
    supported(
        language("css", "CSS", CodeBackend::Syntect, None),
        &["css"],
        &[],
        &[],
        &["css"],
        &["css"],
    ),
    supported(
        language("scss", "SCSS", CodeBackend::Syntect, None),
        &["scss"],
        &[],
        &[],
        &["scss"],
        &["scss"],
    ),
    supported(
        language("sass", "Sass", CodeBackend::Syntect, None),
        &["sass"],
        &[],
        &[],
        &["sass"],
        &["sass"],
    ),
    supported(
        language("less", "Less", CodeBackend::Syntect, None),
        &["less"],
        &[],
        &[],
        &["less"],
        &["less"],
    ),
    supported(
        language("javascript", "JavaScript", CodeBackend::Syntect, None),
        &["js", "mjs", "cjs"],
        &[],
        &[],
        &["javascript"],
        &["js", "javascript"],
    ),
    supported(
        language("jsx", "JSX", CodeBackend::Syntect, None),
        &["jsx"],
        &[],
        &[],
        &["jsx"],
        &["jsx"],
    ),
    supported(
        language("typescript", "TypeScript", CodeBackend::Syntect, None),
        &["ts", "mts", "cts"],
        &[],
        &[],
        &["typescript"],
        &["ts", "typescript"],
    ),
    supported(
        language("tsx", "TSX", CodeBackend::Syntect, None),
        &["tsx"],
        &[],
        &[],
        &["tsx"],
        &["tsx"],
    ),
    supported(
        language("astro", "Astro", CodeBackend::Syntect, None),
        &["astro"],
        &[],
        &[],
        &["astro"],
        &["astro"],
    ),
    supported(
        language("qml", "QML", CodeBackend::Syntect, None),
        &["qml"],
        &[],
        &[],
        &["qml"],
        &["qml"],
    ),
    // Tooling and document languages
    supported(
        language("sql", "SQL", CodeBackend::Syntect, None),
        &["sql"],
        &[],
        &[],
        &["sql"],
        &["sql"],
    ),
    supported(
        language("diff", "Diff", CodeBackend::Syntect, None),
        &["diff", "patch"],
        &[],
        &[],
        &["diff", "patch"],
        &["diff", "patch"],
    ),
    supported(
        language("latex", "LaTeX", CodeBackend::Syntect, None),
        &["tex", "ltx"],
        &[],
        &[],
        &["latex", "tex"],
        &["latex", "tex"],
    ),
    supported(
        language("bibtex", "BibTeX", CodeBackend::Syntect, None),
        &["bib"],
        &[],
        &[],
        &["bibtex", "bib"],
        &["bibtex", "bib"],
    ),
    supported(
        language("tex", "TeX", CodeBackend::Syntect, None),
        &["sty", "cls"],
        &[],
        &[],
        &["tex"],
        &["tex"],
    ),
    supported(
        language("dockerfile", "Dockerfile", CodeBackend::Syntect, None),
        &[],
        &["dockerfile", "containerfile"],
        &[],
        &["dockerfile"],
        &["dockerfile", "docker"],
    ),
    supported(
        language("hcl", "HCL", CodeBackend::Syntect, None),
        &["hcl"],
        &[".terraform.lock.hcl"],
        &[],
        &["hcl"],
        &["hcl"],
    ),
    supported(
        language("terraform", "Terraform", CodeBackend::Syntect, None),
        &["tf", "tfvars", "tfbackend"],
        &["terraform.rc", ".terraformrc"],
        &[],
        &["terraform", "tf", "tfvars"],
        &["terraform", "tf", "tfvars"],
    ),
    supported(
        language("groovy", "Groovy", CodeBackend::Syntect, None),
        &["groovy", "gvy", "gradle"],
        &["build.gradle", "settings.gradle", "init.gradle"],
        &["groovy"],
        &["groovy", "gradle"],
        &["groovy", "gradle"],
    ),
    supported(
        language("scala", "Scala", CodeBackend::Syntect, None),
        &["scala", "sbt"],
        &["build.sbt"],
        &["scala"],
        &["scala", "sbt"],
        &["scala", "sbt"],
    ),
    supported(
        language("just", "Just", CodeBackend::Syntect, None),
        &[],
        &["justfile", ".justfile"],
        &[],
        &["just"],
        &["just"],
    ),
    supported(
        language("make", "Makefile", CodeBackend::Syntect, None),
        &["mk", "mak"],
        &["makefile", "gnumakefile", "bsdmakefile"],
        &[],
        &["make", "makefile"],
        &["make", "makefile"],
    ),
    supported(
        language("nix", "Nix", CodeBackend::Syntect, None),
        &["nix"],
        &[],
        &[],
        &["nix"],
        &["nix"],
    ),
    supported(
        language("cmake", "CMake", CodeBackend::Syntect, None),
        &["cmake"],
        &["cmakelists.txt"],
        &[],
        &["cmake"],
        &["cmake"],
    ),
    // Programming languages
    supported(
        language("perl", "Perl", CodeBackend::Syntect, None),
        &["pl", "pm", "pod", "t"],
        &["cpanfile"],
        &["perl"],
        &["perl", "pl", "pm"],
        &["perl", "pl", "pm"],
    ),
    supported(
        language("haskell", "Haskell", CodeBackend::Syntect, None),
        &["hs", "lhs"],
        &[],
        &["runhaskell"],
        &["haskell", "hs", "lhs"],
        &["haskell", "hs", "lhs"],
    ),
    supported(
        language("julia", "Julia", CodeBackend::Syntect, None),
        &["jl"],
        &[],
        &["julia"],
        &["julia", "jl"],
        &["julia", "jl"],
    ),
    supported(
        language("r", "R", CodeBackend::Syntect, None),
        &["r"],
        &[".rprofile"],
        &["rscript"],
        &["r"],
        &["r", "rscript"],
    ),
    supported(
        language("rust", "Rust", CodeBackend::Syntect, None),
        &["rs"],
        &[],
        &[],
        &["rust", "rs"],
        &["rust", "rs"],
    ),
    supported(
        language("go", "Go", CodeBackend::Syntect, None),
        &["go"],
        &[],
        &[],
        &["go", "golang"],
        &["go", "golang"],
    ),
    supported(
        language("c", "C", CodeBackend::Syntect, None),
        &["c", "h"],
        &[],
        &[],
        &["c", "h"],
        &["c", "h"],
    ),
    supported(
        language("cpp", "C++", CodeBackend::Syntect, None),
        &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
        &[],
        &[],
        &["cpp", "c++", "cc", "cxx", "hpp", "hh", "hxx"],
        &["cpp", "c++", "cc", "cxx", "hpp", "hh", "hxx"],
    ),
    supported(
        language("cs", "C#", CodeBackend::Syntect, None),
        &["cs", "csx"],
        &[],
        &[],
        &["cs", "csharp", "c#"],
        &["cs", "csharp", "c#"],
    ),
    supported(
        language("java", "Java", CodeBackend::Syntect, None),
        &["java"],
        &[],
        &[],
        &["java"],
        &["java"],
    ),
    supported(
        language("dart", "Dart", CodeBackend::Syntect, None),
        &["dart"],
        &[],
        &[],
        &["dart"],
        &["dart"],
    ),
    supported(
        language("zig", "Zig", CodeBackend::Syntect, None),
        &["zig"],
        &[],
        &[],
        &["zig"],
        &["zig"],
    ),
    supported(
        language("php", "PHP", CodeBackend::Syntect, None),
        &["php"],
        &[],
        &[],
        &["php"],
        &["php"],
    ),
    supported(
        language("swift", "Swift", CodeBackend::Syntect, None),
        &["swift"],
        &[],
        &[],
        &["swift"],
        &["swift"],
    ),
    supported(
        language("kotlin", "Kotlin", CodeBackend::Syntect, None),
        &["kt", "kts"],
        &[],
        &[],
        &["kotlin", "kt", "kts"],
        &["kotlin", "kt", "kts"],
    ),
    supported(
        language("elixir", "Elixir", CodeBackend::Syntect, None),
        &["ex", "exs"],
        &[],
        &["elixir"],
        &["elixir", "ex", "exs"],
        &["elixir", "ex", "exs"],
    ),
    supported(
        language("fortran", "Fortran", CodeBackend::Syntect, None),
        &["f", "for", "f90", "f95", "f03", "f08", "fpp"],
        &[],
        &[],
        &["fortran", "f90", "f95", "f03", "f08"],
        &["fortran", "f90", "f95", "f03", "f08"],
    ),
    supported(
        language("cobol", "COBOL", CodeBackend::Syntect, None),
        &["cbl", "cob", "cobol", "cpy"],
        &[],
        &[],
        &["cobol", "cbl", "cob", "cpy"],
        &["cobol", "cbl", "cob", "cpy"],
    ),
    supported(
        language("clojure", "Clojure", CodeBackend::Syntect, None),
        &["clj", "cljs", "cljc", "edn"],
        &["project.clj", "deps.edn", "bb.edn", "shadow-cljs.edn"],
        &["clojure", "clj", "bb"],
        &["clojure", "clj", "cljs", "cljc", "edn"],
        &["clojure", "clj", "cljs", "cljc", "edn"],
    ),
    supported(
        language("ruby", "Ruby", CodeBackend::Syntect, None),
        &["rb"],
        &[],
        &[],
        &["ruby", "rb"],
        &["ruby", "rb"],
    ),
    supported(
        language("python", "Python", CodeBackend::Syntect, None),
        &["py", "pyi", "pyw", "pyx"],
        &[],
        &[],
        &["python", "py"],
        &["python", "py"],
    ),
    supported(
        language("lua", "Lua", CodeBackend::Syntect, None),
        &["lua"],
        &["kyuafile"],
        &[],
        &["lua"],
        &["lua"],
    ),
    // Shell languages
    supported(
        language("sh", "Shell", CodeBackend::Syntect, None),
        &["sh"],
        &[".profile", ".xprofile", ".xsessionrc", ".envrc"],
        &["sh"],
        &["sh", "shell"],
        &["sh", "shell"],
    ),
    supported(
        language("bash", "Bash", CodeBackend::Syntect, None),
        &["bash"],
        &[
            ".bashrc",
            ".bash_profile",
            ".bash_login",
            ".bash_logout",
            ".bash_aliases",
            "pkgbuild",
        ],
        &["bash"],
        &["bash"],
        &["bash"],
    ),
    supported(
        language("zsh", "Zsh", CodeBackend::Syntect, None),
        &["zsh"],
        &[".zshrc", ".zprofile", ".zshenv", ".zlogin", ".zlogout"],
        &["zsh"],
        &["zsh"],
        &["zsh"],
    ),
    supported(
        language("ksh", "KornShell", CodeBackend::Syntect, None),
        &["ksh"],
        &[".kshrc", ".mkshrc"],
        &["ksh"],
        &["ksh"],
        &["ksh"],
    ),
    supported(
        language("fish", "Fish", CodeBackend::Syntect, None),
        &["fish"],
        &[],
        &["fish"],
        &["fish"],
        &["fish"],
    ),
    supported(
        language("powershell", "PowerShell", CodeBackend::Syntect, None),
        &["ps1", "psm1", "psd1"],
        &[],
        &["pwsh", "powershell"],
        &["powershell", "pwsh", "ps1"],
        &["powershell", "pwsh", "ps1"],
    ),
];

#[cfg(test)]
pub(crate) fn syntect_language_ids() -> Vec<&'static str> {
    LANGUAGES
        .iter()
        .filter(|entry| entry.language.backend == CodeBackend::Syntect)
        .map(|entry| entry.language.canonical_id)
        .collect()
}
