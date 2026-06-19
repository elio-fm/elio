use super::{super::*, toml_string};

#[test]
fn config_defaults_goto_to_builtin_entries() {
    let config = Config::default_config();

    assert_eq!(
        config.goto.entries,
        vec![
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Top,
                key: 'g',
            },
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Downloads,
                key: 'd',
            },
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Home,
                key: 'h',
            },
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Config,
                key: 'c',
            },
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Trash,
                key: 't',
            },
        ]
    );
}

#[test]
fn config_can_customize_goto_entries() {
    let workspace = std::env::temp_dir().join("elio-goto-workspace");
    let tmp = std::env::temp_dir().join("elio-goto-tmp");
    let workspace_toml = toml_string(&workspace.display().to_string());
    let tmp_toml = toml_string(&tmp.display().to_string());
    let config = Config::from_str(&format!(
        r#"
[goto]
entries = [
  "home",
  {{ builtin = "downloads", key = "W" }},
  {{ title = "projects", path = {}, key = "p" }},
  {{ title = "temp", path = {}, key = "T" }},
  "trash",
]
"#,
        workspace_toml, tmp_toml
    ))
    .expect("config should parse");

    assert_eq!(
        config.goto.entries,
        vec![
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Home,
                key: 'h',
            },
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Downloads,
                key: 'W',
            },
            GotoEntrySpec::Custom {
                title: "projects".to_string(),
                path: workspace,
                key: 'p',
            },
            GotoEntrySpec::Custom {
                title: "temp".to_string(),
                path: tmp,
                key: 'T',
            },
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Trash,
                key: 't',
            },
        ]
    );
}

#[test]
fn config_goto_skips_duplicate_keys() {
    let homelab = std::env::temp_dir().join("elio-goto-homelab");
    let homelab_toml = toml_string(&homelab.display().to_string());
    let config = Config::from_str(&format!(
        r#"
[goto]
entries = [
  "home",
  {{ title = "homelab", path = {}, key = "h" }},
  "trash",
]
"#,
        homelab_toml
    ))
    .expect("config should parse");

    assert_eq!(
        config.goto.entries,
        vec![
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Home,
                key: 'h',
            },
            GotoEntrySpec::Builtin {
                destination: BuiltinGoto::Trash,
                key: 't',
            },
        ]
    );
}

#[test]
fn config_goto_skips_invalid_custom_entries() {
    let config = Config::from_str(
        r#"
[goto]
entries = [
  { title = "relative", path = "workspace", key = "w" },
  { title = "missing key", path = "/tmp" },
  { title = "multi key", path = "/tmp", key = "tmp" },
  "trash",
]
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.goto.entries,
        vec![GotoEntrySpec::Builtin {
            destination: BuiltinGoto::Trash,
            key: 't',
        }]
    );
}
