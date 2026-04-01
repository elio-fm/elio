use serde::Deserialize;
use std::{env, fs, io, path::PathBuf, sync::OnceLock};

static ACTIVE_CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) struct UiConfig {
    pub show_top_bar: bool,
    pub grid_zoom: u8,
    pub show_hidden: bool,
    pub start_in_grid: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlacesConfig {
    pub show_devices: bool,
    pub entries: Vec<PlaceEntrySpec>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BuiltinPlace {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Root,
    Trash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlaceEntrySpec {
    Builtin {
        place: BuiltinPlace,
        icon: Option<String>,
    },
    Custom {
        title: String,
        path: PathBuf,
        icon: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LayoutConfig {
    pub panes: Option<PaneWeights>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PaneWeights {
    pub places: u16,
    pub files: u16,
    pub preview: u16,
}

/// A browser action that can be triggered by a configurable key binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Quit,
    Yank,
    Cut,
    Paste,
    Trash,
    Create,
    Rename,
    CopyPath,
    SearchFolders,
    Open,
    Sort,
    ToggleView,
    ToggleHidden,
    ScrollPreviewLeft,
    ScrollPreviewRight,
}

/// Single-character key bindings for browser actions.
/// All fields default to the built-in keys; set any field in `[keys]` in
/// `config.toml` to override that binding.
pub(crate) struct KeyBindings {
    pub quit: char,
    pub yank: char,
    pub cut: char,
    pub paste: char,
    pub trash: char,
    pub create: char,
    pub rename: char,
    pub copy_path: char,
    pub search_folders: char,
    pub open: char,
    pub sort: char,
    pub toggle_view: char,
    pub toggle_hidden: char,
    pub scroll_preview_left: char,
    pub scroll_preview_right: char,
}

/// Characters that are hard-wired to non-configurable actions and may not be
/// used as key binding values.
const RESERVED_CHARS: &[char] = &[
    'h', 'j', 'k', 'l', // navigation (vim keys)
    'g', 'G', // go-to overlay / jump to last
    '?', // help
    '[', ']', // page stepping (epub / comic / pdf)
    '+', '=', '-', '_', // grid zoom
    ' ', // toggle selection
];

impl KeyBindings {
    fn default_bindings() -> Self {
        Self {
            quit: 'q',
            yank: 'y',
            cut: 'x',
            paste: 'p',
            trash: 'd',
            create: 'a',
            rename: 'r',
            copy_path: 'c',
            search_folders: 'f',
            open: 'o',
            sort: 's',
            toggle_view: 'v',
            toggle_hidden: '.',
            scroll_preview_left: '<',
            scroll_preview_right: '>',
        }
    }

    /// Returns the action bound to `c`, if any.
    pub(crate) fn action_for(&self, c: char) -> Option<Action> {
        match c {
            _ if c == self.quit => Some(Action::Quit),
            _ if c == self.yank => Some(Action::Yank),
            _ if c == self.cut => Some(Action::Cut),
            _ if c == self.paste => Some(Action::Paste),
            _ if c == self.trash => Some(Action::Trash),
            _ if c == self.create => Some(Action::Create),
            _ if c == self.rename => Some(Action::Rename),
            _ if c == self.copy_path => Some(Action::CopyPath),
            _ if c == self.search_folders => Some(Action::SearchFolders),
            _ if c == self.open => Some(Action::Open),
            _ if c == self.sort => Some(Action::Sort),
            _ if c == self.toggle_view => Some(Action::ToggleView),
            _ if c == self.toggle_hidden => Some(Action::ToggleHidden),
            _ if c == self.scroll_preview_left => Some(Action::ScrollPreviewLeft),
            _ if c == self.scroll_preview_right => Some(Action::ScrollPreviewRight),
            _ => None,
        }
    }

    /// Parse a full config TOML string and return only the resolved key
    /// bindings.  Falls back to defaults on parse error.
    /// Used by integration tests that need a `KeyBindings` from an override
    /// string without going through the process-wide `OnceLock`.
    #[cfg(test)]
    pub(crate) fn from_toml_str(s: &str) -> Self {
        Config::from_str(s)
            .map(|c| c.keys)
            .unwrap_or_else(|_| Self::default_bindings())
    }

    fn from_override(overrides: KeysConfigOverride, defaults: &Self) -> Self {
        // Each entry: (field_name, user_override_string, default_char)
        let raw: [(&str, Option<String>, char); 15] = [
            ("quit", overrides.quit, defaults.quit),
            ("yank", overrides.yank, defaults.yank),
            ("cut", overrides.cut, defaults.cut),
            ("paste", overrides.paste, defaults.paste),
            ("trash", overrides.trash, defaults.trash),
            ("create", overrides.create, defaults.create),
            ("rename", overrides.rename, defaults.rename),
            ("copy_path", overrides.copy_path, defaults.copy_path),
            (
                "search_folders",
                overrides.search_folders,
                defaults.search_folders,
            ),
            ("open", overrides.open, defaults.open),
            ("sort", overrides.sort, defaults.sort),
            ("toggle_view", overrides.toggle_view, defaults.toggle_view),
            (
                "toggle_hidden",
                overrides.toggle_hidden,
                defaults.toggle_hidden,
            ),
            (
                "scroll_preview_left",
                overrides.scroll_preview_left,
                defaults.scroll_preview_left,
            ),
            (
                "scroll_preview_right",
                overrides.scroll_preview_right,
                defaults.scroll_preview_right,
            ),
        ];

        // Step 1: parse each override string independently, falling back to
        //         default on any format or reserved-char error.
        // (resolved_char, is_user_set)
        let mut candidates: [(char, bool); 15] = [(' ', false); 15];
        for (i, (name, override_str, default)) in raw.iter().enumerate() {
            candidates[i] = match override_str {
                None => (*default, false),
                Some(s) => {
                    let mut chars = s.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) if RESERVED_CHARS.contains(&c) => {
                            eprintln!(
                                "elio: keys.{name}: '{c}' is reserved and cannot be rebound; \
                                 using default '{default}'"
                            );
                            (*default, false)
                        }
                        (Some(c), None) if c.is_control() => {
                            eprintln!(
                                "elio: keys.{name}: control characters cannot be used as key \
                                 bindings; using default '{default}'"
                            );
                            (*default, false)
                        }
                        (Some(c), None) => (c, true),
                        _ => {
                            eprintln!(
                                "elio: keys.{name}: {s:?} is not a single character; \
                                 using default '{default}'"
                            );
                            (*default, false)
                        }
                    }
                }
            };
        }

        // Step 2: reject user-set bindings that collide with any other binding
        //         (user-set or default).  Loop until stable so that reverting one
        //         binding does not silently leave a conflict with another.
        loop {
            let mut changed = false;
            for i in 0..15 {
                if !candidates[i].1 {
                    continue;
                }
                let c = candidates[i].0;
                let collision = (0..15).filter(|&j| j != i).any(|j| candidates[j].0 == c);
                if collision {
                    let (name, _, default) = &raw[i];
                    let other = raw
                        .iter()
                        .enumerate()
                        .filter(|&(j, _)| j != i && candidates[j].0 == c)
                        .map(|(_, (n, _, _))| *n)
                        .next()
                        .unwrap_or("another key");
                    eprintln!(
                        "elio: keys.{name}: '{c}' is already bound to {other}; \
                         using default '{default}'"
                    );
                    candidates[i] = (*default, false);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // Step 3: build from the resolved candidates (order matches `raw`).
        let c = |i: usize| candidates[i].0;
        Self {
            quit: c(0),
            yank: c(1),
            cut: c(2),
            paste: c(3),
            trash: c(4),
            create: c(5),
            rename: c(6),
            copy_path: c(7),
            search_folders: c(8),
            open: c(9),
            sort: c(10),
            toggle_view: c(11),
            toggle_hidden: c(12),
            scroll_preview_left: c(13),
            scroll_preview_right: c(14),
        }
    }
}

struct Config {
    ui: UiConfig,
    places: PlacesConfig,
    layout: LayoutConfig,
    keys: KeyBindings,
}

#[derive(Deserialize, Default)]
struct ConfigFile {
    ui: Option<UiConfigOverride>,
    places: Option<PlacesConfigOverride>,
    layout: Option<LayoutConfigOverride>,
    keys: Option<KeysConfigOverride>,
}

#[derive(Deserialize, Default)]
struct UiConfigOverride {
    show_top_bar: Option<bool>,
    grid_zoom: Option<i64>,
    show_hidden: Option<bool>,
    start_in_grid: Option<bool>,
}

#[derive(Deserialize, Default)]
struct PlacesConfigOverride {
    show_devices: Option<bool>,
    entries: Option<Vec<toml::Value>>,
}

#[derive(Deserialize, Default)]
struct KeysConfigOverride {
    quit: Option<String>,
    yank: Option<String>,
    cut: Option<String>,
    paste: Option<String>,
    trash: Option<String>,
    create: Option<String>,
    rename: Option<String>,
    copy_path: Option<String>,
    search_folders: Option<String>,
    open: Option<String>,
    sort: Option<String>,
    toggle_view: Option<String>,
    toggle_hidden: Option<String>,
    scroll_preview_left: Option<String>,
    scroll_preview_right: Option<String>,
}

#[derive(Deserialize, Default)]
struct LayoutConfigOverride {
    panes: Option<PaneWeightsOverride>,
}

#[derive(Deserialize, Default)]
struct PaneWeightsOverride {
    places: Option<u16>,
    files: Option<u16>,
    preview: Option<u16>,
}

pub(crate) fn initialize() {
    let _ = ACTIVE_CONFIG.get_or_init(load_config_from_disk);
}

pub(crate) fn ui() -> UiConfig {
    active_config().ui
}

pub(crate) fn places() -> &'static PlacesConfig {
    &active_config().places
}

pub(crate) fn layout() -> LayoutConfig {
    active_config().layout
}

pub(crate) fn keys() -> &'static KeyBindings {
    &active_config().keys
}

pub(crate) fn config_dir() -> Option<PathBuf> {
    // XDG_CONFIG_HOME is honoured on all platforms so developers can redirect
    // the config location regardless of OS.
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home).join("elio"));
    }

    // dirs::config_dir() returns:
    //   Linux/BSD : $HOME/.config
    //   macOS     : $HOME/Library/Application Support
    //   Windows   : %APPDATA% (Roaming)
    dirs::config_dir().map(|d| d.join("elio"))
}

fn active_config() -> &'static Config {
    ACTIVE_CONFIG.get_or_init(Config::default_config)
}

