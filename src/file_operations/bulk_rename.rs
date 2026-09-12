use super::editor_bulk_rename::confirm_bulk_rename_overlay;
use crate::app::App;
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BulkRenameItem {
    pub(crate) path: PathBuf,
    pub(crate) original_name: String,
    pub(crate) is_dir: bool,
}

pub(crate) struct BulkRenameOverlay {
    pub(crate) items: Vec<BulkRenameItem>,
    pub(crate) new_names: Vec<String>,
    pub(crate) root: Option<PathBuf>,
    pub(crate) cursor_line: usize,
    pub(crate) cursor_col: usize,
    pub(crate) preferred_col: usize,
    pub(crate) line_errors: Vec<Option<String>>,
}

impl App {
    pub(crate) fn open_bulk_rename_prompt(&mut self) {
        if self.file_browser.in_trash {
            return;
        }
        let selected_paths = self.selected_paths_sorted();
        if selected_paths
            .iter()
            .any(|path| self.trash_target_is_inside_trash(path))
        {
            self.status = "Cannot rename items from Trash".to_string();
            return;
        }
        let items: Vec<BulkRenameItem> = selected_paths
            .into_iter()
            .map(bulk_rename_item_from_path)
            .collect();
        if items.is_empty() {
            return;
        }
        let count = items.len();
        let new_names: Vec<String> = items
            .iter()
            .map(|item| item.original_name.clone())
            .collect();
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.file_operations.create = None;
        self.file_operations.rename = None;
        self.file_operations.trash = None;
        self.file_operations.restore = None;
        self.file_operations.bulk_rename = Some(BulkRenameOverlay {
            items,
            new_names,
            root: None,
            cursor_line: 0,
            cursor_col: 0,
            preferred_col: 0,
            line_errors: vec![None; count],
        });
    }

    pub fn bulk_rename_is_open(&self) -> bool {
        self.file_operations.bulk_rename.is_some()
    }

    pub fn bulk_rename_title(&self) -> String {
        let Some(r) = &self.file_operations.bulk_rename else {
            return "Rename".to_string();
        };
        if r.items.len() == 1 {
            return format!("Rename \"{}\"", r.items[0].original_name);
        }
        let files = r.items.iter().filter(|item| !item.is_dir).count();
        let dirs = r.items.iter().filter(|item| item.is_dir).count();
        match (files, dirs) {
            (f, 0) => format!("Rename {} file{}", f, if f == 1 { "" } else { "s" }),
            (0, d) => format!("Rename {} folder{}", d, if d == 1 { "" } else { "s" }),
            (f, d) => format!(
                "Rename {} file{} and {} folder{}",
                f,
                if f == 1 { "" } else { "s" },
                d,
                if d == 1 { "" } else { "s" },
            ),
        }
    }

    pub fn bulk_rename_item_count(&self) -> usize {
        self.file_operations
            .bulk_rename
            .as_ref()
            .map_or(0, |r| r.items.len())
    }

    pub fn bulk_rename_new_name(&self, index: usize) -> &str {
        self.file_operations
            .bulk_rename
            .as_ref()
            .and_then(|r| r.new_names.get(index))
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn bulk_rename_item_is_dir(&self, index: usize) -> bool {
        self.file_operations
            .bulk_rename
            .as_ref()
            .and_then(|r| r.items.get(index))
            .is_some_and(|item| item.is_dir)
    }

    pub fn bulk_rename_live_path(&self, index: usize) -> PathBuf {
        self.file_operations
            .bulk_rename
            .as_ref()
            .and_then(|r| {
                r.new_names
                    .get(index)
                    .map(|name| r.root.as_ref().unwrap_or(&self.file_browser.cwd).join(name))
            })
            .unwrap_or_else(|| self.file_browser.cwd.clone())
    }

    pub fn bulk_rename_line_error(&self, index: usize) -> Option<&str> {
        self.file_operations
            .bulk_rename
            .as_ref()
            .and_then(|r| r.line_errors.get(index))
            .and_then(Option::as_deref)
    }

    pub fn bulk_rename_cursor_line(&self) -> usize {
        self.file_operations
            .bulk_rename
            .as_ref()
            .map_or(0, |r| r.cursor_line)
    }

    pub fn bulk_rename_cursor_col(&self) -> usize {
        self.file_operations
            .bulk_rename
            .as_ref()
            .map_or(0, |r| r.cursor_col)
    }

    pub(crate) fn confirm_bulk_rename(&mut self) -> Result<()> {
        confirm_bulk_rename_overlay(self)
    }
}

fn bulk_rename_item_from_path(path: PathBuf) -> BulkRenameItem {
    let original_name = path_name(&path);
    let is_dir = path.is_dir();
    BulkRenameItem {
        path,
        original_name,
        is_dir,
    }
}

fn path_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}
