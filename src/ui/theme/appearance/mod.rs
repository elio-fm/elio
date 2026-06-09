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
    resolve::builtin_classify_browser_entry,
    types::{EntryClassCacheKey, ResolvedAppearance, Theme},
};
use super::builtin_themes::{
    DEFAULT_THEME_NAME, DEFAULT_THEME_TOML, available_theme_names, builtin_theme_overrides,
};
use crate::core::{Entry, EntryKind, FileClass};
use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{Mutex, OnceLock},
};

const ENTRY_CLASS_CACHE_LIMIT: usize = 4_096;

#[derive(Default)]
struct EntryClassCache {
    classes: HashMap<EntryClassCacheKey, FileClass>,
    order: VecDeque<EntryClassCacheKey>,
}

static ACTIVE_THEME: OnceLock<Theme> = OnceLock::new();
static ENTRY_CLASS_CACHE: OnceLock<Mutex<EntryClassCache>> = OnceLock::new();

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

pub(crate) fn resolve_path_with_class(
    path: &Path,
    kind: EntryKind,
    class: FileClass,
) -> ResolvedAppearance<'static> {
    active_theme().resolve_with_builtin_class(path, kind, class)
}

pub(crate) fn resolve_entry(entry: &Entry) -> ResolvedAppearance<'static> {
    let builtin_class = resolve::symlink_entry_class(entry)
        .unwrap_or_else(|| crate::file_info::inspect_entry_cached(entry).builtin_class);
    active_theme().resolve_with_builtin_class(&entry.path, entry.kind, builtin_class)
}

pub(crate) fn resolve_browser_entry(entry: &Entry) -> ResolvedAppearance<'static> {
    let builtin_class = builtin_classify_browser_entry(entry);
    active_theme().resolve_with_builtin_class(&entry.path, entry.kind, builtin_class)
}

#[cfg(test)]
pub(crate) fn specific_type_label(path: &Path, kind: EntryKind) -> Option<&'static str> {
    crate::file_info::inspect_path(path, kind).specific_type_label
}

fn active_theme() -> &'static Theme {
    ACTIVE_THEME.get_or_init(Theme::default_theme)
}

fn entry_class_cache() -> &'static Mutex<EntryClassCache> {
    ENTRY_CLASS_CACHE.get_or_init(|| Mutex::new(EntryClassCache::default()))
}

impl EntryClassCache {
    fn get(&self, key: &EntryClassCacheKey) -> Option<FileClass> {
        self.classes.get(key).copied()
    }

    fn insert(&mut self, key: EntryClassCacheKey, class: FileClass) {
        self.classes.insert(key.clone(), class);
        self.order.retain(|cached| cached != &key);
        self.order.push_back(key);
        while self.order.len() > ENTRY_CLASS_CACHE_LIMIT {
            if let Some(stale_key) = self.order.pop_front() {
                self.classes.remove(&stale_key);
            }
        }
    }
}

impl Theme {
    fn default_theme() -> Self {
        Self::apply_config_on(Self::base_theme(), DEFAULT_THEME_TOML).unwrap_or_else(|error| {
            eprintln!("elio: failed to load built-in default theme: {error}");
            Self::base_theme()
        })
    }

    /// The built-in theme selected by the top-level `theme` key in config.toml
    /// (the default theme when the key is absent). A user `theme.toml` layers
    /// on top of whatever this returns.
    pub(super) fn selected_builtin_theme(name: Option<&str>) -> Self {
        let name = name.unwrap_or(DEFAULT_THEME_NAME);
        if name == "default" {
            return Self::default_theme();
        }
        let Some(overrides) = builtin_theme_overrides(name) else {
            eprintln!(
                "elio: unknown theme \"{name}\" in config.toml; available built-in themes: {}",
                available_theme_names()
            );
            return Self::default_theme();
        };
        Self::apply_config_on(Self::default_theme(), overrides).unwrap_or_else(|error| {
            eprintln!("elio: failed to load built-in theme \"{name}\": {error}");
            Self::default_theme()
        })
    }
}
