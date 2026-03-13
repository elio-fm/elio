use super::types::shell_file_facts;
use super::{FileFacts, HighlightLanguage, PreviewSpec, StructuredFormat};
use crate::app::FileClass;

pub(super) fn inspect_exact_name(name: &str) -> Option<FileFacts> {
    match name {
        "pkgbuild" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Arch build script"),
            preview: PreviewSpec::source(Some("bash"), Some(HighlightLanguage::Shell), None),
        }),
        "makefile" | "gnumakefile" | "bsdmakefile" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Makefile"),
            preview: PreviewSpec::source(Some("make"), Some(HighlightLanguage::Make), None),
        }),
        "cmakelists.txt" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("CMake project"),
            preview: PreviewSpec::highlighted_source(Some("cmake"), HighlightLanguage::CMake),
        }),
        ".bashrc" | ".bash_profile" | ".bash_login" | ".bash_logout" | ".bash_aliases" => {
            Some(shell_file_facts(FileClass::Config, "Bash config", "bash"))
        }
        ".profile" | ".xprofile" | ".xsessionrc" | ".envrc" => {
            Some(shell_file_facts(FileClass::Config, "Shell config", "sh"))
        }
        ".zshrc" | ".zprofile" | ".zshenv" | ".zlogin" | ".zlogout" => {
            Some(shell_file_facts(FileClass::Config, "Zsh config", "zsh"))
        }
        ".kshrc" | ".mkshrc" => Some(shell_file_facts(
            FileClass::Config,
            "KornShell config",
            "ksh",
        )),
        "cargo.lock" | "poetry.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("toml"),
                Some(HighlightLanguage::Toml),
                Some(StructuredFormat::Toml),
            ),
        }),
        "uv.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: PreviewSpec::source(
                Some("toml"),
                Some(HighlightLanguage::Toml),
                Some(StructuredFormat::Toml),
            ),
        }),
        "package.json" | "tsconfig.json" | "deno.json" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("json"),
                Some(HighlightLanguage::Json),
                Some(StructuredFormat::Json),
            ),
        }),
        "package-lock.json" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("json"),
                Some(HighlightLanguage::Json),
                Some(StructuredFormat::Json),
            ),
        }),
        "composer.lock" | "pipfile.lock" | "flake.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: PreviewSpec::source(
                Some("json"),
                Some(HighlightLanguage::Json),
                Some(StructuredFormat::Json),
            ),
        }),
        "gemfile.lock" | "bun.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: PreviewSpec::source(None, Some(HighlightLanguage::Ini), None),
        }),
        "deno.jsonc" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("JSON with comments"),
            preview: PreviewSpec::source(
                Some("jsonc"),
                Some(HighlightLanguage::Jsonc),
                Some(StructuredFormat::Jsonc),
            ),
        }),
        "compose.yml"
        | "compose.yaml"
        | "docker-compose.yml"
        | "docker-compose.yaml"
        | "pnpm-lock.yaml"
        | "pnpm-workspace.yaml" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("yaml"),
                Some(HighlightLanguage::Yaml),
                Some(StructuredFormat::Yaml),
            ),
        }),
        _ if is_env_name(name) => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Environment file"),
            preview: PreviewSpec::source(
                None,
                Some(HighlightLanguage::Ini),
                Some(StructuredFormat::Dotenv),
            ),
        }),
        _ => None,
    }
}

fn is_env_name(name: &str) -> bool {
    name == ".env" || name.starts_with(".env.")
}
