use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

/// Configuration for browser tabs.
///
/// A missing new-tab destination deliberately means "use the active tab's
/// directory", preserving the behaviour from before tab destinations existed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TabsConfig {
    pub(crate) new_tab_destination: Option<NewTabDestination>,
    pub(crate) switch_tab_creates_missing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewTabDestination {
    pub(crate) destination: TabDestination,
    pub(crate) fallback: Option<TabDestination>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TabDestination {
    Path(PathBuf),
    Command(String),
}

#[derive(Deserialize, Default)]
pub(super) struct TabsConfigOverride {
    new_tab_destination: Option<toml::Value>,
    switch_tab_creates_missing: Option<bool>,
    #[serde(flatten)]
    unknown: BTreeMap<String, toml::Value>,
}

impl TabsConfig {
    pub(super) fn from_override(overrides: TabsConfigOverride, defaults: &Self) -> Self {
        for key in overrides.unknown.keys() {
            eprintln!("elio: tabs.{key}: unknown tabs config key; ignoring");
        }

        let mut resolved = defaults.clone();
        if let Some(destination) = overrides.new_tab_destination {
            resolved.new_tab_destination =
                NewTabDestination::from_toml_value(&destination, "tabs.new_tab_destination");
        }
        if let Some(switch_tab_creates_missing) = overrides.switch_tab_creates_missing {
            resolved.switch_tab_creates_missing = switch_tab_creates_missing;
        }
        resolved
    }
}

impl NewTabDestination {
    fn from_toml_value(value: &toml::Value, field_name: &str) -> Option<Self> {
        const FIELDS: &[&str] = &["path", "command", "fallback"];

        let toml::Value::Table(table) = value else {
            eprintln!(
                "elio: {field_name}: expected {{ path, fallback? }} or {{ command, fallback? }}; ignoring"
            );
            return None;
        };
        for key in table.keys() {
            if !FIELDS.contains(&key.as_str()) {
                eprintln!(
                    "elio: {field_name}: unknown field {key:?}; expected path, command, fallback; ignoring"
                );
                return None;
            }
        }

        let destination = TabDestination::from_fields(table, field_name)?;
        let fallback = table.get("fallback").and_then(|fallback| {
            TabDestination::from_toml_value(fallback, &format!("{field_name}.fallback"))
        });
        Some(Self {
            destination,
            fallback,
        })
    }
}

impl TabDestination {
    fn from_toml_value(value: &toml::Value, field_name: &str) -> Option<Self> {
        let toml::Value::Table(table) = value else {
            eprintln!(
                "elio: {field_name}: expected {{ path }} or {{ command }}; ignoring fallback"
            );
            return None;
        };
        for key in table.keys() {
            if !matches!(key.as_str(), "path" | "command") {
                eprintln!(
                    "elio: {field_name}: unknown field {key:?}; expected path or command; ignoring fallback"
                );
                return None;
            }
        }
        Self::from_fields(table, field_name)
    }

    fn from_fields(table: &toml::map::Map<String, toml::Value>, field_name: &str) -> Option<Self> {
        let path = table
            .get("path")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty());
        let command = table
            .get("command")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|command| !command.is_empty());

        if path.is_some() == command.is_some() {
            eprintln!(
                "elio: {field_name}: requires exactly one non-empty string path or command; ignoring"
            );
            return None;
        }

        if let Some(path) = path {
            match crate::config::places::expand_custom_place_path(path) {
                Ok(path) => Some(Self::Path(path)),
                Err(error) => {
                    eprintln!("elio: {field_name}: {error}; ignoring");
                    None
                }
            }
        } else {
            Some(Self::Command(
                command
                    .expect("command is present when path is absent")
                    .to_string(),
            ))
        }
    }
}
