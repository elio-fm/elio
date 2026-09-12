use crate::app::App;
use crate::file_browser::{DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad};
use anyhow::Result;
use std::fs;

#[derive(Clone, Debug)]
pub(crate) struct RenameOverlay {
    pub(crate) is_dir: bool,
    pub(crate) original_name: String,
    pub(crate) input: String,
    pub(crate) cursor_col: usize,
    pub(crate) error: Option<String>,
}

impl App {
    pub(crate) fn open_rename_prompt(&mut self) {
        if self.file_browser.in_trash {
            return;
        }
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let name = entry.name.clone();
        let is_dir = entry.is_dir();
        let cursor_col = cursor_before_extension(&name);
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.file_operations.create = None;
        self.file_operations.trash = None;
        self.file_operations.restore = None;
        self.file_operations.rename = Some(RenameOverlay {
            is_dir,
            original_name: name.clone(),
            input: name,
            cursor_col,
            error: None,
        });
    }

    pub fn rename_is_open(&self) -> bool {
        self.file_operations.rename.is_some()
    }

    pub fn rename_input(&self) -> &str {
        self.file_operations
            .rename
            .as_ref()
            .map_or("", |r| &r.input)
    }

    pub fn rename_cursor_col(&self) -> usize {
        self.file_operations
            .rename
            .as_ref()
            .map_or(0, |r| r.cursor_col)
    }

    pub fn rename_original_name(&self) -> &str {
        self.file_operations
            .rename
            .as_ref()
            .map_or("", |r| &r.original_name)
    }

    pub fn rename_item_is_dir(&self) -> bool {
        self.file_operations
            .rename
            .as_ref()
            .is_some_and(|r| r.is_dir)
    }

    pub fn rename_error(&self) -> Option<&str> {
        self.file_operations
            .rename
            .as_ref()
            .and_then(|r| r.error.as_deref())
    }

    pub(crate) fn confirm_rename(&mut self) -> Result<()> {
        let Some(r) = &self.file_operations.rename else {
            return Ok(());
        };
        let new_name = r.input.trim().to_string();
        let original_name = r.original_name.clone();

        if new_name.is_empty() {
            if let Some(r) = &mut self.file_operations.rename {
                r.error = Some("Name cannot be empty".to_string());
            }
            return Ok(());
        }
        if new_name.contains('/') {
            if let Some(r) = &mut self.file_operations.rename {
                r.error = Some("Name cannot contain /".to_string());
            }
            return Ok(());
        }
        if new_name == original_name {
            self.file_operations.rename = None;
            return Ok(());
        }
        if self.duplicates_is_open() {
            return self.confirm_duplicate_rename(original_name, new_name);
        }
        let new_path = self.file_browser.cwd.join(&new_name);
        if new_path.exists() {
            if let Some(r) = &mut self.file_operations.rename {
                r.error = Some(format!("\"{}\" already exists", new_name));
            }
            return Ok(());
        }

        let Some(entry) = self
            .file_browser
            .entries
            .iter()
            .find(|entry| entry.name == original_name)
        else {
            self.file_operations.rename = None;
            return Ok(());
        };
        let old_path = entry.path.clone();

        if let Err(error) = fs::rename(&old_path, &new_path) {
            let msg = match error.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    format!("Permission denied renaming \"{}\"", original_name)
                }
                _ => format!("Could not rename: {error}"),
            };
            if let Some(r) = &mut self.file_operations.rename {
                r.error = Some(msg);
            }
            return Ok(());
        }

        self.file_operations.rename = None;
        let status = format!("Renamed \"{}\" → \"{}\"", original_name, new_name);
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: self.file_browser.cwd.clone(),
            previous_cwd: self.file_browser.cwd.clone(),
            previous_selected_path: None,
            previous_selection_name: None,
            reselect_path: Some(new_path),
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(status),
        })?;
        Ok(())
    }
}

pub(super) fn cursor_before_extension(name: &str) -> usize {
    let total = name.chars().count();
    if let Some(dot_pos) = name.rfind('.') {
        let dot_char = name[..dot_pos].chars().count();
        if dot_char > 0 {
            return dot_char;
        }
    }
    total
}
