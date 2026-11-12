use super::super::{
    Config, TabsConfig,
    tabs::{CloseLastTabBehavior, NewTabDestination, NewTabPosition, TabDestination},
};
use std::path::Path;

fn toml_path(path: &Path) -> String {
    toml::Value::String(path.display().to_string()).to_string()
}

#[test]
fn tabs_default_to_opening_new_tabs_in_the_active_directory() {
    for source in ["", "[tabs]"] {
        assert_eq!(
            Config::from_str(source).unwrap().tabs,
            TabsConfig::default()
        );
    }
}

#[test]
fn tabs_switch_tab_creates_missing_defaults_to_false_and_accepts_true() {
    for source in ["", "[tabs]", "[tabs]\nswitch_tab_creates_missing = false"] {
        assert!(
            !Config::from_str(source)
                .expect("config should parse")
                .tabs
                .switch_tab_creates_missing
        );
    }
    assert!(
        Config::from_str("[tabs]\nswitch_tab_creates_missing = true")
            .expect("config should parse")
            .tabs
            .switch_tab_creates_missing
    );
    for value in ["1", "\"true\"", "[]"] {
        assert!(
            Config::from_str(&format!("[tabs]\nswitch_tab_creates_missing = {value}")).is_err()
        );
    }
}

#[test]
fn tabs_new_tab_position_defaults_to_end_and_accepts_after_current() {
    for source in ["", "[tabs]", "[tabs]\nnew_tab_position = \"end\""] {
        assert_eq!(
            Config::from_str(source)
                .expect("config should parse")
                .tabs
                .new_tab_position,
            NewTabPosition::End
        );
    }
    assert_eq!(
        Config::from_str("[tabs]\nnew_tab_position = \"after_current\"")
            .expect("config should parse")
            .tabs
            .new_tab_position,
        NewTabPosition::AfterCurrent
    );
    for value in ["\"before_current\"", "true", "[]"] {
        assert!(Config::from_str(&format!("[tabs]\nnew_tab_position = {value}")).is_err());
    }
}

#[test]
fn tabs_close_last_tab_defaults_to_keeping_it_open_and_accepts_quit_modes() {
    for source in ["", "[tabs]", "[tabs]\nclose_last_tab = \"keep_open\""] {
        assert_eq!(
            Config::from_str(source)
                .expect("config should parse")
                .tabs
                .close_last_tab,
            CloseLastTabBehavior::KeepOpen
        );
    }
    for (value, expected) in [
        ("quit", CloseLastTabBehavior::Quit),
        ("quit_without_cd", CloseLastTabBehavior::QuitWithoutCd),
    ] {
        assert_eq!(
            Config::from_str(&format!("[tabs]\nclose_last_tab = \"{value}\""))
                .expect("config should parse")
                .tabs
                .close_last_tab,
            expected
        );
    }
    for value in ["\"close\"", "true", "[]"] {
        assert!(Config::from_str(&format!("[tabs]\nclose_last_tab = {value}")).is_err());
    }
}

#[test]
fn tabs_accept_a_path_destination_with_a_command_fallback() {
    let workspace = std::env::temp_dir().join("elio-tabs-workspace");
    let workspace_toml = toml_path(&workspace);
    let config = Config::from_str(&format!(
        r#"
[tabs]
new_tab_destination = {{ path = {workspace_toml}, fallback = {{ command = "printf /tmp" }} }}
"#,
    ))
    .expect("config should parse");

    assert_eq!(
        config.tabs.new_tab_destination,
        Some(NewTabDestination {
            destination: TabDestination::Path(workspace),
            fallback: Some(TabDestination::Command("printf /tmp".to_string())),
        })
    );
}

#[test]
fn tabs_accept_a_command_destination_with_a_path_fallback() {
    let fallback = std::env::temp_dir().join("elio-tabs-fallback");
    let fallback_toml = toml_path(&fallback);
    let config = Config::from_str(&format!(
        r#"
[tabs]
new_tab_destination = {{ command = "git rev-parse --show-toplevel", fallback = {{ path = {fallback_toml} }} }}
"#,
    ))
    .expect("config should parse");

    assert_eq!(
        config.tabs.new_tab_destination,
        Some(NewTabDestination {
            destination: TabDestination::Command("git rev-parse --show-toplevel".to_string()),
            fallback: Some(TabDestination::Path(fallback)),
        })
    );
}

#[test]
fn tabs_ignore_invalid_destinations_but_keep_a_valid_primary_destination() {
    let valid_path = std::env::temp_dir().join("elio-tabs-valid");
    let valid_path_toml = toml_path(&valid_path);
    let invalid = vec![
        "\"/tmp\"".to_string(),
        "{ path = \"relative\" }".to_string(),
        format!("{{ path = {valid_path_toml}, command = \"pwd\" }}"),
        format!("{{ path = {valid_path_toml}, extra = true }}"),
    ];
    for destination in invalid {
        let config = Config::from_str(&format!("[tabs]\nnew_tab_destination = {destination}"))
            .expect("config should parse");
        assert_eq!(config.tabs.new_tab_destination, None, "{destination}");
    }

    let config = Config::from_str(&format!(
        r#"
[tabs]
new_tab_destination = {{ path = {valid_path_toml}, fallback = {{ path = {valid_path_toml}, command = "pwd" }} }}
"#,
    ))
    .expect("config should parse");
    assert_eq!(
        config.tabs.new_tab_destination,
        Some(NewTabDestination {
            destination: TabDestination::Path(valid_path),
            fallback: None,
        })
    );
}
