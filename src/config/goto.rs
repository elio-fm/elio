use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GotoConfig {
    pub entries: Vec<GotoEntrySpec>,
}

impl Default for GotoConfig {
    fn default() -> Self {
        Self {
            entries: vec![
                GotoEntrySpec::builtin(BuiltinGoto::Top),
                GotoEntrySpec::builtin(BuiltinGoto::Downloads),
                GotoEntrySpec::builtin(BuiltinGoto::Home),
                GotoEntrySpec::builtin(BuiltinGoto::Config),
                GotoEntrySpec::builtin(BuiltinGoto::Trash),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BuiltinGoto {
    Top,
    Downloads,
    Home,
    Config,
    Trash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GotoEntrySpec {
    Builtin {
        destination: BuiltinGoto,
        key: char,
    },
    Custom {
        title: String,
        path: PathBuf,
        key: char,
    },
}

#[derive(Deserialize, Default)]
pub(super) struct GotoConfigOverride {
    entries: Option<Vec<toml::Value>>,
}

impl GotoConfig {
    pub(super) fn from_override(overrides: GotoConfigOverride, defaults: &Self) -> Self {
        let Some(entries) = overrides.entries else {
            return defaults.clone();
        };

        let mut owners = HashMap::new();
        let entries = entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let field_name = format!("goto.entries[{index}]");
                let entry = GotoEntrySpec::from_toml_value(entry, &field_name)?;
                let key = entry.key();
                if let Some(owner) = owners.get(&key) {
                    eprintln!(
                        "elio: {field_name}: key '{}' is already used by {owner}; skipping entry",
                        entry.key()
                    );
                    return None;
                }
                owners.insert(key, entry.name().to_string());
                Some(entry)
            })
            .collect();

        Self { entries }
    }
}

impl GotoEntrySpec {
    fn builtin(destination: BuiltinGoto) -> Self {
        Self::Builtin {
            destination,
            key: destination.default_key(),
        }
    }

    fn from_toml_value(value: &toml::Value, field_name: &str) -> Option<Self> {
        match value {
            toml::Value::String(name) => BuiltinGoto::parse(name).map(Self::builtin),
            toml::Value::Table(table) => {
                if let Some(builtin) = table.get("builtin") {
                    let Some(name) = builtin
                        .as_str()
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                    else {
                        eprintln!(
                            "elio: {field_name}: builtin goto entries require a non-empty string builtin name; \
                             skipping entry"
                        );
                        return None;
                    };
                    let destination = BuiltinGoto::parse(name)?;
                    if table.contains_key("title") || table.contains_key("path") {
                        eprintln!(
                            "elio: {field_name}: builtin goto entries only support {{ builtin, key }}; \
                             ignoring extra fields"
                        );
                    }
                    let key = parse_goto_key(table.get("key"), field_name)
                        .unwrap_or_else(|| destination.default_key());
                    return Some(Self::Builtin { destination, key });
                }

                let title = table
                    .get("title")
                    .and_then(toml::Value::as_str)
                    .map(str::trim)
                    .filter(|title| !title.is_empty());
                let Some(title) = title else {
                    eprintln!(
                        "elio: {field_name}: custom goto entries require a non-empty string title; \
                         skipping entry"
                    );
                    return None;
                };

                let path = table
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .map(str::trim)
                    .filter(|path| !path.is_empty());
                let Some(path) = path else {
                    eprintln!(
                        "elio: {field_name}: custom goto entries require a non-empty string path; \
                         skipping entry"
                    );
                    return None;
                };

                let Some(key) = parse_goto_key(table.get("key"), field_name) else {
                    eprintln!(
                        "elio: {field_name}: custom goto entries require a single-character string key; \
                         skipping entry"
                    );
                    return None;
                };

                match crate::config::places::expand_custom_place_path(path) {
                    Ok(path) => Some(Self::Custom {
                        title: title.to_string(),
                        path,
                        key,
                    }),
                    Err(error) => {
                        eprintln!("elio: {field_name}: {error}; skipping entry");
                        None
                    }
                }
            }
            _ => {
                eprintln!(
                    "elio: {field_name}: expected a built-in name, {{ builtin, key? }}, or \
                     {{ title, path, key }} object; skipping entry"
                );
                None
            }
        }
    }

    pub(crate) fn key(&self) -> char {
        match self {
            Self::Builtin { key, .. } | Self::Custom { key, .. } => *key,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Builtin { destination, .. } => destination.name(),
            Self::Custom { title, .. } => title,
        }
    }
}

fn parse_goto_key(value: Option<&toml::Value>, field_name: &str) -> Option<char> {
    let value = value?;
    let Some(key) = value.as_str().map(str::trim) else {
        eprintln!("elio: {field_name}: goto key must be a string");
        return None;
    };
    let mut chars = key.chars();
    let Some(ch) = chars.next() else {
        eprintln!("elio: {field_name}: goto key must not be empty");
        return None;
    };
    if chars.next().is_some() {
        eprintln!("elio: {field_name}: goto key must be a single character");
        return None;
    }
    Some(ch)
}

impl BuiltinGoto {
    fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "top" => Some(Self::Top),
            "downloads" => Some(Self::Downloads),
            "home" => Some(Self::Home),
            "config" => Some(Self::Config),
            "trash" => Some(Self::Trash),
            _ => {
                eprintln!(
                    "elio: unknown goto entry {name:?}; expected one of: \
                     top, downloads, home, config, trash"
                );
                None
            }
        }
    }

    fn default_key(self) -> char {
        match self {
            Self::Top => 'g',
            Self::Downloads => 'd',
            Self::Home => 'h',
            Self::Config => 'c',
            Self::Trash => 't',
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Downloads => "downloads",
            Self::Home => "home",
            Self::Config => "config",
            Self::Trash => "trash",
        }
    }
}
