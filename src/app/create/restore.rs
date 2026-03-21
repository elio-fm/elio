use super::super::{
    App,
    state::{DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad, RestoreOverlay},
};
use crate::fs::rect_contains;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub(in crate::app) fn open_restore_prompt(&mut self) {
        if !self.in_trash {
            return;
        }
        let targets = self.selected_trash_targets();

        if targets.is_empty() {
            return;
        }

        self.help_open = false;
        self.search = None;
        self.create = None;
        self.trash = None;
        self.restore = Some(RestoreOverlay {
            targets,
            scroll: 0,
            confirmed: true,
        });
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
                let kind = if r.targets[0].is_dir {
                    "folder"
                } else {
                    "file"
                };
                format!("Restore 1 selected {kind}?")
            }
            _ => {
                let files = r.targets.iter().filter(|target| !target.is_dir).count();
                let dirs = r.targets.iter().filter(|target| target.is_dir).count();
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
            .map(|target| target.name.as_str())
    }

    pub fn restore_target_path_at(&self, index: usize) -> Option<&std::path::Path> {
        self.restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .map(|target| target.path.as_path())
    }

    pub fn restore_target_is_dir_at(&self, index: usize) -> bool {
        self.restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .is_some_and(|target| target.is_dir)
    }

    pub fn restore_confirmed(&self) -> bool {
        self.restore.as_ref().is_some_and(|r| r.confirmed)
    }

    pub(in crate::app) fn handle_restore_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
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
                if self
                    .frame_state
                    .restore_confirm_btn
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
                {
                    self.confirm_restore()?;
                } else if self
                    .frame_state
                    .restore_cancel_btn
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
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

    pub(in crate::app::create) fn confirm_restore(&mut self) -> Result<()> {
        let Some(r) = self.restore.take() else {
            return Ok(());
        };
        let mut restored = 0usize;
        let mut last_error: Option<String> = None;
        for target in &r.targets {
            match crate::fs::restore_trash_item(&target.path) {
                Ok(_) => restored += 1,
                Err(error) => {
                    last_error = Some(format!("Could not restore \"{}\": {error}", target.name));
                }
            }
        }
        self.selected_paths.clear();
        let status = if let Some(error) = last_error {
            if restored == 0 {
                error
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
