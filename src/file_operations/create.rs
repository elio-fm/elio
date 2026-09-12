use crate::app::{App, DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad};
use crate::input_handling::text_editing::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::{fs, path::Path};

pub(crate) struct CreateOverlay {
    pub(crate) lines: Vec<String>,
    pub(crate) cursor_line: usize,
    pub(crate) cursor_col: usize,
    pub(crate) preferred_col: usize,
    pub(crate) line_errors: Vec<Option<String>>,
}

impl App {
    pub fn create_is_open(&self) -> bool {
        self.file_operations.create.is_some()
    }

    pub fn create_line_count(&self) -> usize {
        self.file_operations
            .create
            .as_ref()
            .map_or(0, |c| c.lines.len())
    }

    pub fn create_line(&self, index: usize) -> &str {
        self.file_operations
            .create
            .as_ref()
            .and_then(|c| c.lines.get(index))
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn create_cursor_line(&self) -> usize {
        self.file_operations
            .create
            .as_ref()
            .map_or(0, |c| c.cursor_line)
    }

    pub fn create_cursor_col(&self) -> usize {
        self.file_operations
            .create
            .as_ref()
            .map_or(0, |c| c.cursor_col)
    }

    pub fn create_title(&self) -> String {
        let Some(c) = &self.file_operations.create else {
            return "Create".to_string();
        };
        let files = c
            .lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with('/') && !trimmed.ends_with('/')
            })
            .count();
        let dirs = c
            .lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && (trimmed.starts_with('/') || trimmed.ends_with('/'))
            })
            .count();
        match (files, dirs) {
            (0, 0) => "Create".to_string(),
            (f, 0) => format!("Create {} file{}", f, if f == 1 { "" } else { "s" }),
            (0, d) => format!("Create {} folder{}", d, if d == 1 { "" } else { "s" }),
            (f, d) => format!(
                "Create {} file{} and {} folder{}",
                f,
                if f == 1 { "" } else { "s" },
                d,
                if d == 1 { "" } else { "s" },
            ),
        }
    }

    pub fn create_line_error(&self, index: usize) -> Option<&str> {
        self.file_operations
            .create
            .as_ref()
            .and_then(|c| c.line_errors.get(index))
            .and_then(Option::as_deref)
    }
}

