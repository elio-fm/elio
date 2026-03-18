use super::*;
use super::text_edit::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::{env, fs, path::Path};

// ---------------------------------------------------------------------------
// Public accessors (used by the UI layer)
// ---------------------------------------------------------------------------

impl App {
    pub fn create_is_open(&self) -> bool {
        self.create.is_some()
    }

    pub fn create_line_count(&self) -> usize {
        self.create.as_ref().map_or(0, |c| c.lines.len())
    }

    pub fn create_line(&self, index: usize) -> &str {
        self.create
            .as_ref()
            .and_then(|c| c.lines.get(index))
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn create_cursor_line(&self) -> usize {
        self.create.as_ref().map_or(0, |c| c.cursor_line)
    }

    pub fn create_cursor_col(&self) -> usize {
        self.create.as_ref().map_or(0, |c| c.cursor_col)
    }

    pub fn create_title(&self) -> String {
        let Some(c) = &self.create else {
            return "Create".to_string();
        };
        let files = c.lines.iter().filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('/') && !t.ends_with('/')
        }).count();
        let dirs = c.lines.iter().filter(|l| {
            let t = l.trim();
            !t.is_empty() && (t.starts_with('/') || t.ends_with('/'))
        }).count();
        match (files, dirs) {
            (0, 0) => "Create".to_string(),
            (f, 0) => format!("Create {} file{}", f, if f == 1 { "" } else { "s" }),
            (0, d) => format!("Create {} folder{}", d, if d == 1 { "" } else { "s" }),
            (f, d) => format!(
                "Create {} file{} and {} folder{}",
                f, if f == 1 { "" } else { "s" },
                d, if d == 1 { "" } else { "s" },
            ),
        }
    }

    pub fn create_line_error(&self, index: usize) -> Option<&str> {
        self.create
            .as_ref()
            .and_then(|c| c.line_errors.get(index))
            .and_then(Option::as_deref)
    }


}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

impl App {
    pub(in crate::app) fn open_create_prompt(&mut self) {
        self.help_open = false;
        self.search = None;
        self.create = Some(CreateOverlay {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            preferred_col: 0,
            line_errors: vec![None],
        });
    }
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

impl App {
    pub(in crate::app) fn handle_create_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c'))
        {
            self.create = None;
            return Ok(());
        }

        match key.code {
            // ---------------------------------------------------------------
            // Overlay lifecycle
            // ---------------------------------------------------------------
            KeyCode::Esc => {
                self.create = None;
            }

            // ---------------------------------------------------------------
            // New line: Alt+Enter or Ctrl+J (must come before plain Enter)
            // ---------------------------------------------------------------
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

            // Confirm (plain Enter only)
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_create()?;
            }

            // ---------------------------------------------------------------
            // Horizontal cursor movement
            // ---------------------------------------------------------------
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
                if let Some(c) = &mut self.create {
                    c.cursor_col = 0;
                    c.preferred_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.create {
                    let len = c.lines[c.cursor_line].chars().count();
                    c.cursor_col = len;
                    c.preferred_col = len;
                }
            }

            // ---------------------------------------------------------------
            // Vertical cursor movement
            // ---------------------------------------------------------------
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => {
                self.create_move_vertical(-1);
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                self.create_move_vertical(1);
            }

            // ---------------------------------------------------------------
            // Deletion
            // ---------------------------------------------------------------
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

