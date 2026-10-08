mod goto;
mod key_bindings;
mod layout;
mod loading;
mod open;
mod places;
mod portal;
mod preview;
#[cfg(test)]
mod tests;
mod ui;

use std::path::Path;

pub(crate) use self::{
    goto::{BuiltinGoto, GotoConfig, GotoEntrySpec},
    key_bindings::{
        Action, ChooserKeyAction, KeyBindings, KeyContext, KeyList, normalize_caps_lock_character,
        normalized_plain_key_char,
    },
    layout::{LayoutConfig, PaneWeights},
    loading::{config_dir, config_path},
    open::{OpenConfig, OpenPlatform, OpenRule, OpenTargetType},
    places::{BuiltinPlace, PlaceEntrySpec, PlacesConfig},
    portal::{ConfigurePortalTerminal, PortalConfig, ensure_portal_terminal},
    preview::PreviewConfig,
    ui::UiConfig,
};

#[cfg(test)]
use self::loading::Config;

pub(crate) fn initialize(path: Option<&Path>) -> anyhow::Result<()> {
    loading::initialize(path)
}

pub(crate) fn ui() -> UiConfig {
    loading::active_config().ui
}

pub(crate) fn preview() -> PreviewConfig {
    loading::active_config().preview
}

pub(crate) fn goto() -> &'static GotoConfig {
    &loading::active_config().goto
}

pub(crate) fn places() -> &'static PlacesConfig {
    &loading::active_config().places
}

pub(crate) fn layout() -> LayoutConfig {
    loading::active_config().layout
}

pub(crate) fn key_bindings() -> &'static KeyBindings {
    &loading::active_config().keys
}

pub(crate) fn open() -> &'static OpenConfig {
    &loading::active_config().open
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) fn portal_terminal() -> Option<crate::portal::terminal::TerminalAdapter> {
    loading::active_config().portal.terminal
}
