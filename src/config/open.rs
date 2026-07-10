use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct OpenConfig {
    pub rules: Vec<OpenRule>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OpenRule {
    pub types: Vec<OpenTargetType>,
    pub exts: Vec<String>,
    pub platforms: Vec<OpenPlatform>,
    pub command: String,
    pub terminal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenTargetType {
    Folder,
    Text,
    Code,
    Config,
    Document,
    Image,
    Audio,
    Video,
    Archive,
    Font,
    Data,
    File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenPlatform {
    Linux,
    Bsd,
    Macos,
    Windows,
}

#[derive(Deserialize, Default)]
pub(super) struct OpenConfigOverride {
    rules: Option<Vec<toml::Value>>,
    #[serde(flatten)]
    unknown: BTreeMap<String, toml::Value>,
}

impl OpenConfig {
    pub(super) fn from_override(overrides: OpenConfigOverride, defaults: &Self) -> Self {
        for key in overrides.unknown.keys() {
            eprintln!("elio: open.{key}: unknown open config key; ignoring");
        }

        let mut resolved = defaults.clone();
        if let Some(rules) = overrides.rules {
            resolved.rules = rules
                .iter()
                .enumerate()
                .filter_map(|(index, value)| {
                    OpenRule::from_toml_value(value, &format!("open.rules[{index}]"))
                })
                .collect();
        }
        resolved
    }
}

impl OpenRule {
    fn from_toml_value(value: &toml::Value, field_name: &str) -> Option<Self> {
        const FIELDS: &[&str] = &["type", "ext", "platform", "command", "terminal"];

        let toml::Value::Table(table) = value else {
            eprintln!("elio: {field_name}: expected an open rule object; skipping rule");
            return None;
        };

        for key in table.keys() {
            if !FIELDS.contains(&key.as_str()) {
                eprintln!(
                    "elio: {field_name}: unknown field {key:?}; expected type, ext, platform, command, terminal; skipping rule"
                );
                return None;
            }
        }

        let types = parse_type_list(table.get("type"), field_name)?;
        let exts = parse_ext_list(table.get("ext"), field_name)?;
        let platforms = parse_platform_list(table.get("platform"), field_name)?;
        let terminal = parse_bool(table.get("terminal"), field_name, "terminal")?;

        if types.is_empty() && exts.is_empty() {
            eprintln!(
                "elio: {field_name}: open rules require at least one type or ext matcher; skipping rule"
            );
            return None;
        }

        let command = table
            .get("command")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|command| !command.is_empty());
        let Some(command) = command else {
            eprintln!(
                "elio: {field_name}: open rules require a non-empty string command; skipping rule"
            );
            return None;
        };

        Some(Self {
            types,
            exts,
            platforms,
            command: command.to_string(),
            terminal,
        })
    }
}

fn parse_type_list(value: Option<&toml::Value>, field_name: &str) -> Option<Vec<OpenTargetType>> {
    parse_string_or_list(value, field_name, "type", OpenTargetType::parse)
}

fn parse_ext_list(value: Option<&toml::Value>, field_name: &str) -> Option<Vec<String>> {
    parse_string_or_list(value, field_name, "ext", |ext| {
        let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        (!ext.is_empty()).then_some(ext)
    })
}

fn parse_platform_list(value: Option<&toml::Value>, field_name: &str) -> Option<Vec<OpenPlatform>> {
    parse_string_or_list(value, field_name, "platform", OpenPlatform::parse)
}

fn parse_string_or_list<T>(
    value: Option<&toml::Value>,
    field_name: &str,
    key: &str,
    mut parse: impl FnMut(&str) -> Option<T>,
) -> Option<Vec<T>> {
    let Some(value) = value else {
        return Some(Vec::new());
    };

    match value {
        toml::Value::String(item) => {
            parse_one(item, field_name, key, &mut parse).map(|item| vec![item])
        }
        toml::Value::Array(items) => {
            let mut parsed = Vec::new();
            for item in items {
                let Some(item) = item.as_str() else {
                    eprintln!("elio: {field_name}: {key} entries must be strings; skipping rule");
                    return None;
                };
                parsed.push(parse_one(item, field_name, key, &mut parse)?);
            }
            Some(parsed)
        }
        _ => {
            eprintln!(
                "elio: {field_name}: {key} must be a string or list of strings; skipping rule"
            );
            None
        }
    }
}

fn parse_one<T>(
    item: &str,
    field_name: &str,
    key: &str,
    parse: &mut impl FnMut(&str) -> Option<T>,
) -> Option<T> {
    let Some(parsed) = parse(item) else {
        eprintln!("elio: {field_name}: unknown {key} value {item:?}; skipping rule");
        return None;
    };
    Some(parsed)
}

fn parse_bool(value: Option<&toml::Value>, field_name: &str, key: &str) -> Option<bool> {
    let Some(value) = value else {
        return Some(false);
    };
    match value {
        toml::Value::Boolean(value) => Some(*value),
        _ => {
            eprintln!("elio: {field_name}: {key} must be true or false; skipping rule");
            None
        }
    }
}

impl OpenTargetType {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "folder" | "directory" | "dir" => Some(Self::Folder),
            "text" => Some(Self::Text),
            "code" => Some(Self::Code),
            "config" => Some(Self::Config),
            "document" | "doc" => Some(Self::Document),
            "image" => Some(Self::Image),
            "audio" => Some(Self::Audio),
            "video" => Some(Self::Video),
            "archive" => Some(Self::Archive),
            "font" => Some(Self::Font),
            "data" => Some(Self::Data),
            "file" => Some(Self::File),
            _ => None,
        }
    }
}

impl OpenPlatform {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "linux" => Some(Self::Linux),
            "bsd" | "freebsd" | "openbsd" | "netbsd" | "dragonfly" => Some(Self::Bsd),
            "macos" | "mac" | "darwin" => Some(Self::Macos),
            "windows" | "win" => Some(Self::Windows),
            _ => None,
        }
    }

    pub(crate) fn current() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(any(
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ))]
        {
            Self::Bsd
        }
        #[cfg(target_os = "macos")]
        {
            Self::Macos
        }
        #[cfg(windows)]
        {
            Self::Windows
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly",
            target_os = "macos",
            windows
        )))]
        {
            Self::Linux
        }
    }
}
