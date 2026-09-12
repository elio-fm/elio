use crate::{
    config::{BuiltinGoto, GotoEntrySpec},
    places::{PlaceKind, PlaceRow, trash_dir},
};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GotoDestination {
    Top,
    Path(PathBuf),
    Missing(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GotoMenuEntry {
    shortcut: char,
    label: String,
    destination: GotoDestination,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GotoMenu {
    title: String,
    entries: Vec<GotoMenuEntry>,
}

impl GotoMenuEntry {
    pub(crate) fn new(
        shortcut: char,
        label: impl Into<String>,
        destination: GotoDestination,
    ) -> Self {
        Self {
            shortcut,
            label: label.into(),
            destination,
        }
    }

    pub(crate) fn shortcut(&self) -> char {
        self.shortcut
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }
}

impl GotoMenu {
    pub(crate) fn new(entries: Vec<GotoMenuEntry>) -> Self {
        Self {
            title: "Go to".to_string(),
            entries,
        }
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn entry(&self, index: usize) -> Option<&GotoMenuEntry> {
        self.entries.get(index)
    }

    pub(crate) fn index_for_shortcut(&self, shortcut: char) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.shortcut == shortcut)
    }

    pub(crate) fn destination(&self, index: usize) -> Option<GotoDestination> {
        self.entries
            .get(index)
            .map(|entry| entry.destination.clone())
    }
}

pub(crate) fn build_goto_menu(
    configured_entries: &[GotoEntrySpec],
    place_rows: &[PlaceRow],
) -> GotoMenu {
    let entries = configured_entries
        .iter()
        .map(|entry| build_configured_entry(entry, place_rows))
        .collect();
    GotoMenu::new(entries)
}

fn build_configured_entry(entry: &GotoEntrySpec, place_rows: &[PlaceRow]) -> GotoMenuEntry {
    match entry {
        GotoEntrySpec::Builtin { destination, key } => {
            let (label, destination) = builtin_destination(*destination, place_rows);
            GotoMenuEntry::new(*key, label, destination)
        }
        GotoEntrySpec::Custom { title, path, key } => {
            let destination = if path.exists() {
                GotoDestination::Path(path.clone())
            } else {
                GotoDestination::Missing(format!("{title} not available"))
            };
            GotoMenuEntry::new(*key, title, destination)
        }
    }
}

fn builtin_destination(
    destination: BuiltinGoto,
    place_rows: &[PlaceRow],
) -> (&'static str, GotoDestination) {
    match destination {
        BuiltinGoto::Top => ("top", GotoDestination::Top),
        BuiltinGoto::Downloads => (
            "downloads",
            place_path(place_rows, PlaceKind::Downloads)
                .filter(|path| path.exists())
                .map(GotoDestination::Path)
                .unwrap_or_else(|| GotoDestination::Missing("Downloads not available".to_string())),
        ),
        BuiltinGoto::Home => (
            "home",
            crate::elevated_session::home_dir()
                .map(GotoDestination::Path)
                .unwrap_or_else(|| GotoDestination::Missing("Home not available".to_string())),
        ),
        BuiltinGoto::Config => (
            config_label(),
            dirs::config_dir()
                .map(GotoDestination::Path)
                .unwrap_or_else(|| {
                    GotoDestination::Missing(format!("{} not available", config_label()))
                }),
        ),
        BuiltinGoto::Trash => (
            "trash",
            place_path(place_rows, PlaceKind::Trash)
                .or_else(|| {
                    crate::elevated_session::trash_home_dir().and_then(|home| trash_dir(&home))
                })
                .map(GotoDestination::Path)
                .unwrap_or_else(|| GotoDestination::Missing("Trash not available".to_string())),
        ),
    }
}

fn place_path(place_rows: &[PlaceRow], kind: PlaceKind) -> Option<PathBuf> {
    place_rows
        .iter()
        .filter_map(PlaceRow::item)
        .find(|item| item.kind == kind)
        .map(|item| item.path.clone())
}

fn config_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "App Support"
    } else if cfg!(windows) {
        "AppData"
    } else {
        ".config"
    }
}
