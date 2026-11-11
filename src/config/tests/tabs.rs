use super::super::{
    Config, TabsConfig,
    tabs::{NewTabDestination, TabDestination},
};
use std::path::PathBuf;

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
fn tabs_accept_a_path_destination_with_a_command_fallback() {
    let config = Config::from_str(
        r#"
[tabs]
new_tab_destination = { path = "/workspace", fallback = { command = "printf /tmp" } }
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.tabs.new_tab_destination,
        Some(NewTabDestination {
            destination: TabDestination::Path(PathBuf::from("/workspace")),
            fallback: Some(TabDestination::Command("printf /tmp".to_string())),
        })
    );
}

#[test]
fn tabs_accept_a_command_destination_with_a_path_fallback() {
    let config = Config::from_str(
        r#"
[tabs]
new_tab_destination = { command = "git rev-parse --show-toplevel", fallback = { path = "/tmp" } }
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.tabs.new_tab_destination,
        Some(NewTabDestination {
            destination: TabDestination::Command("git rev-parse --show-toplevel".to_string()),
            fallback: Some(TabDestination::Path(PathBuf::from("/tmp"))),
        })
    );
}

#[test]
fn tabs_ignore_invalid_destinations_but_keep_a_valid_primary_destination() {
    let invalid = [
        "\"/tmp\"",
        "{ path = \"relative\" }",
        "{ path = \"/tmp\", command = \"pwd\" }",
        "{ path = \"/tmp\", extra = true }",
    ];
    for destination in invalid {
        let config = Config::from_str(&format!("[tabs]\nnew_tab_destination = {destination}"))
            .expect("config should parse");
        assert_eq!(config.tabs.new_tab_destination, None, "{destination}");
    }

    let config = Config::from_str(
        r#"
[tabs]
new_tab_destination = { path = "/tmp", fallback = { path = "/tmp", command = "pwd" } }
"#,
    )
    .expect("config should parse");
    assert_eq!(
        config.tabs.new_tab_destination,
        Some(NewTabDestination {
            destination: TabDestination::Path(PathBuf::from("/tmp")),
            fallback: None,
        })
    );
}
