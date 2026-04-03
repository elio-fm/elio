mod loading;
mod parsing;
mod resolve;
mod rules;
#[cfg(test)]
mod tests;
mod types;

pub(crate) use self::types::{CodePreviewPalette, Palette};
use self::{
    loading::load_theme_from_disk,
    resolve::builtin_classify_entry,
    types::{EntryClassCacheKey, ResolvedAppearance, Theme},
};
use super::builtin_themes::DEFAULT_THEME_TOML;
use crate::core::{Entry, EntryKind, FileClass};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, OnceLock},
};

static ACTIVE_THEME: OnceLock<Theme> = OnceLock::new();
static ENTRY_CLASS_CACHE: OnceLock<Mutex<HashMap<EntryClassCacheKey, FileClass>>> = OnceLock::new();

pub(crate) fn initialize() {
    let _ = ACTIVE_THEME.get_or_init(load_theme_from_disk);
}

pub(crate) fn palette() -> Palette {
    active_theme().palette
}

pub(crate) fn code_preview_palette() -> CodePreviewPalette {
    active_theme().preview.code
}

pub(crate) fn resolve_path(path: &Path, kind: EntryKind) -> ResolvedAppearance<'static> {
    active_theme().resolve(path, kind)
}

pub(crate) fn resolve_entry(entry: &Entry) -> ResolvedAppearance<'static> {
    let builtin_class = builtin_classify_entry(entry);
    active_theme().resolve_with_builtin_class(&entry.path, entry.kind, builtin_class)
}

#[cfg(test)]
pub(crate) fn specific_type_label(path: &Path, kind: EntryKind) -> Option<&'static str> {
    crate::file_info::inspect_path(path, kind).specific_type_label
}

fn active_theme() -> &'static Theme {
    ACTIVE_THEME.get_or_init(Theme::default_theme)
}

fn entry_class_cache() -> &'static Mutex<HashMap<EntryClassCacheKey, FileClass>> {
    ENTRY_CLASS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

impl Theme {
    fn default_theme() -> Self {
        Self::apply_config_on(Self::base_theme(), DEFAULT_THEME_TOML).unwrap_or_else(|error| {
            eprintln!("elio: failed to load built-in default theme: {error}");
            Self::base_theme()
        })
    }
}