fn load_config_from_disk() -> Config {
    let Some(path) = config_path() else {
        return Config::default_config();
    };
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Config::default_config(),
        Err(error) => {
            eprintln!(
                "elio: failed to read config from {}: {error}",
                path.display()
            );
            return Config::default_config();
        }
    };

    match Config::from_str(&contents) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "elio: failed to load config from {}: {error}",
                path.display()
            );
            Config::default_config()
        }
    }
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.toml"))
}

impl Config {
    fn default_config() -> Self {
        Self {
            ui: UiConfig {
                show_top_bar: false,
                grid_zoom: 1,
                show_hidden: false,
                start_in_grid: false,
            },
            places: PlacesConfig::default_places(),
            layout: LayoutConfig { panes: None },
            keys: KeyBindings::default_bindings(),
        }
    }

    fn from_str(config: &str) -> anyhow::Result<Self> {
        let parsed: ConfigFile = toml::from_str(config)?;
        let mut resolved = Self::default_config();
        if let Some(ui) = parsed.ui {
            if let Some(show_top_bar) = ui.show_top_bar {
                resolved.ui.show_top_bar = show_top_bar;
            }
            if let Some(zoom) = ui.grid_zoom {
                resolved.ui.grid_zoom = zoom.clamp(0, 2) as u8;
            }
            if let Some(show_hidden) = ui.show_hidden {
                resolved.ui.show_hidden = show_hidden;
            }
            if let Some(start_in_grid) = ui.start_in_grid {
                resolved.ui.start_in_grid = start_in_grid;
            }
        }
        if let Some(places) = parsed.places {
            resolved.places = PlacesConfig::from_override(places, &resolved.places);
        }
        if let Some(layout) = parsed.layout {
            match LayoutConfig::from_override(layout) {
                Ok(layout) => resolved.layout = layout,
                Err(error) => eprintln!("elio: invalid [layout.panes] config: {error}"),
            }
        }
        if let Some(keys) = parsed.keys {
            resolved.keys = KeyBindings::from_override(keys, &KeyBindings::default_bindings());
        }
        Ok(resolved)
    }
}

