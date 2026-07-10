use super::super::*;

#[test]
fn config_defaults_open_rules_to_empty() {
    let config = Config::default_config();
    assert!(config.open.rules.is_empty());
}

#[test]
fn config_parses_open_rules_with_string_and_list_fields() {
    let config = Config::from_str(
        r#"
[open]
rules = [
  { type = ["text", "code"], ext = "md", command = "$EDITOR", terminal = true },
  { ext = ["pdf", ".epub"], command = "open -a Preview", platform = "macos" },
  { type = "image", command = "imv", platform = ["linux", "bsd"] },
]
"#,
    )
    .expect("config should parse");

    assert_eq!(config.open.rules.len(), 3);
    assert_eq!(
        config.open.rules[0].types,
        vec![OpenTargetType::Text, OpenTargetType::Code]
    );
    assert_eq!(config.open.rules[0].exts, vec!["md"]);
    assert_eq!(config.open.rules[0].command, "$EDITOR");
    assert!(config.open.rules[0].terminal);

    assert_eq!(config.open.rules[1].exts, vec!["pdf", "epub"]);
    assert_eq!(config.open.rules[1].platforms, vec![OpenPlatform::Macos]);

    assert_eq!(config.open.rules[2].types, vec![OpenTargetType::Image]);
    assert_eq!(
        config.open.rules[2].platforms,
        vec![OpenPlatform::Linux, OpenPlatform::Bsd]
    );
}

#[test]
fn config_skips_invalid_open_rules_without_failing_parse() {
    let config = Config::from_str(
        r#"
[open]
rules = [
  { type = "unknown", command = "bad" },
  { ext = "", command = "bad" },
  { ext = "md" },
  { command = "missing matcher" },
  { ext = "txt", command = "hx" },
]
"#,
    )
    .expect("config should parse");

    assert_eq!(config.open.rules.len(), 1);
    assert_eq!(config.open.rules[0].exts, vec!["txt"]);
    assert_eq!(config.open.rules[0].command, "hx");
}
