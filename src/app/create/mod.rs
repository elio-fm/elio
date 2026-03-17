use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::fs;

impl App {
    pub fn create_is_open(&self) -> bool {
        self.create.is_some()
    }

    pub fn create_input(&self) -> &str {
        self.create.as_ref().map(|c| c.input.as_str()).unwrap_or("")
    }

    pub fn create_cursor(&self) -> usize {
        self.create.as_ref().map(|c| c.cursor).unwrap_or(0)
    }

    pub fn create_error(&self) -> Option<&str> {
        self.create.as_ref().and_then(|c| c.error.as_deref())
    }

    pub(in crate::app) fn open_create_prompt(&mut self) {
        self.help_open = false;
        self.search = None;
        self.create = Some(CreateOverlay {
            input: String::new(),
            cursor: 0,
            error: None,
        });
    }

    pub(in crate::app) fn handle_create_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c'))
        {
            self.create = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.create = None;
            }
            KeyCode::Enter => {
                self.confirm_create()?;
            }
            KeyCode::Left => {
                if let Some(c) = &mut self.create {
                    c.cursor = c.cursor.saturating_sub(1);
                }
            }
            KeyCode::Right => {
                if let Some(c) = &mut self.create {
                    c.cursor = (c.cursor + 1).min(c.input.chars().count());
                }
            }
            KeyCode::Home => {
                if let Some(c) = &mut self.create {
                    c.cursor = 0;
                }
            }
            KeyCode::End => {
                if let Some(c) = &mut self.create {
                    c.cursor = c.input.chars().count();
                }
            }
            KeyCode::Backspace => {
                if let Some(c) = &mut self.create {
                    if c.cursor > 0 {
                        let start = char_byte_pos(&c.input, c.cursor - 1);
                        let end = char_byte_pos(&c.input, c.cursor);
                        c.input.replace_range(start..end, "");
                        c.cursor -= 1;
                    }
                    c.error = None;
                }
            }
            KeyCode::Delete => {
                if let Some(c) = &mut self.create {
                    let start = char_byte_pos(&c.input, c.cursor);
                    if start < c.input.len() {
                        let end = char_byte_pos(&c.input, c.cursor + 1);
                        c.input.replace_range(start..end, "");
                    }
                    c.error = None;
                }
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(c) = &mut self.create {
                    let byte_pos = char_byte_pos(&c.input, c.cursor);
                    c.input.insert(byte_pos, ch);
                    c.cursor += 1;
                    c.error = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(in crate::app) fn handle_create_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            let inside = self
                .frame_state
                .create_panel
                .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
            if !inside {
                self.create = None;
            }
        }
        Ok(())
    }

    fn confirm_create(&mut self) -> Result<()> {
        let input = match &self.create {
            Some(c) => c.input.clone(),
            None => return Ok(()),
        };

        let is_dir = input.starts_with('/') || input.ends_with('/');
        let clean = input.trim_matches('/').to_string();

        if clean.is_empty() {
            if let Some(c) = &mut self.create {
                c.error = Some("Name cannot be empty".to_string());
            }
            return Ok(());
        }

        if clean.contains('/') {
            if let Some(c) = &mut self.create {
                c.error = Some("Name cannot contain /".to_string());
            }
            return Ok(());
        }

        let path = self.cwd.join(&clean);

        if path.exists() {
            if let Some(c) = &mut self.create {
                c.error = Some(format!("\"{clean}\" already exists"));
            }
            return Ok(());
        }

        let result = if is_dir {
            fs::create_dir(&path).map_err(anyhow::Error::from)
        } else {
            fs::File::create_new(&path)
                .map(|_| ())
                .map_err(anyhow::Error::from)
        };

        match result {
            Ok(()) => {
                self.create = None;
                self.queue_directory_load(PendingDirectoryLoad {
                    token: 0,
                    target_cwd: self.cwd.clone(),
                    previous_cwd: self.cwd.clone(),
                    previous_selected_path: self.selected_entry().map(|e| e.path.clone()),
                    previous_selection_name: self.selected_entry().map(|e| e.name.clone()),
                    reselect_path: Some(path),
                    history_mode: DirectoryHistoryMode::None,
                    refresh_search: false,
                    completion: DirectoryLoadCompletion::Clear,
                })?;
            }
            Err(e) => {
                if let Some(c) = &mut self.create {
                    c.error = Some(e.to_string());
                }
            }
        }

        Ok(())
    }

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
                    (f, d) => format!("{f} file{} and {d} folder{}", if f == 1 { "" } else { "s" }, if d == 1 { "" } else { "s" }),
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

fn char_byte_pos(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}