impl PlacesConfig {
    fn default_places() -> Self {
        Self {
            show_devices: true,
            entries: vec![
                PlaceEntrySpec::builtin(BuiltinPlace::Home),
                PlaceEntrySpec::builtin(BuiltinPlace::Desktop),
                PlaceEntrySpec::builtin(BuiltinPlace::Documents),
                PlaceEntrySpec::builtin(BuiltinPlace::Downloads),
                PlaceEntrySpec::builtin(BuiltinPlace::Pictures),
                PlaceEntrySpec::builtin(BuiltinPlace::Music),
                PlaceEntrySpec::builtin(BuiltinPlace::Videos),
                PlaceEntrySpec::builtin(BuiltinPlace::Root),
                PlaceEntrySpec::builtin(BuiltinPlace::Trash),
            ],
        }
    }

    fn from_override(overrides: PlacesConfigOverride, defaults: &Self) -> Self {
        let mut resolved = defaults.clone();
        if let Some(show_devices) = overrides.show_devices {
            resolved.show_devices = show_devices;
        }
        if let Some(entries) = overrides.entries {
            resolved.entries = entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    PlaceEntrySpec::from_toml_value(entry, &format!("places.entries[{index}]"))
                })
                .collect();
        }
        resolved
    }
}

impl PlaceEntrySpec {
    fn builtin(place: BuiltinPlace) -> Self {
        Self::Builtin { place, icon: None }
    }

