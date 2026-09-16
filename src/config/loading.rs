use super::{
    GotoConfig, KeyBindings, LayoutConfig, OpenConfig, PlacesConfig, PreviewConfig, UiConfig,
};
#[cfg(unix)]
use crate::elevated_session::InvocationContext;
use serde::Deserialize;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

static ACTIVE_CONFIG: OnceLock<Config> = OnceLock::new();

pub(super) struct Config {
    pub(super) ui: UiConfig,
    pub(super) preview: PreviewConfig,
    pub(super) goto: GotoConfig,
    pub(super) places: PlacesConfig,
    pub(super) layout: LayoutConfig,
    pub(super) keys: KeyBindings,
    pub(super) open: OpenConfig,
}

#[derive(Deserialize, Default)]
struct ConfigFile {
    ui: Option<super::ui::UiConfigOverride>,
    preview: Option<super::preview::PreviewConfigOverride>,
    goto: Option<super::goto::GotoConfigOverride>,
    places: Option<super::places::PlacesConfigOverride>,
    layout: Option<super::layout::LayoutConfigOverride>,
    keys: Option<super::key_bindings::KeysConfigOverride>,
    open: Option<super::open::OpenConfigOverride>,
}

pub(super) fn initialize(path: Option<&Path>) -> anyhow::Result<()> {
    #[cfg(unix)]
    // Snapshot the invocation context before loading config or starting workers.
    let _ = crate::elevated_session::context();

    if ACTIVE_CONFIG.get().is_none() {
        let config = load_config_from_disk(path)?;
        let _ = ACTIVE_CONFIG.set(config);
    }
    Ok(())
}

pub(super) fn active_config() -> &'static Config {
    ACTIVE_CONFIG.get_or_init(Config::default_config)
}

fn config_home() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        let process_xdg_home = env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        let process_home = dirs::home_dir();
        config_home_for_context(
            crate::elevated_session::context(),
            process_xdg_home.as_deref(),
            process_home.as_deref(),
        )
    }

    #[cfg(windows)]
    {
        // XDG_CONFIG_HOME is honoured on Windows so developers can redirect
        // the config location regardless of OS.
        if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(config_home));
        }
        dirs::config_dir()
    }
}

pub(crate) fn config_dir() -> Option<PathBuf> {
    config_home().map(|home| home.join("elio"))
}

#[cfg(unix)]
fn config_home_for_context(
    context: &InvocationContext,
    process_xdg_home: Option<&Path>,
    process_home: Option<&Path>,
) -> Option<PathBuf> {
    match context {
        InvocationContext::Normal | InvocationContext::RootSession => {
            platform_config_home(process_xdg_home, process_home)
        }
        InvocationContext::Elevated(user) => {
            platform_config_home(user.xdg_config_home.as_deref(), Some(&user.home))
        }
        InvocationContext::ElevatedUnresolved => None,
    }
}

#[cfg(unix)]
fn platform_config_home(xdg_home: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(xdg_home) = xdg_home {
        return Some(xdg_home.to_path_buf());
    }
    let home = home?;

    #[cfg(target_os = "macos")]
    {
        // Prefer XDG-style config on macOS only when it contains Elio config
        // files, avoiding empty ~/.config/elio directories shadowing the native
        // Application Support location.
        let xdg_home = home.join(".config");
        let xdg_dir = xdg_home.join("elio");
        if xdg_dir.join("config.toml").is_file() || xdg_dir.join("theme.toml").is_file() {
            return Some(xdg_home);
        }
        return Some(home.join("Library/Application Support"));
    }

    #[cfg(not(target_os = "macos"))]
    Some(home.join(".config"))
}

fn load_config_from_disk(override_path: Option<&Path>) -> anyhow::Result<Config> {
    let is_override = override_path.is_some();
    let Some(path) = override_path.map(Path::to_path_buf).or_else(config_path) else {
        return Ok(Config::default_config());
    };
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if !is_override && error.kind() == io::ErrorKind::NotFound => {
            return Ok(Config::default_config());
        }
        Err(error) if is_override => {
            anyhow::bail!(
                "elio: failed to read config from {}: {error}",
                path.display()
            );
        }
        Err(error) => {
            eprintln!(
                "elio: failed to read config from {}: {error}",
                path.display()
            );
            return Ok(Config::default_config());
        }
    };

    Ok(match Config::from_str(&contents) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "elio: failed to load config from {}: {error}",
                path.display()
            );
            Config::default_config()
        }
    })
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.toml"))
}

impl Config {
    pub(super) fn default_config() -> Self {
        Self {
            ui: UiConfig::default(),
            preview: PreviewConfig::default(),
            goto: GotoConfig::default(),
            places: PlacesConfig::default(),
            layout: LayoutConfig::default(),
            keys: KeyBindings::default(),
            open: OpenConfig::default(),
        }
    }

    pub(super) fn from_str(config: &str) -> anyhow::Result<Self> {
        let parsed: ConfigFile = toml::from_str(config)?;
        let mut resolved = Self::default_config();
        if let Some(ui) = parsed.ui {
            resolved.ui.apply_override(ui);
        }
        if let Some(preview) = parsed.preview {
            resolved.preview.apply_override(preview);
        }
        if let Some(goto) = parsed.goto {
            resolved.goto = GotoConfig::from_override(goto, &resolved.goto);
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
            resolved.keys = KeyBindings::from_override(keys, &KeyBindings::default());
        }
        if let Some(open) = parsed.open {
            resolved.open = OpenConfig::from_override(open, &resolved.open);
        }
        Ok(resolved)
    }
}

#[cfg(test)]
#[path = "tests/loading.rs"]
mod tests;
