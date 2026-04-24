use super::{
    entry_class_cache,
    rules::normalize_key,
    types::{EntryClassCacheKey, ResolvedAppearance, Theme},
};
use crate::{
    app::{Entry, EntryKind, FileClass},
    file_info,
};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

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

        let exact_rule = match kind {
            EntryKind::Directory => self.directories.get(&normalized_name),
            EntryKind::File => self.files.get(&normalized_name),
        };
        let ext_rule = (kind == EntryKind::File)
            .then(|| self.extensions.get(&ext))
            .flatten();
        let prefer_builtin_license = exact_rule.is_none() && builtin_class == FileClass::License;

        let class = exact_rule
            .and_then(|rule| rule.class)
            .or(prefer_builtin_license.then_some(FileClass::License))
            .or_else(|| ext_rule.and_then(|rule| rule.class))
            .unwrap_or(builtin_class);

        let base = self.classes.get(&class).unwrap_or_else(|| {
            self.classes
                .get(&FileClass::File)
                .expect("default file style")
        });

        let icon = exact_rule
            .and_then(|rule| rule.icon.as_deref())
            .or_else(|| {
                (!prefer_builtin_license)
                    .then(|| ext_rule.and_then(|rule| rule.icon.as_deref()))
                    .flatten()
            })
            .unwrap_or(base.icon.as_str());
        let color = exact_rule
            .and_then(|rule| rule.color)
            .or_else(|| {
                (!prefer_builtin_license)
                    .then(|| ext_rule.and_then(|rule| rule.color))
                    .flatten()
            })
            .unwrap_or(base.color);

        ResolvedAppearance {
            #[cfg(test)]
            class,
            icon,
            color,
        }
    }
}

pub(super) fn builtin_classify_path(path: &Path, kind: EntryKind) -> FileClass {
    file_info::inspect_path(path, kind).builtin_class
}

pub(super) fn builtin_classify_browser_entry(entry: &Entry) -> FileClass {
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

    let class = file_info::inspect_entry_fast(entry).builtin_class;
    entry_class_cache()
        .lock()
        .expect("entry class cache lock")
        .insert(key, class);
    class
}

fn fingerprint_time(modified: Option<SystemTime>) -> Option<(u64, u32)> {
    modified
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
}
