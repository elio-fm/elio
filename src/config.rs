use serde::Deserialize;
use std::{env, fs, io, path::PathBuf, sync::OnceLock};

static ACTIVE_CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) struct UiConfig {
    pub show_top_bar: bool,
}

#[derive(Clone, Copy)]
struct Config {
    ui: UiConfig,
}

#[derive(Deserialize, Default)]
struct ConfigFile {
    ui: Option<UiConfigOverride>,
}

#[derive(Deserialize, Default)]
struct UiConfigOverride {
    show_top_bar: Option<bool>,
}

pub(crate) fn initialize() {
    let _ = ACTIVE_CONFIG.get_or_init(load_config_from_disk);
}

pub(crate) fn ui() -> UiConfig {
    active_config().ui
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
            },
        }
    }

    fn from_str(config: &str) -> anyhow::Result<Self> {
        let parsed: ConfigFile = toml::from_str(config)?;
        let mut resolved = Self::default_config();
        if let Some(ui) = parsed.ui
            && let Some(show_top_bar) = ui.show_top_bar
        {
            resolved.ui.show_top_bar = show_top_bar;
        }
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_hide_top_bar() {
        let config = Config::default_config();
        assert!(!config.ui.show_top_bar);
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
}
