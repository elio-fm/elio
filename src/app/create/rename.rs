use super::super::text_edit::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use super::super::{
    App,
    state::{DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad, RenameOverlay},
};
use crate::fs::rect_contains;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::fs;

impl App {
    pub(in crate::app) fn open_rename_prompt(&mut self) {
        if self.in_trash {
            return;
        }
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let name = entry.name.clone();
        let is_dir = entry.is_dir();
        let cursor_col = cursor_before_extension(&name);
        self.help_open = false;
        self.search = None;
        self.create = None;
        self.trash = None;
        self.restore = None;
        self.rename = Some(RenameOverlay {
            is_dir,
            original_name: name.clone(),
            input: name,
            cursor_col,
            error: None,
        });
    }

    pub fn rename_is_open(&self) -> bool {
        self.rename.is_some()
    }

    pub fn rename_input(&self) -> &str {
        self.rename.as_ref().map_or("", |r| &r.input)
    }

    pub fn rename_cursor_col(&self) -> usize {
        self.rename.as_ref().map_or(0, |r| r.cursor_col)
    }

    pub fn rename_original_name(&self) -> &str {
        self.rename.as_ref().map_or("", |r| &r.original_name)
    }

    pub fn rename_item_is_dir(&self) -> bool {
        self.rename.as_ref().is_some_and(|r| r.is_dir)
    }

    pub fn rename_error(&self) -> Option<&str> {
        self.rename.as_ref().and_then(|r| r.error.as_deref())
    }

    pub(in crate::app) fn handle_rename_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.rename = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.rename = None;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_rename()?;
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.rename {
                    let new_col = previous_word_start(&r.input, r.cursor_col);
                    r.cursor_col = new_col;
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.rename {
                    let new_col = next_word_start(&r.input, r.cursor_col);
                    r.cursor_col = new_col;
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.rename {
                    r.cursor_col = r.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.rename {
                    let len = r.input.chars().count();
                    if r.cursor_col < len {
                        r.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.rename {
                    r.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.rename {
                    r.cursor_col = r.input.chars().count();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.rename
                    && r.cursor_col > 0
                {
                    let start = previous_delete_start(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, start, r.cursor_col);
                    r.cursor_col = start;
                    r.error = None;
                }
            }
            KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.rename
                    && r.cursor_col > 0
                {
                    let start = previous_delete_start(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, start, r.cursor_col);
                    r.cursor_col = start;
                    r.error = None;
                }
            }
            KeyCode::Delete
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.rename {
                    let end = next_delete_end(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, r.cursor_col, end);
                    r.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(r) = &mut self.rename {
                    let end = next_delete_end(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, r.cursor_col, end);
                    r.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.rename
                    && r.cursor_col > 0
                {
                    let start = char_to_byte(&r.input, r.cursor_col - 1);
                    let end = char_to_byte(&r.input, r.cursor_col);
                    r.input.replace_range(start..end, "");
                    r.cursor_col -= 1;
                    r.error = None;
                }
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.rename {
                    let len = r.input.chars().count();
                    if r.cursor_col < len {
                        let start = char_to_byte(&r.input, r.cursor_col);
                        let end = char_to_byte(&r.input, r.cursor_col + 1);
                        r.input.replace_range(start..end, "");
                        r.error = None;
                    }
                }
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.rename {
                    let byte = char_to_byte(&r.input, r.cursor_col);
                    r.input.insert(byte, ch);
                    r.cursor_col += 1;
                    r.error = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::app) fn handle_rename_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .frame_state
                .rename_panel
                .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
            if !inside {
                self.rename = None;
            }
        }
        Ok(())
    }

    pub(super) fn confirm_rename(&mut self) -> Result<()> {
        let Some(r) = &self.rename else {
            return Ok(());
        };
        let new_name = r.input.trim().to_string();
        let original_name = r.original_name.clone();

        if new_name.is_empty() {
            if let Some(r) = &mut self.rename {
                r.error = Some("Name cannot be empty".to_string());
            }
            return Ok(());
        }
        if new_name.contains('/') {
            if let Some(r) = &mut self.rename {
                r.error = Some("Name cannot contain /".to_string());
            }
            return Ok(());
        }
        if new_name == original_name {
            self.rename = None;
            return Ok(());
        }
        let new_path = self.cwd.join(&new_name);
        if new_path.exists() {
            if let Some(r) = &mut self.rename {
                r.error = Some(format!("\"{}\" already exists", new_name));
            }
            return Ok(());
        }

        let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.name == original_name)
        else {
            self.rename = None;
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
            if let Some(r) = &mut self.rename {
                r.error = Some(msg);
            }
            return Ok(());
        }

        self.rename = None;
        let status = format!("Renamed \"{}\" → \"{}\"", original_name, new_name);
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: self.cwd.clone(),
            previous_cwd: self.cwd.clone(),
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