    fn from_toml_value(value: &toml::Value, field_name: &str) -> Option<Self> {
        match value {
            toml::Value::String(name) => BuiltinPlace::parse(name).map(Self::builtin),
            toml::Value::Table(table) => {
                let icon = parse_place_icon(table.get("icon"), field_name);
                if let Some(builtin) = table.get("builtin") {
                    let Some(name) = builtin
                        .as_str()
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                    else {
                        eprintln!(
                            "elio: {field_name}: builtin places require a non-empty string builtin name; \
                             skipping entry"
                        );
                        return None;
                    };
                    let Some(place) = BuiltinPlace::parse(name) else {
                        return None;
                    };
                    if table.contains_key("title") || table.contains_key("path") {
                        eprintln!(
                            "elio: {field_name}: builtin places only support {{ builtin, icon }}; \
                             ignoring extra fields"
                        );
                    }
                    return Some(Self::Builtin { place, icon });
                }

                let title = table
                    .get("title")
                    .and_then(toml::Value::as_str)
                    .map(str::trim)
                    .filter(|title| !title.is_empty());
                let Some(title) = title else {
                    eprintln!(
                        "elio: {field_name}: custom places require a non-empty string title; \
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
                        "elio: {field_name}: custom places require a non-empty string path; \
                         skipping entry"
                    );
                    return None;
                };

                match expand_custom_place_path(path) {
                    Ok(path) => Some(Self::Custom {
                        title: title.to_string(),
                        path,
                        icon,
                    }),
                    Err(error) => {
                        eprintln!("elio: {field_name}: {error}; skipping entry");
                        None
                    }
                }
            }
            _ => {
                eprintln!(
                    "elio: {field_name}: expected a built-in name, {{ builtin, icon? }}, or \
                     {{ title, path, icon? }} object; skipping entry"
                );
                None
            }
        }
    }
}

fn parse_place_icon(value: Option<&toml::Value>, field_name: &str) -> Option<String> {
    let Some(value) = value else {
        return None;
    };
    match value {
        toml::Value::String(icon) => {
            let icon = icon.trim();
            if icon.is_empty() {
                eprintln!("elio: {field_name}: icon must be a non-empty string; using default");
                None
            } else {
                Some(icon.to_string())
            }
        }
        _ => {
            eprintln!("elio: {field_name}: icon must be a string; using default");
            None
        }
    }
}

impl BuiltinPlace {
    fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "home" => Some(Self::Home),
            "desktop" => Some(Self::Desktop),
            "documents" => Some(Self::Documents),
            "downloads" => Some(Self::Downloads),
            "pictures" => Some(Self::Pictures),
            "music" => Some(Self::Music),
            "videos" => Some(Self::Videos),
            "root" => Some(Self::Root),
            "trash" => Some(Self::Trash),
            _ => {
                eprintln!(
                    "elio: unknown places entry {name:?}; expected one of: \
                     home, desktop, documents, downloads, pictures, music, videos, root, trash \
                     (use semantic ids like \"downloads\", not localized folder names)"
                );
                None
            }
        }
    }
}

