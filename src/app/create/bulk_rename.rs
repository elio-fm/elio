use super::super::text_edit::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use super::super::{
    App,
    state::{
        BulkRenameItem, BulkRenameOverlay, DirectoryHistoryMode, DirectoryLoadCompletion,
        PendingDirectoryLoad,
    },
};
use crate::fs::rect_contains;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::{fs, path::PathBuf};

impl App {
    pub(in crate::app) fn open_bulk_rename_prompt(&mut self) {
        if self.navigation.in_trash {
            return;
        }
        let items: Vec<BulkRenameItem> = self
            .navigation
            .entries
            .iter()
            .filter(|entry| self.navigation.selected_paths.contains(&entry.path))
            .map(|entry| BulkRenameItem {
                path: entry.path.clone(),
                original_name: entry.name.clone(),
                is_dir: entry.is_dir(),
            })
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
        self.overlays.search = None;
        self.overlays.create = None;
        self.overlays.rename = None;
        self.overlays.trash = None;
        self.overlays.restore = None;
        self.overlays.bulk_rename = Some(BulkRenameOverlay {
            items,
            new_names,
            cursor_line: 0,
            cursor_col: 0,
            preferred_col: 0,
            line_errors: vec![None; count],
        });
    }

    pub fn bulk_rename_is_open(&self) -> bool {
        self.overlays.bulk_rename.is_some()
    }

