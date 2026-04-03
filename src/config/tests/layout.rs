use super::super::*;

#[test]
fn config_defaults_to_legacy_layout() {
    let config = Config::default_config();
    assert_eq!(config.layout.panes, None);
}

#[test]
fn config_can_parse_layout_panes() {
    let config = Config::from_str(
        r#"
[layout.panes]
places = 10
files = 45
preview = 45
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.layout.panes,
        Some(PaneWeights {
            places: 10,
            files: 45,
            preview: 45,
        })
    );
}

#[test]
fn partial_layout_panes_leave_default_layout_active() {
    let config = Config::from_str(
        r#"
[layout.panes]
places = 10
files = 45
"#,
    )
    .expect("config should parse");

    assert_eq!(config.layout.panes, None);
}

#[test]
fn invalid_layout_panes_preserve_other_valid_config_values() {
    let config = Config::from_str(
        r#"
[ui]
show_top_bar = true

[layout.panes]
places = 10
files = 0
preview = 90
"#,
    )
    .expect("config should parse");

    assert!(config.ui.show_top_bar);
    assert_eq!(config.layout.panes, None);
}
