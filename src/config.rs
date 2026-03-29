use serde::Deserialize;
use std::{env, fs, io, path::PathBuf, sync::OnceLock};

static ACTIVE_CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) struct UiConfig {
    pub show_top_bar: bool,
    pub grid_zoom: u8,
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

#[derive(Clone, Copy)]
struct Config {
    ui: UiConfig,
    layout: LayoutConfig,
}

#[derive(Deserialize, Default)]
struct ConfigFile {
    ui: Option<UiConfigOverride>,
    layout: Option<LayoutConfigOverride>,
}

#[derive(Deserialize, Default)]
struct UiConfigOverride {
    show_top_bar: Option<bool>,
    grid_zoom: Option<i64>,
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

pub(crate) fn layout() -> LayoutConfig {
    active_config().layout
}

pub(crate) fn config_dir() -> Option<PathBuf> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home).join("elio"));
    }

    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/elio"))
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
            },
            layout: LayoutConfig { panes: None },
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
        }
        if let Some(layout) = parsed.layout {
            match LayoutConfig::from_override(layout) {
                Ok(layout) => resolved.layout = layout,
                Err(error) => eprintln!("elio: invalid [layout.panes] config: {error}"),
            }
        }
        Ok(resolved)
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
}