impl App {
    pub(crate) fn create_insert_newline(&mut self) {
        let Some(create) = &mut self.file_operations.create else {
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
        let Some(create) = &mut self.file_operations.create else {
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
        let Some(create) = &mut self.file_operations.create else {
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
        let Some(create) = &mut self.file_operations.create else {
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
        let Some(create) = &mut self.file_operations.create else {
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
        let Some(create) = &mut self.file_operations.create else {
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
        let Some(create) = &mut self.file_operations.create else {
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
        let Some(create) = &mut self.file_operations.create else {
            return;
        };
        let line = &mut create.lines[create.cursor_line];
        let end = next_delete_end(line, create.cursor_col);
        remove_char_range(line, create.cursor_col, end);
        create.line_errors[create.cursor_line] = None;
    }
}

struct ParsedCreateItem {
    raw: String,
    name: String,
    is_dir: bool,
}

fn parse_create_line(line: &str) -> ParsedCreateItem {
    let is_dir = line.starts_with('/') || line.ends_with('/');
    let name = line.trim_matches('/').to_string();
    ParsedCreateItem {
        raw: line.to_string(),
        name,
        is_dir,
    }
}

fn validate_parsed_item(item: &ParsedCreateItem, cwd: &Path) -> Option<String> {
    if item.name.is_empty() {
        return Some("Name cannot be empty".to_string());
    }
    if item.name.contains('/') {
        return Some("Name cannot contain /".to_string());
    }
    if cwd.join(&item.name).exists() {
        return Some(format!("\"{}\" already exists", item.name));
    }
    None
}

impl App {
    pub(crate) fn open_create_prompt(&mut self) {
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.file_operations.create = Some(CreateOverlay {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            preferred_col: 0,
            line_errors: vec![None],
        });
    }
}

impl App {
    pub(crate) fn handle_create_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.create = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.file_operations.create = None;
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
                if let Some(c) = &mut self.file_operations.create {
                    c.cursor_col = 0;
                    c.preferred_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.file_operations.create {
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
                if let Some(c) = &mut self.file_operations.create {
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
}

impl App {
    pub(crate) fn handle_create_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .input
                    .frame_state
                    .create_panel
                    .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
                if !inside {
                    self.file_operations.create = None;
                    return Ok(());
                }
                if let Some(list_area) = self.input.frame_state.create_list_area
                    && list_area.contains((mouse.column, mouse.row).into())
                {
                    let scroll_top = self.input.frame_state.create_scroll_top;
                    let row_offset = (mouse.row - list_area.y) as usize;
                    let line_idx = scroll_top + row_offset;
                    let line_count = self.create_line_count();
                    if line_idx < line_count {
                        let line_len = self.create_line(line_idx).chars().count();
                        let char_col = (mouse.column.saturating_sub(list_area.x + 3)) as usize;
                        let cursor_col = char_col.min(line_len);
                        if let Some(c) = &mut self.file_operations.create {
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

impl App {
    pub(super) fn confirm_create(&mut self) -> Result<()> {
        let Some(c) = &self.file_operations.create else {
            return Ok(());
        };

        let items: Vec<(usize, ParsedCreateItem)> = c
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(index, line)| (index, parse_create_line(line)))
            .collect();

        if items.is_empty() {
            self.file_operations.create = None;
            return Ok(());
        }

        let mut errors: Vec<Option<String>> = self
            .file_operations
            .create
            .as_ref()
            .expect("create overlay should still be present")
            .lines
            .iter()
            .map(|_| None)
            .collect();
        let mut first_error_line: Option<usize> = None;
        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (line_idx, item) in &items {
            let msg = if !seen_names.insert(item.name.clone()) {
                Some(format!("\"{}\" appears more than once", item.name))
            } else {
                validate_parsed_item(item, &self.file_browser.cwd)
            };
            if let Some(msg) = msg {
                errors[*line_idx] = Some(msg);
                if first_error_line.is_none() {
                    first_error_line = Some(*line_idx);
                }
            }
        }

        if let Some(err_line) = first_error_line {
            if let Some(c) = &mut self.file_operations.create {
                c.line_errors = errors;
                c.cursor_line = err_line;
                c.cursor_col = c.cursor_col.min(c.lines[err_line].chars().count());
                c.preferred_col = c.cursor_col;
            }
            return Ok(());
        }

        let mut last_path: Option<std::path::PathBuf> = None;
        for (_, item) in &items {
            let path = self.file_browser.cwd.join(&item.name);
            let result = if item.is_dir {
                fs::create_dir(&path).map_err(anyhow::Error::from)
            } else {
                fs::File::create_new(&path)
                    .map(|_| ())
                    .map_err(anyhow::Error::from)
            };
            if let Err(error) = result {
                let line_idx = items
                    .iter()
                    .find(|(_, candidate)| candidate.raw == item.raw)
                    .map(|(index, _)| *index)
                    .unwrap_or(0);
                let msg = error
                    .downcast_ref::<std::io::Error>()
                    .and_then(|io_error| match io_error.kind() {
                        std::io::ErrorKind::AlreadyExists => {
                            Some(format!("\"{}\" already exists", item.name))
                        }
                        std::io::ErrorKind::PermissionDenied => {
                            Some(format!("\"{}\" — permission denied", item.name))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| error.to_string());
                if let Some(c) = &mut self.file_operations.create {
                    c.line_errors[line_idx] = Some(msg);
                    c.cursor_line = line_idx;
                }
                return Ok(());
            }
            last_path = Some(path);
        }

        self.file_operations.create = None;
        let files = items.iter().filter(|(_, item)| !item.is_dir).count();
        let dirs = items.iter().filter(|(_, item)| item.is_dir).count();
        let status = match (files, dirs) {
            (1, 0) => format!(
                "Created \"{}\"",
                items
                    .iter()
                    .find(|(_, item)| !item.is_dir)
                    .expect("file item should exist")
                    .1
                    .name
            ),
            (0, 1) => format!(
                "Created \"{}\"",
                items
                    .iter()
                    .find(|(_, item)| item.is_dir)
                    .expect("directory item should exist")
                    .1
                    .name
            ),
            (f, 0) => format!("Created {f} files"),
            (0, d) => format!("Created {d} folders"),
            (f, d) => format!(
                "Created {f} file{} and {d} folder{}",
                if f == 1 { "" } else { "s" },
                if d == 1 { "" } else { "s" },
            ),
        };
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: self.file_browser.cwd.clone(),
            previous_cwd: self.file_browser.cwd.clone(),
            previous_selected_path: self.selected_entry().map(|entry| entry.path.clone()),
            previous_selection_name: self.selected_entry().map(|entry| entry.name.clone()),
            reselect_path: last_path,
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(status),
        })?;

        Ok(())
    }
}
