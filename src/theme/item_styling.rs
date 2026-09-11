use super::theme_loading::{Palette, Theme, active_theme, normalize_key};
use crate::{
    file_classification::{self, FileClass},
    fs::{Entry, EntryKind, SymlinkInfo},
};
use ratatui::style::Color;
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const ENTRY_CLASS_CACHE_LIMIT: usize = 4_096;

#[derive(Clone)]
pub(super) struct ClassStyle {
    pub(super) icon: String,
    pub(super) color: Color,
}

#[derive(Clone, Default)]
pub(super) struct RuleOverride {
    pub(super) class: Option<FileClass>,
    pub(super) icon: Option<String>,
    pub(super) color: Option<Color>,
}

pub(crate) struct ResolvedAppearance<'a> {
    #[cfg(test)]
    pub class: FileClass,
    pub icon: &'a str,
    pub color: Color,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct EntryClassCacheKey {
    path: PathBuf,
    is_dir: bool,
    size: u64,
    modified: Option<(u64, u32)>,
}

#[derive(Default)]
struct EntryClassCache {
    classes: HashMap<EntryClassCacheKey, FileClass>,
    order: VecDeque<EntryClassCacheKey>,
}

static ENTRY_CLASS_CACHE: OnceLock<Mutex<EntryClassCache>> = OnceLock::new();

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
    let builtin_class = symlink_entry_class(entry)
        .unwrap_or_else(|| file_classification::inspect_entry_cached(entry).builtin_class);
    active_theme().resolve_with_builtin_class(&entry.path, entry.kind, builtin_class)
}

pub(crate) fn resolve_browser_entry(entry: &Entry) -> ResolvedAppearance<'static> {
    let builtin_class = builtin_classify_browser_entry(entry);
    active_theme().resolve_with_builtin_class(&entry.path, entry.kind, builtin_class)
}

pub(crate) fn mix_color(base: Color, tint: Color, tint_weight: u8) -> Color {
    match (base, tint) {
        (Color::Rgb(br, bg, bb), Color::Rgb(tr, tg, tb)) => {
            let weight = u16::from(tint_weight);
            let base_weight = 255 - weight;
            Color::Rgb(
                ((u16::from(br) * base_weight + u16::from(tr) * weight) / 255) as u8,
                ((u16::from(bg) * base_weight + u16::from(tg) * weight) / 255) as u8,
                ((u16::from(bb) * base_weight + u16::from(tb) * weight) / 255) as u8,
            )
        }
        _ => base,
    }
}

pub(crate) fn entry_color(entry: &Entry, palette: Palette) -> Color {
    let _ = palette;
    resolve_entry(entry).color
}

pub(crate) fn entry_symbol(entry: &Entry) -> &'static str {
    resolve_entry(entry).icon
}

pub(crate) fn path_color(path: &Path, is_dir: bool, palette: Palette) -> Color {
    let _ = palette;
    resolve_path(path, entry_kind(is_dir)).color
}

pub(crate) fn path_symbol(path: &Path, is_dir: bool) -> &'static str {
    resolve_path(path, entry_kind(is_dir)).icon
}

pub(crate) fn path_symbol_with_symlink(
    path: &Path,
    is_dir: bool,
    symlink: Option<&SymlinkInfo>,
) -> &'static str {
    let kind = entry_kind(is_dir);
    match symlink_file_class(symlink) {
        Some(class) => resolve_path_with_class(path, kind, class).icon,
        None => resolve_path(path, kind).icon,
    }
}

pub(crate) fn path_color_with_symlink(
    path: &Path,
    is_dir: bool,
    symlink: Option<&SymlinkInfo>,
    palette: Palette,
) -> Color {
    let _ = palette;
    let kind = entry_kind(is_dir);
    match symlink_file_class(symlink) {
        Some(class) => resolve_path_with_class(path, kind, class).color,
        None => resolve_path(path, kind).color,
    }
}

fn entry_kind(is_dir: bool) -> EntryKind {
    if is_dir {
        EntryKind::Directory
    } else {
        EntryKind::File
    }
}

