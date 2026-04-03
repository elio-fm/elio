use super::super::types::RuleOverride;
use super::shared::{normalize_key, rgb};
use crate::core::FileClass;
use std::collections::HashMap;

pub(super) fn default_file_rules() -> HashMap<String, RuleOverride> {
    HashMap::from([
        (
            normalize_key("Cargo.lock"),
            RuleOverride {
                class: Some(FileClass::Data),
                icon: Some("󰈡".to_string()),
                color: None,
            },
        ),
        (
            normalize_key("package.json"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(226, 180, 80)),
            },
        ),
        (
            normalize_key("package-lock.json"),
            RuleOverride {
                class: Some(FileClass::Data),
                icon: Some("󰈡".to_string()),
                color: Some(rgb(210, 146, 89)),
            },
        ),
        (
            normalize_key("pnpm-lock.yaml"),
            RuleOverride {
                class: Some(FileClass::Data),
                icon: Some("󰈡".to_string()),
                color: Some(rgb(255, 184, 107)),
            },
        ),
        (
            normalize_key("yarn.lock"),
            RuleOverride {
                class: Some(FileClass::Data),
                icon: Some("󰈡".to_string()),
                color: Some(rgb(36, 217, 184)),
            },
        ),
        (
            normalize_key("bun.lock"),
            RuleOverride {
                class: Some(FileClass::Data),
                icon: Some("󰈡".to_string()),
                color: Some(rgb(247, 200, 94)),
            },
        ),
        (
            normalize_key("bun.lockb"),
            RuleOverride {
                class: Some(FileClass::Data),
                icon: Some("󰈡".to_string()),
                color: Some(rgb(247, 200, 94)),
            },
        ),
        (
            normalize_key("poetry.lock"),
            RuleOverride {
                class: Some(FileClass::Data),
                icon: Some("󰈡".to_string()),
                color: Some(rgb(141, 223, 109)),
            },
        ),
        (
            normalize_key("Pipfile.lock"),
            RuleOverride {
                class: Some(FileClass::Data),
                icon: Some("󰈡".to_string()),
                color: Some(rgb(89, 222, 148)),
            },
        ),
        (
            normalize_key("uv.lock"),
            RuleOverride {
                class: Some(FileClass::Data),
                icon: Some("󰈡".to_string()),
                color: Some(rgb(89, 222, 148)),
            },
        ),
        (
            normalize_key("Dockerfile"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("󰡨".to_string()),
                color: Some(rgb(94, 162, 227)),
            },
        ),
        (
            normalize_key("Containerfile"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("󰡨".to_string()),
                color: Some(rgb(94, 162, 227)),
            },
        ),
        (
            normalize_key("compose.yml"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("󰡨".to_string()),
                color: Some(rgb(94, 162, 227)),
            },
        ),
        (
            normalize_key("compose.yaml"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("󰡨".to_string()),
                color: Some(rgb(94, 162, 227)),
            },
        ),
        (
            normalize_key(".terraform.lock.hcl"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(179, 140, 255)),
            },
        ),
        (
            normalize_key("build.gradle"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(112, 182, 117)),
            },
        ),
        (
            normalize_key("settings.gradle"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(112, 182, 117)),
            },
        ),
        (
            normalize_key("init.gradle"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(112, 182, 117)),
            },
        ),
        (
            normalize_key("build.sbt"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(232, 90, 90)),
            },
        ),
        (
            normalize_key(".rprofile"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("󰟔".to_string()),
                color: Some(rgb(95, 153, 219)),
            },
        ),
        (
            normalize_key("project.clj"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(128, 176, 92)),
            },
        ),
        (
            normalize_key("deps.edn"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(128, 176, 92)),
            },
        ),
        (
            normalize_key("bb.edn"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(128, 176, 92)),
            },
        ),
        (
            normalize_key("shadow-cljs.edn"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(128, 176, 92)),
            },
        ),
        (
            normalize_key("Justfile"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(255, 184, 107)),
            },
        ),
        (
            normalize_key(".justfile"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(255, 184, 107)),
            },
        ),
        (
            normalize_key("build.zig.zon"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(245, 173, 64)),
            },
        ),
        (
            normalize_key("README.md"),
            RuleOverride {
                class: Some(FileClass::Document),
                icon: Some("".to_string()),
                color: Some(rgb(211, 170, 124)),
            },
        ),
        (
            normalize_key("AUTHORS"),
            RuleOverride {
                class: Some(FileClass::Document),
                icon: Some("󰭘".to_string()),
                color: Some(rgb(155, 143, 199)),
            },
        ),
        (
            normalize_key("AUTHORS.md"),
            RuleOverride {
                class: Some(FileClass::Document),
                icon: Some("󰭘".to_string()),
                color: Some(rgb(155, 143, 199)),
            },
        ),
        (
            normalize_key("AUTHORS.txt"),
            RuleOverride {
                class: Some(FileClass::Document),
                icon: Some("󰭘".to_string()),
                color: Some(rgb(155, 143, 199)),
            },
        ),
        (
            normalize_key("CONTRIBUTORS"),
            RuleOverride {
                class: Some(FileClass::Document),
                icon: Some("󰭘".to_string()),
                color: Some(rgb(155, 143, 199)),
            },
        ),
        (
            normalize_key("CONTRIBUTORS.md"),
            RuleOverride {
                class: Some(FileClass::Document),
                icon: Some("󰭘".to_string()),
                color: Some(rgb(155, 143, 199)),
            },
        ),
        (
            normalize_key(".gitignore"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("󰊢".to_string()),
                color: Some(rgb(232, 153, 88)),
            },
        ),
        (
            normalize_key(".env"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("󰒓".to_string()),
                color: Some(rgb(144, 192, 121)),
            },
        ),
        (
            normalize_key("PKGBUILD"),
            RuleOverride {
                class: Some(FileClass::Config),
                icon: Some("".to_string()),
                color: Some(rgb(102, 187, 255)),
            },
        ),
    ])
}