impl LayoutConfig {
    fn from_override(overrides: LayoutConfigOverride) -> anyhow::Result<Self> {
        let panes = overrides
            .panes
            .map(PaneWeights::from_override)
            .transpose()?;
        Ok(Self { panes })
    }
}

impl PaneWeights {
    fn from_override(overrides: PaneWeightsOverride) -> anyhow::Result<Self> {
        let places = overrides
            .places
            .ok_or_else(|| anyhow::anyhow!("layout.panes.places must be set"))?;
        let files = overrides
            .files
            .ok_or_else(|| anyhow::anyhow!("layout.panes.files must be set"))?;
        let preview = overrides
            .preview
            .ok_or_else(|| anyhow::anyhow!("layout.panes.preview must be set"))?;

        if files == 0 {
            anyhow::bail!("layout.panes.files must be greater than 0");
        }

        Ok(Self {
            places,
            files,
            preview,
        })
    }
}

fn expand_custom_place_path(path: &str) -> anyhow::Result<PathBuf> {
    let expanded = if path == "~" {
        crate::fs::home_dir().ok_or_else(|| anyhow::anyhow!("could not resolve home directory"))?
    } else if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        crate::fs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("could not resolve home directory"))?
            .join(rest)
    } else {
        PathBuf::from(path)
    };

    if !expanded.is_absolute() {
        anyhow::bail!("custom place paths must be absolute or start with ~/");
    }

    Ok(normalize_absolute_path(&expanded))
}

