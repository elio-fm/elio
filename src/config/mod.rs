mod keys;
mod layout;
mod loading;
mod places;
#[cfg(test)]
mod tests;
mod ui;

use serde::Deserialize;

pub(crate) use self::{
    keys::{Action, KeyBindings},
    layout::{LayoutConfig, PaneWeights},
    loading::config_dir,
    places::{BuiltinPlace, PlaceEntrySpec, PlacesConfig},
    ui::UiConfig,
};

struct Config {
    ui: UiConfig,
    places: PlacesConfig,
    layout: LayoutConfig,
    keys: KeyBindings,
}

#[derive(Deserialize, Default)]
struct ConfigFile {
    ui: Option<ui::UiConfigOverride>,
    places: Option<places::PlacesConfigOverride>,
    layout: Option<layout::LayoutConfigOverride>,
    keys: Option<keys::KeysConfigOverride>,
}

pub(crate) fn initialize() {
    loading::initialize();
}

pub(crate) fn ui() -> UiConfig {
    loading::active_config().ui
}

pub(crate) fn places() -> &'static PlacesConfig {
    &loading::active_config().places
}

pub(crate) fn layout() -> LayoutConfig {
    loading::active_config().layout
}

pub(crate) fn keys() -> &'static KeyBindings {
    &loading::active_config().keys
}

impl Config {
    fn default_config() -> Self {
        Self {
            ui: UiConfig::default(),
            places: PlacesConfig::default(),
            layout: LayoutConfig::default(),
            keys: KeyBindings::default(),
        }
    }

    fn from_str(config: &str) -> anyhow::Result<Self> {
        let parsed: ConfigFile = toml::from_str(config)?;
        let mut resolved = Self::default_config();
        if let Some(ui) = parsed.ui {
            resolved.ui.apply_override(ui);
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
        Ok(resolved)
    }
}