    pub fn bulk_rename_title(&self) -> String {
        let Some(r) = &self.overlays.bulk_rename else {
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
        self.overlays
            .bulk_rename
            .as_ref()
            .map_or(0, |r| r.items.len())
    }

    pub fn bulk_rename_new_name(&self, index: usize) -> &str {
        self.overlays
            .bulk_rename
            .as_ref()
            .and_then(|r| r.new_names.get(index))
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn bulk_rename_item_is_dir(&self, index: usize) -> bool {
        self.overlays
            .bulk_rename
            .as_ref()
            .and_then(|r| r.items.get(index))
            .is_some_and(|item| item.is_dir)
    }

    pub fn bulk_rename_line_error(&self, index: usize) -> Option<&str> {
        self.overlays
            .bulk_rename
            .as_ref()
            .and_then(|r| r.line_errors.get(index))
            .and_then(Option::as_deref)
    }

    pub fn bulk_rename_cursor_line(&self) -> usize {
        self.overlays
            .bulk_rename
            .as_ref()
            .map_or(0, |r| r.cursor_line)
    }

    pub fn bulk_rename_cursor_col(&self) -> usize {
        self.overlays
            .bulk_rename
            .as_ref()
            .map_or(0, |r| r.cursor_col)
    }

    pub(in crate::app) fn handle_bulk_rename_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.bulk_rename = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.overlays.bulk_rename = None;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_bulk_rename()?;
            }
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => {
                self.bulk_rename_move_vertical(-1);
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                self.bulk_rename_move_vertical(1);
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    let new_col = previous_word_start(&r.new_names[r.cursor_line], r.cursor_col);
                    r.cursor_col = new_col;
                    r.preferred_col = new_col;
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    let new_col = next_word_start(&r.new_names[r.cursor_line], r.cursor_col);
                    r.cursor_col = new_col;
                    r.preferred_col = new_col;
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    r.cursor_col = r.cursor_col.saturating_sub(1);
                    r.preferred_col = r.cursor_col;
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    let len = r.new_names[r.cursor_line].chars().count();
                    if r.cursor_col < len {
                        r.cursor_col += 1;
                    }
                    r.preferred_col = r.cursor_col;
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    r.cursor_col = 0;
                    r.preferred_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    r.cursor_col = r.new_names[r.cursor_line].chars().count();
                    r.preferred_col = r.cursor_col;
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.overlays.bulk_rename
                    && r.cursor_col > 0
                {
                    let start = previous_delete_start(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], start, r.cursor_col);
                    r.cursor_col = start;
                    r.preferred_col = start;
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.overlays.bulk_rename
                    && r.cursor_col > 0
                {
                    let start = previous_delete_start(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], start, r.cursor_col);
                    r.cursor_col = start;
                    r.preferred_col = start;
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Delete
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    let end = next_delete_end(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], r.cursor_col, end);
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    let end = next_delete_end(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], r.cursor_col, end);
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.overlays.bulk_rename
                    && r.cursor_col > 0
                {
                    let start = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col - 1);
                    let end = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col);
                    r.new_names[r.cursor_line].replace_range(start..end, "");
                    r.cursor_col -= 1;
                    r.preferred_col = r.cursor_col;
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    let len = r.new_names[r.cursor_line].chars().count();
                    if r.cursor_col < len {
                        let start = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col);
                        let end = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col + 1);
                        r.new_names[r.cursor_line].replace_range(start..end, "");
                        r.line_errors[r.cursor_line] = None;
                    }
                }
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.overlays.bulk_rename {
                    let byte = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col);
                    r.new_names[r.cursor_line].insert(byte, ch);
                    r.cursor_col += 1;
                    r.preferred_col = r.cursor_col;
                    r.line_errors[r.cursor_line] = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn bulk_rename_move_vertical(&mut self, delta: isize) {
        let Some(r) = &mut self.overlays.bulk_rename else {
            return;
        };
        let new_line =
            (r.cursor_line as isize + delta).clamp(0, r.items.len() as isize - 1) as usize;
        if new_line == r.cursor_line {
            return;
        }
        r.cursor_line = new_line;
        let max_col = r.new_names[r.cursor_line].chars().count();
        r.cursor_col = r.preferred_col.min(max_col);
    }

    pub(in crate::app) fn handle_bulk_rename_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .input
                    .frame_state
                    .rename_panel
                    .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
                if !inside {
                    self.overlays.bulk_rename = None;
                    return Ok(());
                }
                if let Some(list_area) = self.input.frame_state.bulk_rename_list_area
                    && rect_contains(list_area, mouse.column, mouse.row)
                {
                    let scroll_top = self.input.frame_state.bulk_rename_scroll_top;
                    let row_offset = (mouse.row - list_area.y) as usize;
                    let line_idx = scroll_top + row_offset;
                    let count = self.bulk_rename_item_count();
                    if line_idx < count {
                        let line_len = self.bulk_rename_new_name(line_idx).chars().count();
                        let char_col = (mouse.column.saturating_sub(list_area.x + 3)) as usize;
                        let cursor_col = char_col.min(line_len);
                        if let Some(r) = &mut self.overlays.bulk_rename {
                            r.cursor_line = line_idx;
                            r.cursor_col = cursor_col;
                            r.preferred_col = cursor_col;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                self.bulk_rename_move_vertical(-1);
            }
            MouseEventKind::ScrollDown => {
                self.bulk_rename_move_vertical(1);
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::app::create) fn confirm_bulk_rename(&mut self) -> Result<()> {
        let Some(r) = &self.overlays.bulk_rename else {
            return Ok(());
        };

        let count = r.items.len();
        let mut errors: Vec<Option<String>> = vec![None; count];
        let mut first_error: Option<usize> = None;
        let renaming_paths: std::collections::HashSet<&PathBuf> =
            r.items.iter().map(|item| &item.path).collect();
        let mut seen_new_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for (index, (item, new_name_raw)) in r.items.iter().zip(r.new_names.iter()).enumerate() {
            let new_name = new_name_raw.trim().to_string();
            let err = if new_name.is_empty() {
                Some("Name cannot be empty".to_string())
            } else if new_name.contains('/') {
                Some("Name cannot contain /".to_string())
            } else if !seen_new_names.insert(new_name.clone()) {
                Some(format!("\"{}\" appears more than once", new_name))
            } else {
                let new_path = self.navigation.cwd.join(&new_name);
                if new_path.exists() && !renaming_paths.contains(&new_path) {
                    Some(format!("\"{}\" already exists", new_name))
                } else {
                    let _ = item;
                    None
                }
            };
            if let Some(msg) = err {
                errors[index] = Some(msg);
                if first_error.is_none() {
                    first_error = Some(index);
                }
            }
        }

        if let Some(err_line) = first_error {
            if let Some(r) = &mut self.overlays.bulk_rename {
                r.line_errors = errors;
                r.cursor_line = err_line;
                r.cursor_col = r.cursor_col.min(r.new_names[err_line].chars().count());
                r.preferred_col = r.cursor_col;
            }
            return Ok(());
        }

        let ops: Vec<(PathBuf, String, String)> = r
            .items
            .iter()
            .zip(r.new_names.iter())
            .map(|(item, new_name)| {
                (
                    item.path.clone(),
                    item.original_name.clone(),
                    new_name.trim().to_string(),
                )
            })
            .collect();

        self.overlays.bulk_rename = None;
        self.navigation.selected_paths.clear();

        let mut renamed = 0usize;
        let mut last_new_path: Option<PathBuf> = None;

        for (old_path, original_name, new_name) in &ops {
            if *new_name == *original_name {
                continue;
            }
            let new_path = self.navigation.cwd.join(new_name);
            if let Err(error) = fs::rename(old_path, &new_path) {
                let msg = match error.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        format!("Permission denied renaming \"{}\"", original_name)
                    }
                    _ => format!("Could not rename \"{}\": {error}", original_name),
                };
                self.queue_directory_load(PendingDirectoryLoad {
                    token: 0,
                    target_cwd: self.navigation.cwd.clone(),
                    previous_cwd: self.navigation.cwd.clone(),
                    previous_selected_path: None,
                    previous_selection_name: None,
                    reselect_path: last_new_path,
                    history_mode: DirectoryHistoryMode::None,
                    refresh_search: false,
                    completion: DirectoryLoadCompletion::Status(msg),
                })?;
                return Ok(());
            }
            last_new_path = Some(self.navigation.cwd.join(new_name));
            renamed += 1;
        }

        let status = match renamed {
            0 => "No files renamed".to_string(),
            1 => {
                let (_, original, new_name) = ops
                    .iter()
                    .find(|(_, original, new_name)| original != new_name)
                    .expect("changed rename op should exist");
                format!("Renamed \"{}\" → \"{}\"", original, new_name)
            }
            n => format!("Renamed {} items", n),
        };
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: self.navigation.cwd.clone(),
            previous_cwd: self.navigation.cwd.clone(),
            previous_selected_path: None,
            previous_selection_name: None,
            reselect_path: last_new_path,
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(status),
        })?;
        Ok(())
    }
}