fn normalize_absolute_path(path: &std::path::Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_hide_top_bar() {
        let config = Config::default_config();
        assert!(!config.ui.show_top_bar);
        assert_eq!(config.layout.panes, None);
    }

    #[test]
    fn config_default_hides_hidden_files() {
        let config = Config::default_config();
        assert!(!config.ui.show_hidden);
    }

    #[test]
    fn config_default_starts_in_list_view() {
        let config = Config::default_config();
        assert!(!config.ui.start_in_grid);
    }

    #[test]
    fn config_defaults_places_to_builtin_sidebar_and_devices() {
        let config = Config::default_config();
        assert!(config.places.show_devices);
        assert_eq!(
            config.places.entries,
            vec![
                PlaceEntrySpec::builtin(BuiltinPlace::Home),
                PlaceEntrySpec::builtin(BuiltinPlace::Desktop),
                PlaceEntrySpec::builtin(BuiltinPlace::Documents),
                PlaceEntrySpec::builtin(BuiltinPlace::Downloads),
                PlaceEntrySpec::builtin(BuiltinPlace::Pictures),
                PlaceEntrySpec::builtin(BuiltinPlace::Music),
                PlaceEntrySpec::builtin(BuiltinPlace::Videos),
                PlaceEntrySpec::builtin(BuiltinPlace::Root),
                PlaceEntrySpec::builtin(BuiltinPlace::Trash),
            ]
        );
    }

    #[test]
    fn config_can_enable_show_hidden() {
        let config = Config::from_str("[ui]\nshow_hidden = true").expect("config should parse");
        assert!(config.ui.show_hidden);
    }

    #[test]
    fn config_can_enable_start_in_grid() {
        let config = Config::from_str("[ui]\nstart_in_grid = true").expect("config should parse");
        assert!(config.ui.start_in_grid);
    }

    #[test]
    fn config_can_customize_places_entries_and_hide_devices() {
        let projects = std::env::temp_dir().join("elio-places-projects");
        let config = Config::from_str(&format!(
            r#"
[places]
show_devices = false
entries = [
  "downloads",
  {{ title = "Projects", path = "{}" }},
  "trash",
]
"#,
            projects.display()
        ))
        .expect("config should parse");

        assert!(!config.places.show_devices);
        assert_eq!(
            config.places.entries,
            vec![
                PlaceEntrySpec::builtin(BuiltinPlace::Downloads),
                PlaceEntrySpec::Custom {
                    title: "Projects".to_string(),
                    path: normalize_absolute_path(&projects),
                    icon: None,
                },
                PlaceEntrySpec::builtin(BuiltinPlace::Trash),
            ]
        );
    }

    #[test]
    fn config_places_skips_relative_custom_paths_without_failing_parse() {
        let config = Config::from_str(
            r#"
[places]
entries = [
  { title = "Projects", path = "projects" },
  "downloads",
]
"#,
        )
        .expect("config should parse");

        assert_eq!(
            config.places.entries,
            vec![PlaceEntrySpec::builtin(BuiltinPlace::Downloads)]
        );
    }

    #[test]
    fn config_places_skips_unknown_builtin_names_without_failing_parse() {
        let config = Config::from_str(
            r#"
[places]
entries = ["downloads", "workspace", "trash"]
"#,
        )
        .expect("config should parse");

        assert_eq!(
            config.places.entries,
            vec![
                PlaceEntrySpec::builtin(BuiltinPlace::Downloads),
                PlaceEntrySpec::builtin(BuiltinPlace::Trash),
            ]
        );
    }

    #[test]
    fn config_places_can_customize_icons_for_builtin_and_custom_entries() {
        let projects = std::env::temp_dir().join("elio-places-projects-icons");
        let config = Config::from_str(&format!(
            r#"
[places]
entries = [
  {{ builtin = "downloads", icon = "D" }},
  {{ title = "Projects", path = "{}", icon = "P" }},
]
"#,
            projects.display()
        ))
        .expect("config should parse");

        assert_eq!(
            config.places.entries,
            vec![
                PlaceEntrySpec::Builtin {
                    place: BuiltinPlace::Downloads,
                    icon: Some("D".to_string()),
                },
                PlaceEntrySpec::Custom {
                    title: "Projects".to_string(),
                    path: normalize_absolute_path(&projects),
                    icon: Some("P".to_string()),
                },
            ]
        );
    }

    #[test]
    fn config_places_accepts_builtin_object_form_without_icon() {
        let config = Config::from_str(
            r#"
[places]
entries = [
  { builtin = "downloads" },
  "trash",
]
"#,
        )
        .expect("config should parse");

        assert_eq!(
            config.places.entries,
            vec![
                PlaceEntrySpec::builtin(BuiltinPlace::Downloads),
                PlaceEntrySpec::builtin(BuiltinPlace::Trash),
            ]
        );
    }

    #[test]
    fn config_places_ignores_invalid_icons_without_skipping_entries() {
        let projects = std::env::temp_dir().join("elio-places-invalid-icons");
        let config = Config::from_str(&format!(
            r#"
[places]
entries = [
  {{ builtin = "downloads", icon = "" }},
  {{ title = "Projects", path = "{}", icon = "   " }},
]
"#,
            projects.display()
        ))
        .expect("config should parse");

        assert_eq!(
            config.places.entries,
            vec![
                PlaceEntrySpec::builtin(BuiltinPlace::Downloads),
                PlaceEntrySpec::Custom {
                    title: "Projects".to_string(),
                    path: normalize_absolute_path(&projects),
                    icon: None,
                },
            ]
        );
    }

    #[test]
    fn config_default_grid_zoom_is_1() {
        let config = Config::default_config();
        assert_eq!(config.ui.grid_zoom, 1);
    }

    #[test]
    fn config_can_set_grid_zoom() {
        let config = Config::from_str("[ui]\ngrid_zoom = 0").expect("config should parse");
        assert_eq!(config.ui.grid_zoom, 0);

        let config = Config::from_str("[ui]\ngrid_zoom = 2").expect("config should parse");
        assert_eq!(config.ui.grid_zoom, 2);
    }

    #[test]
    fn config_grid_zoom_clamps_out_of_range() {
        let config = Config::from_str("[ui]\ngrid_zoom = -3").expect("config should parse");
        assert_eq!(config.ui.grid_zoom, 0);

        let config = Config::from_str("[ui]\ngrid_zoom = 4").expect("config should parse");
        assert_eq!(config.ui.grid_zoom, 2);
    }

    #[test]
    fn config_can_enable_top_bar() {
        let config = Config::from_str(
            r#"
[ui]
show_top_bar = true
"#,
        )
        .expect("config should parse");

        assert!(config.ui.show_top_bar);
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

    // --- [keys] tests ---

    #[test]
    fn keys_default_bindings_are_sane() {
        let config = Config::default_config();
        assert_eq!(config.keys.yank, 'y');
        assert_eq!(config.keys.cut, 'x');
        assert_eq!(config.keys.paste, 'p');
        assert_eq!(config.keys.quit, 'q');
    }

    #[test]
    fn keys_can_be_overridden() {
        let config = Config::from_str(
            r#"
[keys]
yank = "Y"
cut = "X"
"#,
        )
        .expect("config should parse");
        assert_eq!(config.keys.yank, 'Y');
        assert_eq!(config.keys.cut, 'X');
        // unset keys stay at default
        assert_eq!(config.keys.paste, 'p');
    }

    #[test]
    fn keys_rejects_multi_char_string_and_uses_default() {
        let config = Config::from_str(
            r#"
[keys]
yank = "yy"
"#,
        )
        .expect("config should parse");
        assert_eq!(config.keys.yank, 'y'); // falls back to default
    }

    #[test]
    fn keys_rejects_empty_string_and_uses_default() {
        let config = Config::from_str(
            r#"
[keys]
yank = ""
"#,
        )
        .expect("config should parse");
        assert_eq!(config.keys.yank, 'y');
    }

    #[test]
    fn keys_rejects_reserved_char_and_uses_default() {
        // 'j' is a reserved navigation key
        let config = Config::from_str(
            r#"
[keys]
yank = "j"
"#,
        )
        .expect("config should parse");
        assert_eq!(config.keys.yank, 'y');
    }

    #[test]
    fn keys_rejects_control_characters_and_uses_default() {
        // \t and \n are dispatched as dedicated KeyCode variants (Tab, Enter),
        // not as KeyCode::Char, so they can never fire the action dispatch path.
        let config = Config::from_str("[keys]\nquit = \"\\t\"").expect("config should parse");
        assert_eq!(config.keys.quit, 'q');

        let config = Config::from_str("[keys]\nquit = \"\\n\"").expect("config should parse");
        assert_eq!(config.keys.quit, 'q');
    }

    #[test]
    fn keys_rejects_user_user_duplicate_and_uses_defaults() {
        // Both yank and paste set to "p" — conflict, both revert to defaults
        let config = Config::from_str(
            r#"
[keys]
yank = "p"
paste = "p"
"#,
        )
        .expect("config should parse");
        assert_eq!(config.keys.yank, 'y');
        assert_eq!(config.keys.paste, 'p');
    }

    #[test]
    fn keys_rejects_user_default_collision_and_uses_default() {
        // yank set to "d" which is trash's default — conflict, yank reverts
        let config = Config::from_str(
            r#"
[keys]
yank = "d"
"#,
        )
        .expect("config should parse");
        assert_eq!(config.keys.yank, 'y');
        assert_eq!(config.keys.trash, 'd');
    }

    #[test]
    fn keys_allows_swapping_two_defaults() {
        // yank = "x", cut = "y" — each takes the other's default, no conflict
        let config = Config::from_str(
            r#"
[keys]
yank = "x"
cut = "y"
"#,
        )
        .expect("config should parse");
        assert_eq!(config.keys.yank, 'x');
        assert_eq!(config.keys.cut, 'y');
    }

    #[test]
    fn action_for_returns_correct_action_for_default_bindings() {
        let kb = KeyBindings::default_bindings();
        assert_eq!(kb.action_for('y'), Some(Action::Yank));
        assert_eq!(kb.action_for('x'), Some(Action::Cut));
        assert_eq!(kb.action_for('p'), Some(Action::Paste));
        assert_eq!(kb.action_for('q'), Some(Action::Quit));
        assert_eq!(kb.action_for('j'), None); // reserved, never bindable
        assert_eq!(kb.action_for('z'), None); // unbound
    }

    #[test]
    fn action_for_reflects_overridden_binding() {
        let config = Config::from_str(
            r#"
[keys]
yank = "Y"
"#,
        )
        .expect("config should parse");
        assert_eq!(config.keys.action_for('Y'), Some(Action::Yank));
        assert_eq!(config.keys.action_for('y'), None);
    }
}
