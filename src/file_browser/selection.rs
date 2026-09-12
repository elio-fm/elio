use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default)]
pub(crate) struct SelectedPaths {
    inner: HashSet<PathBuf>,
    order: Vec<PathBuf>,
    ancestor_counts: HashMap<PathBuf, usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionChange {
    Inserted,
    Removed,
    NestingConflict,
}

impl SelectedPaths {
    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(crate) fn contains(&self, path: &Path) -> bool {
        self.inner.contains(path)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &PathBuf> {
        self.inner.iter()
    }

    #[cfg(unix)]
    pub(crate) fn ordered(&self) -> impl Iterator<Item = &PathBuf> {
        self.order.iter()
    }

    pub(crate) fn clear(&mut self) {
        self.inner.clear();
        self.order.clear();
        self.ancestor_counts.clear();
    }

    pub(crate) fn insert(&mut self, path: PathBuf) -> bool {
        if self.has_nesting_conflict(&path) {
            return false;
        }
        if !self.inner.insert(path.clone()) {
            return false;
        }
        self.order.push(path.clone());
        self.add_ancestors(&path);
        true
    }

    pub(crate) fn remove(&mut self, path: &Path) -> bool {
        if !self.inner.remove(path) {
            return false;
        }
        self.order.retain(|selected| selected != path);
        self.remove_ancestors(path);
        true
    }

    pub(crate) fn has_nesting_conflict(&self, path: &Path) -> bool {
        self.ancestor_counts.contains_key(path)
            || path
                .ancestors()
                .skip(1)
                .any(|ancestor| self.inner.contains(ancestor))
    }

    fn add_ancestors(&mut self, path: &Path) {
        for ancestor in path.ancestors().skip(1) {
            *self
                .ancestor_counts
                .entry(ancestor.to_path_buf())
                .or_default() += 1;
        }
    }

    fn remove_ancestors(&mut self, path: &Path) {
        for ancestor in path.ancestors().skip(1) {
            let Some(count) = self.ancestor_counts.get_mut(ancestor) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.ancestor_counts.remove(ancestor);
            }
        }
    }
}

impl super::FileBrowserState {
    pub(crate) fn is_selected(&self, path: &Path) -> bool {
        self.selected_paths.contains(path)
    }

    pub(crate) fn selection_count(&self) -> usize {
        self.selected_paths.len()
    }

    pub(crate) fn selected_paths_sorted(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.selected_paths.iter().cloned().collect();
        paths.sort();
        paths
    }

    #[cfg(unix)]
    pub(crate) fn selected_paths_in_selection_order(&self) -> Vec<PathBuf> {
        self.selected_paths.ordered().cloned().collect()
    }

    pub(crate) fn toggle_selected_path(&mut self, path: PathBuf) -> SelectionChange {
        if self.selected_paths.remove(&path) {
            return SelectionChange::Removed;
        }
        if self.selected_paths.has_nesting_conflict(&path) {
            return SelectionChange::NestingConflict;
        }
        self.selected_paths.insert(path);
        SelectionChange::Inserted
    }

    pub(crate) fn select_all_visible(&mut self) -> bool {
        let mut blocked = false;
        for path in self.entries.iter().map(|entry| entry.path.clone()) {
            if self.selected_paths.has_nesting_conflict(&path) {
                blocked = true;
                continue;
            }
            self.selected_paths.insert(path);
        }
        blocked
    }

    pub(crate) fn clear_selection(&mut self) -> bool {
        if self.selected_paths.is_empty() {
            return false;
        }
        self.selected_paths.clear();
        true
    }
}
