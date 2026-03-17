use super::*;
use super::text_edit::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::{fs, path::Path};

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
                if key.modifiers.contains(KeyModifiers::ALT)
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

        self.help_open = false;
        self.search = None;
        self.create = None;
        self.trash = Some(TrashOverlay { targets, scroll: 0, confirmed: true });
    }

    pub fn trash_is_open(&self) -> bool {
        self.trash.is_some()
    }

    pub fn trash_title(&self) -> String {
        let Some(t) = &self.trash else {
            return String::new();
        };
        match t.targets.len() {
            0 => String::new(),
            1 => {
                let kind = if t.targets[0].is_dir { "folder" } else { "file" };
                format!("Trash 1 selected {kind}?")
            }
            n => {
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
                let _ = n;
                format!("Trash {desc}?")
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
                    t.confirmed = false;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(t) = &mut self.trash {
                    t.confirmed = true;
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
            trash::delete(&target.path)
                .map_err(|e| anyhow::anyhow!("Could not trash \"{}\": {e}", target.name))?;
        }
        self.selected_paths.clear();
        let status = match t.targets.len() {
            0 => String::new(),
            1 => format!("Trashed \"{}\"", t.targets[0].name),
            n => format!("Trashed {n} items"),
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
