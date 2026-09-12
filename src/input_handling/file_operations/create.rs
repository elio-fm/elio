use super::super::text_editing::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use crate::app::App;
use crate::file_browser::{DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub(crate) fn open_create_prompt(&mut self) {
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.file_operations.open_create_prompt();
    }

    pub(crate) fn confirm_create(&mut self) -> Result<()> {
        let cwd = self.file_browser.cwd.clone();
        let Some(completion) = self.file_operations.confirm_create(&cwd)? else {
            return Ok(());
        };
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: cwd.clone(),
            previous_cwd: cwd,
            previous_selected_path: self.selected_entry().map(|entry| entry.path.clone()),
            previous_selection_name: self.selected_entry().map(|entry| entry.name.clone()),
            reselect_path: completion.reselect_path,
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(completion.status),
        })?;
        Ok(())
    }

    pub(crate) fn create_insert_newline(&mut self) {
        let Some(create) = self.file_operations.create_overlay_mut() else {
            return;
        };
        let tail = {
            let byte = char_to_byte(&create.lines[create.cursor_line], create.cursor_col);
            create.lines[create.cursor_line].split_off(byte)
        };
        create.cursor_line += 1;
        create.lines.insert(create.cursor_line, tail);
        create.line_errors.insert(create.cursor_line, None);
        create.cursor_col = 0;
        create.preferred_col = 0;
    }

    fn create_move_horizontal(&mut self, delta: isize) {
        let Some(create) = self.file_operations.create_overlay_mut() else {
            return;
        };
        if delta < 0 {
            if create.cursor_col > 0 {
                create.cursor_col -= 1;
            } else if create.cursor_line > 0 {
                create.cursor_line -= 1;
                create.cursor_col = create.lines[create.cursor_line].chars().count();
            }
        } else {
            let len = create.lines[create.cursor_line].chars().count();
            if create.cursor_col < len {
                create.cursor_col += 1;
            } else if create.cursor_line + 1 < create.lines.len() {
                create.cursor_line += 1;
                create.cursor_col = 0;
            }
        }
        create.preferred_col = create.cursor_col;
    }

    fn create_move_word(&mut self, direction: isize) {
        let Some(create) = self.file_operations.create_overlay_mut() else {
            return;
        };
        let line = &create.lines[create.cursor_line];
        let new_col = if direction < 0 {
            previous_word_start(line, create.cursor_col)
        } else {
            next_word_start(line, create.cursor_col)
        };
        create.cursor_col = new_col;
        create.preferred_col = new_col;
    }

    fn create_move_vertical(&mut self, delta: isize) {
        let Some(create) = self.file_operations.create_overlay_mut() else {
            return;
        };
        let new_line = (create.cursor_line as isize + delta)
            .clamp(0, create.lines.len() as isize - 1) as usize;
        if new_line == create.cursor_line {
            return;
        }
        create.cursor_line = new_line;
        let max_col = create.lines[create.cursor_line].chars().count();
        create.cursor_col = create.preferred_col.min(max_col);
    }

    fn create_backspace(&mut self) {
        let Some(create) = self.file_operations.create_overlay_mut() else {
            return;
        };
        if create.cursor_col > 0 {
            let start = char_to_byte(&create.lines[create.cursor_line], create.cursor_col - 1);
            let end = char_to_byte(&create.lines[create.cursor_line], create.cursor_col);
            create.lines[create.cursor_line].replace_range(start..end, "");
            create.cursor_col -= 1;
            create.preferred_col = create.cursor_col;
            create.line_errors[create.cursor_line] = None;
        } else if create.cursor_line > 0 {
            let removed = create.lines.remove(create.cursor_line);
            create.line_errors.remove(create.cursor_line);
            create.cursor_line -= 1;
            create.cursor_col = create.lines[create.cursor_line].chars().count();
            create.preferred_col = create.cursor_col;
            create.lines[create.cursor_line].push_str(&removed);
            create.line_errors[create.cursor_line] = None;
        }
    }

    fn create_delete(&mut self) {
        let Some(create) = self.file_operations.create_overlay_mut() else {
            return;
        };
        let len = create.lines[create.cursor_line].chars().count();
        if create.cursor_col < len {
            let start = char_to_byte(&create.lines[create.cursor_line], create.cursor_col);
            let end = char_to_byte(&create.lines[create.cursor_line], create.cursor_col + 1);
            create.lines[create.cursor_line].replace_range(start..end, "");
            create.line_errors[create.cursor_line] = None;
        } else if create.cursor_line + 1 < create.lines.len() {
            let next = create.lines.remove(create.cursor_line + 1);
            create.line_errors.remove(create.cursor_line + 1);
            create.lines[create.cursor_line].push_str(&next);
            create.line_errors[create.cursor_line] = None;
        }
    }

    fn create_delete_word_back(&mut self) {
        let Some(create) = self.file_operations.create_overlay_mut() else {
            return;
        };
        if create.cursor_col == 0 {
            return;
        }
        let line = &mut create.lines[create.cursor_line];
        let start = previous_delete_start(line, create.cursor_col);
        remove_char_range(line, start, create.cursor_col);
        create.cursor_col = start;
        create.preferred_col = start;
        create.line_errors[create.cursor_line] = None;
    }

    fn create_delete_word_forward(&mut self) {
        let Some(create) = self.file_operations.create_overlay_mut() else {
            return;
        };
        let line = &mut create.lines[create.cursor_line];
        let end = next_delete_end(line, create.cursor_col);
        remove_char_range(line, create.cursor_col, end);
        create.line_errors[create.cursor_line] = None;
    }

    pub(crate) fn handle_create_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.dismiss_create();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.file_operations.dismiss_create();
            }
            KeyCode::Enter
                if (key.modifiers.contains(KeyModifiers::ALT)
                    || key.modifiers.contains(KeyModifiers::SHIFT))
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.create_insert_newline();
            }
            KeyCode::Char('j') if key.modifiers == KeyModifiers::CONTROL => {
                self.create_insert_newline();
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_create()?;
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_move_word(-1);
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_move_word(1);
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                self.create_move_horizontal(-1);
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                self.create_move_horizontal(1);
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = self.file_operations.create_overlay_mut() {
                    c.cursor_col = 0;
                    c.preferred_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = self.file_operations.create_overlay_mut() {
                    let len = c.lines[c.cursor_line].chars().count();
                    c.cursor_col = len;
                    c.preferred_col = len;
                }
            }
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => {
                self.create_move_vertical(-1);
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                self.create_move_vertical(1);
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_delete_word_back();
            }
            KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_delete_word_back();
            }
            KeyCode::Delete
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_delete_word_forward();
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.create_delete_word_forward();
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                self.create_backspace();
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                self.create_delete();
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(c) = self.file_operations.create_overlay_mut() {
                    let byte = char_to_byte(&c.lines[c.cursor_line], c.cursor_col);
                    c.lines[c.cursor_line].insert(byte, ch);
                    c.cursor_col += 1;
                    c.preferred_col = c.cursor_col;
                    c.line_errors[c.cursor_line] = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_create_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .input
                    .screen_regions
                    .create_panel
                    .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
                if !inside {
                    self.file_operations.dismiss_create();
                    return Ok(());
                }
                if let Some(list_area) = self.input.screen_regions.create_list_area
                    && list_area.contains((mouse.column, mouse.row).into())
                {
                    let scroll_top = self.input.screen_regions.create_scroll_top;
                    let row_offset = (mouse.row - list_area.y) as usize;
                    let line_idx = scroll_top + row_offset;
                    let line_count = self.file_operations.create_line_count();
                    if line_idx < line_count {
                        let line_len = self.file_operations.create_line(line_idx).chars().count();
                        let char_col = (mouse.column.saturating_sub(list_area.x + 3)) as usize;
                        let cursor_col = char_col.min(line_len);
                        if let Some(c) = self.file_operations.create_overlay_mut() {
                            c.cursor_line = line_idx;
                            c.cursor_col = cursor_col;
                            c.preferred_col = cursor_col;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                self.create_move_vertical(-1);
            }
            MouseEventKind::ScrollDown => {
                self.create_move_vertical(1);
            }
            _ => {}
        }
        Ok(())
    }
}