            // ---------------------------------------------------------------
            // Character input
            // ---------------------------------------------------------------
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(c) = &mut self.create {
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

// ---------------------------------------------------------------------------
// Mouse handling
// ---------------------------------------------------------------------------

impl App {
    pub(in crate::app) fn handle_create_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .frame_state
                    .create_panel
                    .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
                if !inside {
                    self.create = None;
                    return Ok(());
                }
                // Click inside list area — move cursor to clicked line and column.
                if let Some(list_area) = self.frame_state.create_list_area {
                    if rect_contains(list_area, mouse.column, mouse.row) {
                        let scroll_top = self.frame_state.create_scroll_top;
                        let row_offset = (mouse.row - list_area.y) as usize;
                        let line_idx = scroll_top + row_offset;
                        let line_count = self.create_line_count();
                        if line_idx < line_count {
                            let line_len = self.create_line(line_idx).chars().count();
                            // Text starts after icon (3 cells).
                            let char_col = (mouse.column.saturating_sub(list_area.x + 3)) as usize;
                            let cursor_col = char_col.min(line_len);
                            if let Some(c) = &mut self.create {
                                c.cursor_line = line_idx;
                                c.cursor_col = cursor_col;
                                c.preferred_col = cursor_col;
                            }
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

// ---------------------------------------------------------------------------
// Editing operations (private)
// ---------------------------------------------------------------------------

impl App {
    fn create_insert_newline(&mut self) {
        let Some(c) = &mut self.create else { return };
        let tail = {
            let byte = char_to_byte(&c.lines[c.cursor_line], c.cursor_col);
            c.lines[c.cursor_line].split_off(byte)
        };
        c.cursor_line += 1;
        c.lines.insert(c.cursor_line, tail);
        c.line_errors.insert(c.cursor_line, None);
        c.cursor_col = 0;
        c.preferred_col = 0;
    }

    fn create_move_horizontal(&mut self, delta: isize) {
        let Some(c) = &mut self.create else { return };
        if delta < 0 {
            if c.cursor_col > 0 {
                c.cursor_col -= 1;
            } else if c.cursor_line > 0 {
                c.cursor_line -= 1;
                c.cursor_col = c.lines[c.cursor_line].chars().count();
            }
        } else {
            let len = c.lines[c.cursor_line].chars().count();
            if c.cursor_col < len {
                c.cursor_col += 1;
            } else if c.cursor_line + 1 < c.lines.len() {
                c.cursor_line += 1;
                c.cursor_col = 0;
            }
        }
        c.preferred_col = c.cursor_col;
    }

    fn create_move_word(&mut self, direction: isize) {
        let Some(c) = &mut self.create else { return };
        let line = &c.lines[c.cursor_line];
        let new_col = if direction < 0 {
            previous_word_start(line, c.cursor_col)
        } else {
            next_word_start(line, c.cursor_col)
        };
        c.cursor_col = new_col;
        c.preferred_col = new_col;
    }

    fn create_move_vertical(&mut self, delta: isize) {
        let Some(c) = &mut self.create else { return };
        let new_line = (c.cursor_line as isize + delta)
            .clamp(0, c.lines.len() as isize - 1) as usize;
        if new_line == c.cursor_line {
            return;
        }
        c.cursor_line = new_line;
        // Clamp to the target line length but preserve preferred_col for future moves.
        let max_col = c.lines[c.cursor_line].chars().count();
        c.cursor_col = c.preferred_col.min(max_col);
    }

    fn create_backspace(&mut self) {
        let Some(c) = &mut self.create else { return };
        if c.cursor_col > 0 {
            let start = char_to_byte(&c.lines[c.cursor_line], c.cursor_col - 1);
            let end = char_to_byte(&c.lines[c.cursor_line], c.cursor_col);
            c.lines[c.cursor_line].replace_range(start..end, "");
            c.cursor_col -= 1;
            c.preferred_col = c.cursor_col;
            c.line_errors[c.cursor_line] = None;
        } else if c.cursor_line > 0 {
            // Join with the previous line.
            let removed = c.lines.remove(c.cursor_line);
            c.line_errors.remove(c.cursor_line);
            c.cursor_line -= 1;
            c.cursor_col = c.lines[c.cursor_line].chars().count();
            c.preferred_col = c.cursor_col;
            c.lines[c.cursor_line].push_str(&removed);
            c.line_errors[c.cursor_line] = None;
        }
    }

    fn create_delete(&mut self) {
        let Some(c) = &mut self.create else { return };
        let len = c.lines[c.cursor_line].chars().count();
        if c.cursor_col < len {
            let start = char_to_byte(&c.lines[c.cursor_line], c.cursor_col);
            let end = char_to_byte(&c.lines[c.cursor_line], c.cursor_col + 1);
            c.lines[c.cursor_line].replace_range(start..end, "");
            c.line_errors[c.cursor_line] = None;
        } else if c.cursor_line + 1 < c.lines.len() {
            // Join next line into current.
            let next = c.lines.remove(c.cursor_line + 1);
            c.line_errors.remove(c.cursor_line + 1);
            c.lines[c.cursor_line].push_str(&next);
            c.line_errors[c.cursor_line] = None;
        }
    }

    fn create_delete_word_back(&mut self) {
        let Some(c) = &mut self.create else { return };
        if c.cursor_col == 0 {
            return;
        }
        let line = &mut c.lines[c.cursor_line];
        let start = previous_delete_start(line, c.cursor_col);
        remove_char_range(line, start, c.cursor_col);
        c.cursor_col = start;
        c.preferred_col = start;
        c.line_errors[c.cursor_line] = None;
    }

    fn create_delete_word_forward(&mut self) {
        let Some(c) = &mut self.create else { return };
        let line = &mut c.lines[c.cursor_line];
        let end = next_delete_end(line, c.cursor_col);
        remove_char_range(line, c.cursor_col, end);
        c.line_errors[c.cursor_line] = None;
    }
}

// ---------------------------------------------------------------------------
// Confirm / validate
// ---------------------------------------------------------------------------

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
    fn confirm_create(&mut self) -> Result<()> {
        let Some(c) = &self.create else {
            return Ok(());
        };

        // Skip entirely-blank lines.
        let items: Vec<(usize, ParsedCreateItem)> = c
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
            .map(|(i, l)| (i, parse_create_line(l)))
            .collect();

        if items.is_empty() {
            // All lines blank — treat as cancel.
            self.create = None;
            return Ok(());
        }

        // Validate all items first (including duplicates within the batch).
        let mut errors: Vec<Option<String>> = self
            .create
            .as_ref()
            .unwrap()
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
                validate_parsed_item(item, &self.cwd)
            };
            if let Some(msg) = msg {
                errors[*line_idx] = Some(msg);
                if first_error_line.is_none() {
                    first_error_line = Some(*line_idx);
                }
            }
        }

        if let Some(err_line) = first_error_line {
            if let Some(c) = &mut self.create {
                c.line_errors = errors;
                c.cursor_line = err_line;
                c.cursor_col = c.cursor_col.min(c.lines[err_line].chars().count());
                c.preferred_col = c.cursor_col;
            }
            return Ok(());
        }

        // All valid — create.
        let mut last_path: Option<std::path::PathBuf> = None;
        for (_, item) in &items {
            let path = self.cwd.join(&item.name);
            let result = if item.is_dir {
                fs::create_dir(&path).map_err(anyhow::Error::from)
            } else {
                fs::File::create_new(&path)
                    .map(|_| ())
                    .map_err(anyhow::Error::from)
            };
            if let Err(e) = result {
                // Rare OS error — report it on the relevant line with a clean message.
                let line_idx = items
                    .iter()
                    .find(|(_, i)| i.raw == item.raw)
                    .map(|(idx, _)| *idx)
                    .unwrap_or(0);
                let msg = e
                    .downcast_ref::<std::io::Error>()
                    .and_then(|io| match io.kind() {
                        std::io::ErrorKind::AlreadyExists => {
                            Some(format!("\"{}\" already exists", item.name))
                        }
                        std::io::ErrorKind::PermissionDenied => {
                            Some(format!("\"{}\" — permission denied", item.name))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| e.to_string());
                if let Some(c) = &mut self.create {
                    c.line_errors[line_idx] = Some(msg);
                    c.cursor_line = line_idx;
                }
                return Ok(());
            }
            last_path = Some(path);
        }

        self.create = None;
        let files = items.iter().filter(|(_, i)| !i.is_dir).count();
        let dirs = items.iter().filter(|(_, i)| i.is_dir).count();
        let status = match (files, dirs) {
            (1, 0) => format!("Created \"{}\"", items.iter().find(|(_, i)| !i.is_dir).unwrap().1.name),
            (0, 1) => format!("Created \"{}\"", items.iter().find(|(_, i)| i.is_dir).unwrap().1.name),
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
            target_cwd: self.cwd.clone(),
            previous_cwd: self.cwd.clone(),
            previous_selected_path: self.selected_entry().map(|e| e.path.clone()),
            previous_selection_name: self.selected_entry().map(|e| e.name.clone()),
            reselect_path: last_path,
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(status),
        })?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Trash overlay (kept in this file for historical reasons)
// ---------------------------------------------------------------------------

impl App {
    /// True once the trash directory has fully loaded (set in apply_directory_snapshot).
    pub(in crate::app) fn cwd_is_trash(&self) -> bool {
        self.in_trash
    }

    pub(in crate::app) fn path_is_trash(path: &std::path::Path) -> bool {
        let home = env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/"));
        crate::fs::trash_dir(&home).is_some_and(|trash| path == trash)
    }

    /// Effective show_hidden for the current loaded directory.
    /// Hidden files are always visible inside the trash folder.
    pub(in crate::app) fn effective_show_hidden(&self) -> bool {
        self.show_hidden || self.in_trash
    }

    /// Effective show_hidden for a navigation target (used before the load completes).
    pub(in crate::app) fn effective_show_hidden_for(&self, path: &std::path::Path) -> bool {
        self.show_hidden || Self::path_is_trash(path)
    }
}

impl App {
    pub(in crate::app) fn open_trash_prompt(&mut self) {
        let targets: Vec<TrashTarget> = if !self.selected_paths.is_empty() {
            self.entries
                .iter()
                .filter(|e| self.selected_paths.contains(&e.path))
                .map(|e| TrashTarget {
                    path: e.path.clone(),
                    name: e.name.clone(),
                    is_dir: e.is_dir(),
                })
                .collect()
        } else {
            let Some(entry) = self.selected_entry() else {
                return;
            };
            vec![TrashTarget {
                path: entry.path.clone(),
                name: entry.name.clone(),
                is_dir: entry.is_dir(),
            }]
        };

        if targets.is_empty() {
            return;
        }

        let permanent = self.cwd_is_trash();
        self.help_open = false;
        self.search = None;
        self.create = None;
        self.trash = Some(TrashOverlay { targets, scroll: 0, confirmed: true, permanent });
    }

    pub fn trash_is_open(&self) -> bool {
        self.trash.is_some()
    }

    pub fn trash_title(&self) -> String {
        let Some(t) = &self.trash else {
            return String::new();
        };
        let verb = if t.permanent { "Delete permanently" } else { "Trash" };
        match t.targets.len() {
            0 => String::new(),
            1 => {
                let kind = if t.targets[0].is_dir { "folder" } else { "file" };
                format!("{verb} 1 selected {kind}?")
            }
            _ => {
                let files = t.targets.iter().filter(|t| !t.is_dir).count();
                let dirs = t.targets.iter().filter(|t| t.is_dir).count();
                let desc = match (files, dirs) {
                    (f, 0) => format!("{f} file{}", if f == 1 { "" } else { "s" }),
                    (0, d) => format!("{d} folder{}", if d == 1 { "" } else { "s" }),
                    (f, d) => format!(
                        "{f} file{} and {d} folder{}",
                        if f == 1 { "" } else { "s" },
                        if d == 1 { "" } else { "s" }
                    ),
                };
                format!("{verb} {desc}?")
            }
        }
    }

    pub fn trash_scroll(&self) -> usize {
        self.trash.as_ref().map_or(0, |t| t.scroll)
    }

    pub fn trash_target_count(&self) -> usize {
        self.trash.as_ref().map_or(0, |t| t.targets.len())
    }

    pub fn trash_visible_rows(&self) -> usize {
        self.trash_target_count().min(8)
    }

    pub fn trash_target_name_at(&self, index: usize) -> Option<&str> {
        self.trash
            .as_ref()
            .and_then(|t| t.targets.get(index))
            .map(|t| t.name.as_str())
    }

    pub fn trash_confirmed(&self) -> bool {
        self.trash.as_ref().is_some_and(|t| t.confirmed)
    }

    pub(in crate::app) fn handle_trash_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c'))
        {
            self.trash = None;
            return Ok(());
        }
        match key.code {
            KeyCode::Esc => {
                self.trash = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(t) = &mut self.trash {
                    t.scroll = t.scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(t) = &mut self.trash {
                    let visible = t.targets.len().min(8);
                    let max_scroll = t.targets.len().saturating_sub(visible);
                    t.scroll = (t.scroll + 1).min(max_scroll);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(t) = &mut self.trash {
                    t.confirmed = true;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(t) = &mut self.trash {
                    t.confirmed = false;
                }
            }
            KeyCode::Tab => {
                if let Some(t) = &mut self.trash {
                    t.confirmed = !t.confirmed;
                }
            }
            KeyCode::Enter => {
                if self.trash.as_ref().is_some_and(|t| t.confirmed) {
                    self.confirm_trash()?;
                } else {
                    self.trash = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::app) fn handle_trash_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .frame_state
                    .trash_panel
                    .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
                if !inside {
                    self.trash = None;
                    return Ok(());
                }
                if self.frame_state.trash_confirm_btn
                    .is_some_and(|r| rect_contains(r, mouse.column, mouse.row))
                {
                    self.confirm_trash()?;
                } else if self.frame_state.trash_cancel_btn
                    .is_some_and(|r| rect_contains(r, mouse.column, mouse.row))
                {
                    self.trash = None;
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(t) = &mut self.trash {
                    t.scroll = t.scroll.saturating_sub(1);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(t) = &mut self.trash {
                    let visible = t.targets.len().min(8);
                    let max_scroll = t.targets.len().saturating_sub(visible);
                    t.scroll = (t.scroll + 1).min(max_scroll);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn confirm_trash(&mut self) -> Result<()> {
        let Some(t) = self.trash.take() else {
            return Ok(());
        };
        for target in &t.targets {
            if t.permanent {
                if target.is_dir {
                    fs::remove_dir_all(&target.path)
                        .map_err(|e| anyhow::anyhow!("Could not delete \"{}\": {e}", target.name))?;
                } else {
                    fs::remove_file(&target.path)
                        .map_err(|e| anyhow::anyhow!("Could not delete \"{}\": {e}", target.name))?;
                }
            } else {
                trash::delete(&target.path)
                    .map_err(|e| anyhow::anyhow!("Could not trash \"{}\": {e}", target.name))?;
            }
        }
        self.selected_paths.clear();
        let verb = if t.permanent { "Permanently deleted" } else { "Trashed" };
        let status = match t.targets.len() {
            0 => String::new(),
            1 => format!("{verb} \"{}\"", t.targets[0].name),
            n => format!("{verb} {n} items"),
        };
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: self.cwd.clone(),
            previous_cwd: self.cwd.clone(),
            previous_selected_path: None,
            previous_selection_name: None,
            reselect_path: None,
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(status),
        })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Rename overlay
// ---------------------------------------------------------------------------

impl App {
    pub(in crate::app) fn open_rename_prompt(&mut self) {
        if self.in_trash {
            return;
        }
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let name = entry.name.clone();
        // Place cursor before the extension (e.g. "foo.txt" → col 3).
        // Dot-prefixed hidden files ("." at index 0) are treated as having no extension.
        let cursor_col = cursor_before_extension(&name);
        self.help_open = false;
        self.search = None;
        self.create = None;
        self.trash = None;
        self.restore = None;
        self.rename = Some(RenameOverlay {
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

    pub fn rename_error(&self) -> Option<&str> {
        self.rename.as_ref().and_then(|r| r.error.as_deref())
    }

    pub(in crate::app) fn handle_rename_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c'))
        {
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

            // Horizontal cursor movement
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

            // Deletion
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.rename {
                    if r.cursor_col > 0 {
                        let start = previous_delete_start(&r.input, r.cursor_col);
                        remove_char_range(&mut r.input, start, r.cursor_col);
                        r.cursor_col = start;
                        r.error = None;
                    }
                }
            }
            KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.rename {
                    if r.cursor_col > 0 {
                        let start = previous_delete_start(&r.input, r.cursor_col);
                        remove_char_range(&mut r.input, start, r.cursor_col);
                        r.cursor_col = start;
                        r.error = None;
                    }
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
                if let Some(r) = &mut self.rename {
                    if r.cursor_col > 0 {
                        let start = char_to_byte(&r.input, r.cursor_col - 1);
                        let end = char_to_byte(&r.input, r.cursor_col);
                        r.input.replace_range(start..end, "");
                        r.cursor_col -= 1;
                        r.error = None;
                    }
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

            // Character input
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
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .frame_state
                    .rename_panel
                    .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
                if !inside {
                    self.rename = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn confirm_rename(&mut self) -> Result<()> {
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

        let Some(entry) = self.entries.iter().find(|e| e.name == original_name) else {
            self.rename = None;
            return Ok(());
        };
        let old_path = entry.path.clone();

        if let Err(e) = fs::rename(&old_path, &new_path) {
            let msg = match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    format!("Permission denied renaming \"{}\"", original_name)
                }
                _ => format!("Could not rename: {e}"),
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

/// Returns the char index where the cursor should start in a rename prompt:
/// just before the last extension, skipping dot-prefixed hidden file names.
fn cursor_before_extension(name: &str) -> usize {
    let total = name.chars().count();
    // Find the last '.' that isn't the first character (hidden file dot).
    if let Some(dot_pos) = name.rfind('.') {
        let dot_char = name[..dot_pos].chars().count();
        if dot_char > 0 {
            return dot_char;
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Restore overlay
// ---------------------------------------------------------------------------

impl App {
    pub(in crate::app) fn open_restore_prompt(&mut self) {
        if !self.in_trash {
            return;
        }
        let targets: Vec<TrashTarget> = if !self.selected_paths.is_empty() {
            self.entries
                .iter()
                .filter(|e| self.selected_paths.contains(&e.path))
                .map(|e| TrashTarget {
                    path: e.path.clone(),
                    name: e.name.clone(),
                    is_dir: e.is_dir(),
                })
                .collect()
        } else {
            let Some(entry) = self.selected_entry() else {
                return;
            };
            vec![TrashTarget {
                path: entry.path.clone(),
                name: entry.name.clone(),
                is_dir: entry.is_dir(),
            }]
        };

        if targets.is_empty() {
            return;
        }

        self.help_open = false;
        self.search = None;
        self.create = None;
        self.trash = None;
        self.restore = Some(RestoreOverlay { targets, scroll: 0, confirmed: true });
    }

    pub fn restore_is_open(&self) -> bool {
        self.restore.is_some()
    }

    pub fn restore_title(&self) -> String {
        let Some(r) = &self.restore else {
            return String::new();
        };
        match r.targets.len() {
            0 => String::new(),
            1 => {
                let kind = if r.targets[0].is_dir { "folder" } else { "file" };
                format!("Restore 1 selected {kind}?")
            }
            _ => {
                let files = r.targets.iter().filter(|t| !t.is_dir).count();
                let dirs = r.targets.iter().filter(|t| t.is_dir).count();
                let desc = match (files, dirs) {
                    (f, 0) => format!("{f} file{}", if f == 1 { "" } else { "s" }),
                    (0, d) => format!("{d} folder{}", if d == 1 { "" } else { "s" }),
                    (f, d) => format!(
                        "{f} file{} and {d} folder{}",
                        if f == 1 { "" } else { "s" },
                        if d == 1 { "" } else { "s" }
                    ),
                };
                format!("Restore {desc}?")
            }
        }
    }

    pub fn restore_scroll(&self) -> usize {
        self.restore.as_ref().map_or(0, |r| r.scroll)
    }

    pub fn restore_target_count(&self) -> usize {
        self.restore.as_ref().map_or(0, |r| r.targets.len())
    }

    pub fn restore_visible_rows(&self) -> usize {
        self.restore_target_count().min(8)
    }

    pub fn restore_target_name_at(&self, index: usize) -> Option<&str> {
        self.restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .map(|t| t.name.as_str())
    }

    pub fn restore_confirmed(&self) -> bool {
        self.restore.as_ref().is_some_and(|r| r.confirmed)
    }

    pub(in crate::app) fn handle_restore_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c'))
        {
            self.restore = None;
            return Ok(());
        }
        match key.code {
            KeyCode::Esc => {
                self.restore = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(r) = &mut self.restore {
                    r.scroll = r.scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(r) = &mut self.restore {
                    let visible = r.targets.len().min(8);
                    let max_scroll = r.targets.len().saturating_sub(visible);
                    r.scroll = (r.scroll + 1).min(max_scroll);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(r) = &mut self.restore {
                    r.confirmed = true;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(r) = &mut self.restore {
                    r.confirmed = false;
                }
            }
            KeyCode::Tab => {
                if let Some(r) = &mut self.restore {
                    r.confirmed = !r.confirmed;
                }
            }
            KeyCode::Enter => {
                if self.restore.as_ref().is_some_and(|r| r.confirmed) {
                    self.confirm_restore()?;
                } else {
                    self.restore = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::app) fn handle_restore_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .frame_state
                    .restore_panel
                    .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
                if !inside {
                    self.restore = None;
                    return Ok(());
                }
                if self.frame_state.restore_confirm_btn
                    .is_some_and(|r| rect_contains(r, mouse.column, mouse.row))
                {
                    self.confirm_restore()?;
                } else if self.frame_state.restore_cancel_btn
                    .is_some_and(|r| rect_contains(r, mouse.column, mouse.row))
                {
                    self.restore = None;
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(r) = &mut self.restore {
                    r.scroll = r.scroll.saturating_sub(1);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(r) = &mut self.restore {
                    let visible = r.targets.len().min(8);
                    let max_scroll = r.targets.len().saturating_sub(visible);
                    r.scroll = (r.scroll + 1).min(max_scroll);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn confirm_restore(&mut self) -> Result<()> {
        let Some(r) = self.restore.take() else {
            return Ok(());
        };
        let mut restored = 0usize;
        let mut last_error: Option<String> = None;
        for target in &r.targets {
            match crate::fs::restore_trash_item(&target.path) {
                Ok(_) => restored += 1,
                Err(e) => {
                    last_error = Some(format!("Could not restore \"{}\": {e}", target.name));
                }
            }
        }
        self.selected_paths.clear();
        let status = if let Some(err) = last_error {
            if restored == 0 {
                err
            } else {
                format!("Restored {restored} item(s) with errors")
            }
        } else {
            match r.targets.len() {
                0 => String::new(),
                1 => format!("Restored \"{}\"", r.targets[0].name),
                n => format!("Restored {n} items"),
            }
        };
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: self.cwd.clone(),
            previous_cwd: self.cwd.clone(),
            previous_selected_path: None,
            previous_selection_name: None,
            reselect_path: None,
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(status),
        })?;
        Ok(())
    }
}