fn symlink_file_class(symlink: Option<&SymlinkInfo>) -> Option<FileClass> {
    let symlink = symlink?;
    match symlink.target_kind {
        Some(EntryKind::Directory) => Some(FileClass::SymlinkDirectory),
        None => Some(FileClass::BrokenSymlink),
        Some(EntryKind::File) => None,
    }
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
    pub(super) fn resolve(&self, path: &Path, kind: EntryKind) -> ResolvedAppearance<'_> {
        let builtin_class = builtin_classify_path(path, kind);
        self.resolve_with_builtin_class(path, kind, builtin_class)
    }

    pub(super) fn resolve_with_builtin_class(
        &self,
        path: &Path,
        kind: EntryKind,
        builtin_class: FileClass,
    ) -> ResolvedAppearance<'_> {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let normalized_name = normalize_key(file_name);
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let template_name = (kind == EntryKind::File)
            .then(|| normalized_name.strip_suffix(".in"))
            .flatten()
            .filter(|name| !name.is_empty());
        let template_ext = template_name
            .and_then(|name| Path::new(name).extension())
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase);

        let exact_rule = match kind {
            EntryKind::Directory => self.directories.get(&normalized_name),
            EntryKind::File => self.files.get(&normalized_name).or_else(|| {
                template_name.and_then(|name| {
                    self.files
                        .get(name)
                        .filter(|rule| is_template_rule_candidate(rule))
                })
            }),
        };
        let ext_rule = (kind == EntryKind::File)
            .then(|| self.extensions.get(&ext))
            .flatten()
            .or_else(|| {
                template_ext
                    .as_deref()
                    .and_then(|ext| self.extensions.get(ext))
                    .filter(|rule| is_template_rule_candidate(rule))
            });
        let prefer_builtin_symlink = matches!(
            builtin_class,
            FileClass::SymlinkDirectory | FileClass::BrokenSymlink
        );
        let prefer_builtin_license = exact_rule.is_none() && builtin_class == FileClass::License;

        let class = if prefer_builtin_symlink {
            builtin_class
        } else {
            exact_rule
                .and_then(|rule| rule.class)
                .or(prefer_builtin_license.then_some(FileClass::License))
                .or_else(|| ext_rule.and_then(|rule| rule.class))
                .unwrap_or(builtin_class)
        };

        let base = self.classes.get(&class).unwrap_or_else(|| {
            self.classes
                .get(&FileClass::File)
                .expect("default file style")
        });

        let icon = if prefer_builtin_symlink {
            base.icon.as_str()
        } else {
            exact_rule
                .and_then(|rule| rule.icon.as_deref())
                .or_else(|| {
                    (!prefer_builtin_license)
                        .then(|| ext_rule.and_then(|rule| rule.icon.as_deref()))
                        .flatten()
                })
                .unwrap_or(base.icon.as_str())
        };
        let color = if prefer_builtin_symlink {
            base.color
        } else {
            exact_rule
                .and_then(|rule| rule.color)
                .or_else(|| {
                    (!prefer_builtin_license)
                        .then(|| ext_rule.and_then(|rule| rule.color))
                        .flatten()
                })
                .unwrap_or(base.color)
        };

        ResolvedAppearance {
            #[cfg(test)]
            class,
            icon,
            color,
        }
    }
}

fn is_template_rule_candidate(rule: &RuleOverride) -> bool {
    rule.class.is_none_or(|class| {
        matches!(
            class,
            FileClass::Code | FileClass::Config | FileClass::Document | FileClass::Data
        )
    })
}

pub(super) fn builtin_classify_path(path: &Path, kind: EntryKind) -> FileClass {
    file_classification::inspect_path(path, kind).builtin_class
}

pub(super) fn builtin_classify_browser_entry(entry: &Entry) -> FileClass {
    if let Some(class) = symlink_entry_class(entry) {
        return class;
    }

    let key = EntryClassCacheKey {
        path: entry.path.clone(),
        is_dir: entry.kind == EntryKind::Directory,
        size: entry.size,
        modified: fingerprint_time(entry.modified),
    };

    {
        let cache = entry_class_cache().lock().expect("entry class cache lock");
        if let Some(class) = cache.get(&key) {
            return class;
        }
    }

    let class = file_classification::inspect_entry_fast(entry).builtin_class;
    entry_class_cache()
        .lock()
        .expect("entry class cache lock")
        .insert(key, class);
    class
}

pub(super) fn symlink_entry_class(entry: &Entry) -> Option<FileClass> {
    let symlink = entry.symlink.as_ref()?;
    Some(match symlink.target_kind {
        Some(EntryKind::Directory) => FileClass::SymlinkDirectory,
        Some(EntryKind::File) => return None,
        None => FileClass::BrokenSymlink,
    })
}

fn fingerprint_time(modified: Option<SystemTime>) -> Option<(u64, u32)> {
    modified
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
}
