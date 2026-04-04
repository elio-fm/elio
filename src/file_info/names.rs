use super::{FileFacts, PreviewSpec};
use crate::{core::FileClass, preview::code::registry};

fn preview_for_exact_name(name: &str) -> PreviewSpec {
    registry::language_for_exact_name(name)
        .expect("exact-name registry entry should exist for code preview")
        .preview_spec()
}

pub(super) fn inspect_exact_name(name: &str) -> Option<FileFacts> {
    match name {
        "pkgbuild" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Arch build script"),
            preview: preview_for_exact_name(name),
        }),
        "makefile" | "gnumakefile" | "bsdmakefile" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Makefile"),
            preview: preview_for_exact_name(name),
        }),
        "cmakelists.txt" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("CMake project"),
            preview: preview_for_exact_name(name),
        }),
        "dockerfile" | "containerfile" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Docker build file"),
            preview: preview_for_exact_name(name),
        }),
        "terraform.rc" | ".terraformrc" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Terraform CLI config"),
            preview: preview_for_exact_name(name),
        }),
        ".terraform.lock.hcl" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Terraform lockfile"),
            preview: preview_for_exact_name(name),
        }),
        "build.gradle" | "settings.gradle" | "init.gradle" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Gradle build script"),
            preview: preview_for_exact_name(name),
        }),
        "build.sbt" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("sbt build definition"),
            preview: preview_for_exact_name(name),
        }),
        "project.clj" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Leiningen project"),
            preview: preview_for_exact_name(name),
        }),
        "deps.edn" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Clojure deps config"),
            preview: preview_for_exact_name(name),
        }),
        "bb.edn" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Babashka config"),
            preview: preview_for_exact_name(name),
        }),
        "shadow-cljs.edn" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("shadow-cljs config"),
            preview: preview_for_exact_name(name),
        }),
        "justfile" | ".justfile" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Justfile"),
            preview: preview_for_exact_name(name),
        }),
        ".rprofile" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("R profile"),
            preview: preview_for_exact_name(name),
        }),
        ".bashrc" | ".bash_profile" | ".bash_login" | ".bash_logout" | ".bash_aliases" => {
            Some(FileFacts {
                builtin_class: FileClass::Config,
                specific_type_label: Some("Bash config"),
                preview: preview_for_exact_name(name),
            })
        }
        ".profile" | ".xprofile" | ".xsessionrc" | ".envrc" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Shell config"),
            preview: preview_for_exact_name(name),
        }),
        ".zshrc" | ".zprofile" | ".zshenv" | ".zlogin" | ".zlogout" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Zsh config"),
            preview: preview_for_exact_name(name),
        }),
        ".kshrc" | ".mkshrc" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("KornShell config"),
            preview: preview_for_exact_name(name),
        }),
        "cargo.lock" | "poetry.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: None,
            preview: preview_for_exact_name(name),
        }),
        "uv.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: preview_for_exact_name(name),
        }),
        "package.json" | "tsconfig.json" | "deno.json" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: preview_for_exact_name(name),
        }),
        "package-lock.json" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: None,
            preview: preview_for_exact_name(name),
        }),
        "composer.lock" | "pipfile.lock" | "flake.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: preview_for_exact_name(name),
        }),
        "gemfile.lock" | "bun.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: preview_for_exact_name(name),
        }),
        "deno.jsonc" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("JSON with comments"),
            preview: preview_for_exact_name(name),
        }),
        "compose.yml"
        | "compose.yaml"
        | "docker-compose.yml"
        | "docker-compose.yaml"
        | "pnpm-lock.yaml"
        | "pnpm-workspace.yaml" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: preview_for_exact_name(name),
        }),
        _ if is_env_name(name) => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Environment file"),
            preview: preview_for_exact_name(name),
        }),
        _ => None,
    }
}

fn is_env_name(name: &str) -> bool {
    name == ".env" || name.starts_with(".env.")
}
