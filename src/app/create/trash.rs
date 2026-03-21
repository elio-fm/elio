use super::super::{
    App,
    state::{
        DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad, TrashOverlay,
        TrashTarget,
    },
};
use crate::fs::rect_contains;
use ::trash::delete;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::{env, fs, path::Path};

impl App {
    pub(in crate::app) fn cwd_is_trash(&self) -> bool {
        self.in_trash
    }

    pub(in crate::app) fn path_is_trash(path: &Path) -> bool {
        let home = env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/"));
        crate::fs::trash_dir(&home).is_some_and(|trash| path == trash)
    }

    pub(in crate::app) fn effective_show_hidden(&self) -> bool {
        self.show_hidden || self.in_trash
    }

    pub(in crate::app) fn effective_show_hidden_for(&self, path: &Path) -> bool {
        self.show_hidden || Self::path_is_trash(path)
    }
}

impl App {
    pub(in crate::app::create) fn selected_trash_targets(&self) -> Vec<TrashTarget> {
        if !self.selected_paths.is_empty() {
            self.entries
                .iter()
                .filter(|entry| self.selected_paths.contains(&entry.path))
                .map(|entry| TrashTarget {
                    path: entry.path.clone(),
                    name: entry.name.clone(),
                    is_dir: entry.is_dir(),
                })
                .collect()
        } else {
            self.selected_entry()
                .map(|entry| {
                    vec![TrashTarget {
                        path: entry.path.clone(),
                        name: entry.name.clone(),
                        is_dir: entry.is_dir(),
                    }]
                })
                .unwrap_or_default()
        }
    }

    pub(in crate::app) fn open_trash_prompt(&mut self) {
        let targets = self.selected_trash_targets();

        if targets.is_empty() {
            return;
        }

        let permanent = self.cwd_is_trash();
        self.help_open = false;
        self.search = None;
        self.create = None;
        self.trash = Some(TrashOverlay {
            targets,
            scroll: 0,
            confirmed: true,
            permanent,
        });
    }

    pub fn trash_is_open(&self) -> bool {
        self.trash.is_some()
    }

    pub fn trash_title(&self) -> String {
        let Some(t) = &self.trash else {
            return String::new();
        };
        let verb = if t.permanent {
            "Delete permanently"
        } else {
            "Trash"
        };
        match t.targets.len() {
            0 => String::new(),
            1 => {
                let kind = if t.targets[0].is_dir {
                    "folder"
                } else {
                    "file"
                };
                format!("{verb} 1 selected {kind}?")
            }
            _ => {
                let files = t.targets.iter().filter(|target| !target.is_dir).count();
                let dirs = t.targets.iter().filter(|target| target.is_dir).count();
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
            .map(|target| target.name.as_str())
    }

    pub fn trash_target_path_at(&self, index: usize) -> Option<&std::path::Path> {
        self.trash
            .as_ref()
            .and_then(|t| t.targets.get(index))
            .map(|target| target.path.as_path())
    }

    pub fn trash_target_is_dir_at(&self, index: usize) -> bool {
        self.trash
            .as_ref()
            .and_then(|t| t.targets.get(index))
            .is_some_and(|target| target.is_dir)
    }

    pub fn trash_confirmed(&self) -> bool {
        self.trash.as_ref().is_some_and(|t| t.confirmed)
    }

    pub(in crate::app) fn handle_trash_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
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
                if self
                    .frame_state
                    .trash_confirm_btn
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
                {
                    self.confirm_trash()?;
                } else if self
                    .frame_state
                    .trash_cancel_btn
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
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

    pub(in crate::app::create) fn confirm_trash(&mut self) -> Result<()> {
        let Some(t) = self.trash.take() else {
            return Ok(());
        };
        for target in &t.targets {
            if t.permanent {
                if target.is_dir {
                    fs::remove_dir_all(&target.path).map_err(|error| {
                        anyhow::anyhow!("Could not delete \"{}\": {error}", target.name)
                    })?;
                } else {
                    fs::remove_file(&target.path).map_err(|error| {
                        anyhow::anyhow!("Could not delete \"{}\": {error}", target.name)
                    })?;
                }
            } else {
                delete(&target.path).map_err(|error| {
                    anyhow::anyhow!("Could not trash \"{}\": {error}", target.name)
                })?;
            }
        }
        self.selected_paths.clear();
        let verb = if t.permanent {
            "Permanently deleted"
        } else {
            "Trashed"
        };
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
