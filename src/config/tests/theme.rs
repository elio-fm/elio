use super::super::*;

#[test]
fn config_defaults_to_no_theme_selection() {
    let config = Config::default_config();
    assert!(config.theme.is_none());

    let config = Config::from_str("").expect("empty config should parse");
    assert!(config.theme.is_none());
}

#[test]
fn theme_key_parses_top_level_string() {
    let config = Config::from_str(r#"theme = "transparent""#).expect("config should parse");
    assert_eq!(config.theme.as_deref(), Some("transparent"));
}

#[test]
fn theme_key_coexists_with_other_sections() {
    let config = Config::from_str("theme = \"tokyo-night\"\n\n[ui]\nshow_top_bar = true\n")
        .expect("config should parse");
    assert_eq!(config.theme.as_deref(), Some("tokyo-night"));
    assert!(config.ui.show_top_bar);
}
