use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::fs::{Entry, EntryKind};

use super::{
    DuplicateFile, DuplicateGroup, DuplicateScanBatch, DuplicateScanResult, DuplicateScanStats,
    sort_duplicate_groups,
};

#[derive(Clone, Debug)]
pub(crate) struct DuplicateFinderState {
    pub(crate) cwd: PathBuf,
    pub(crate) groups: Vec<DuplicateGroup>,
    pub(crate) stats: DuplicateScanStats,
    pub(crate) selected: usize,
    pub(crate) scroll: usize,
    pub(crate) selected_paths: HashSet<PathBuf>,
    pub(crate) loading: bool,
    pub(crate) partial: bool,
    pub(crate) error: Option<String>,
    pub(crate) preview_visible: bool,
    pub(crate) preview_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct DuplicateRow {
    pub index: usize,
    pub group_rank: usize,
    pub group_first: bool,
    pub path: PathBuf,
    pub name: String,
    pub parent: String,
    pub size: u64,
    pub selected: bool,
    pub focused: bool,
}

impl DuplicateFinderState {
    pub(crate) fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            groups: Vec::new(),
            stats: DuplicateScanStats::default(),
            selected: 0,
            scroll: 0,
            selected_paths: HashSet::new(),
            loading: true,
            partial: false,
            error: None,
            preview_visible: true,
            preview_path: None,
        }
    }

    pub(crate) fn focused_path(&self) -> Option<PathBuf> {
        self.file_at(self.selected).map(|file| file.path.clone())
    }

    pub(crate) fn focused_entry(&self) -> Option<Entry> {
        let file = self.file_at(self.selected)?;
        Some(Entry {
            path: file.path.clone(),
            name: file.name.clone(),
            name_key: file.name.to_lowercase(),
            kind: EntryKind::File,
            symlink: None,
            size: file.size,
            modified: file.modified,
            readonly: false,
        })
    }

    pub(crate) fn rows(&self, max_rows: usize) -> Vec<DuplicateRow> {
        let end = self.scroll + max_rows;
        let mut rows = Vec::with_capacity(max_rows);
        let mut flat_index = 0usize;
        for (group_rank, group) in self.groups.iter().enumerate() {
            let group_rank = group_rank + 1;
            let group_start = flat_index;
            for file in &group.files {
                if flat_index >= end {
                    return rows;
                }
                if flat_index >= self.scroll {
                    rows.push(DuplicateRow {
                        index: flat_index,
                        group_rank,
                        group_first: flat_index == group_start,
                        path: file.path.clone(),
                        name: file.name.clone(),
                        parent: duplicate_parent_label(&file.path, &self.cwd),
                        size: file.size,
                        selected: self.selected_paths.contains(&file.path),
                        focused: flat_index == self.selected,
                    });
                }
                flat_index += 1;
            }
        }
        rows
    }

    pub(crate) fn file_count(&self) -> usize {
        self.groups.iter().map(|group| group.files.len()).sum()
    }

    pub(crate) fn stop_with_partial_results(&mut self) {
        sort_duplicate_groups(&mut self.groups);
        self.loading = false;
        self.partial = true;
        self.error = None;
        self.selected = 0;
        self.scroll = 0;
        self.stats.groups = self.groups.len();
        self.stats.duplicate_bytes = self
            .groups
            .iter()
            .map(DuplicateGroup::duplicate_bytes)
            .sum();
        self.selected_paths.retain(|path| {
            self.groups
                .iter()
                .any(|group| group.files.iter().any(|file| &file.path == path))
        });
    }

    pub(crate) fn apply_batch(&mut self, batch: DuplicateScanBatch) -> bool {
        let had_files = self.file_count() > 0;
        self.stats = batch.stats;
        self.error = None;
        self.groups.extend(batch.groups);
        !had_files && self.file_count() > 0
    }

    pub(crate) fn apply_result(&mut self, result: Result<DuplicateScanResult, String>) -> bool {
        let had_files = self.file_count() > 0;
        self.loading = false;
        match result {
            Ok(result) => {
                self.groups = result.groups;
                self.stats = result.stats;
                self.selected = 0;
                self.scroll = 0;
                self.partial = false;
                self.error = None;
            }
            Err(error) => {
                self.groups.clear();
                self.stats = DuplicateScanStats::default();
                self.partial = false;
                self.error = Some(error);
            }
        }
        had_files != (self.file_count() > 0)
    }

    pub(crate) fn apply_rename_pairs(&mut self, pairs: Vec<(PathBuf, PathBuf)>) {
        for (old_path, new_path) in pairs {
            if self.selected_paths.remove(&old_path) {
                self.selected_paths.insert(new_path.clone());
            }
            if self.preview_path.as_ref() == Some(&old_path) {
                self.preview_path = None;
            }
            for group in &mut self.groups {
                for file in &mut group.files {
                    if file.path == old_path {
                        file.path = new_path.clone();
                        file.name = new_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .map(str::to_owned)
                            .unwrap_or_else(|| new_path.display().to_string());
                        file.relative = new_path
                            .strip_prefix(&self.cwd)
                            .unwrap_or(&new_path)
                            .to_string_lossy()
                            .replace('\\', "/");
                    }
                }
            }
        }
    }

    pub(crate) fn remove_paths(&mut self, paths: &[PathBuf]) {
        let removed = paths.iter().collect::<HashSet<_>>();
        self.selected_paths.retain(|path| !removed.contains(path));
        if self
            .preview_path
            .as_ref()
            .is_some_and(|path| removed.contains(path))
        {
            self.preview_path = None;
        }
        self.groups.retain_mut(|group| {
            group.files.retain(|file| !removed.contains(&file.path));
            !group.files.is_empty()
        });
        if self.groups.is_empty() {
            self.selected = 0;
            self.scroll = 0;
        }
    }

    fn file_at(&self, index: usize) -> Option<&DuplicateFile> {
        let mut remaining = index;
        for group in &self.groups {
            if remaining < group.files.len() {
                return group.files.get(remaining);
            }
            remaining = remaining.saturating_sub(group.files.len());
        }
        None
    }
}

fn duplicate_parent_label(path: &Path, cwd: &Path) -> String {
    path.parent()
        .and_then(|parent| parent.strip_prefix(cwd).ok())
        .map(|parent| {
            let label = parent.to_string_lossy().replace('\\', "/");
            if label.is_empty() {
                ".".to_string()
            } else {
                label
            }
        })
        .unwrap_or_else(|| {
            path.parent()
                .map(|parent| parent.display().to_string())
                .unwrap_or_default()
        })
}

#[cfg(test)]
#[path = "tests/duplicate_results.rs"]
mod tests;
