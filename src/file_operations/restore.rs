use super::trash_delete::TrashTarget;
use crate::app::{App, RestoreRequest};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct RestoreProgress {
    pub(crate) completed: usize,
    pub(crate) total: usize,
    pub(crate) next_selection: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub(crate) struct RestoreOverlay {
    pub(crate) targets: Vec<TrashTarget>,
    pub(crate) scroll: usize,
    pub(crate) confirmed: bool,
}

impl App {
    pub(crate) fn open_restore_prompt(&mut self) {
        if !self.file_browser.in_trash {
            return;
        }
        let targets = self.selected_trash_targets();

        if targets.is_empty() {
            return;
        }

        if !self.file_browser.selected_paths.is_empty() {
            let has_trash = targets
                .iter()
                .any(|target| self.trash_target_is_inside_trash(&target.path));
            let has_normal = targets
                .iter()
                .any(|target| !self.trash_target_is_inside_trash(&target.path));
            match (has_trash, has_normal) {
                (true, true) => {
                    self.status = "Selection mixes trash and normal files".to_string();
                    return;
                }
                (false, true) => {
                    self.status = "Cannot restore normal files".to_string();
                    return;
                }
                _ => {}
            }
        }

        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.file_operations.create = None;
        self.file_operations.trash = None;
        self.file_operations.restore = Some(RestoreOverlay {
            targets,
            scroll: 0,
            confirmed: true,
        });
    }

    pub fn restore_is_open(&self) -> bool {
        self.file_operations.restore.is_some()
    }

    /// Returns `(completed, total)` for an in-progress restore, or `None` when idle.
    pub fn restore_progress(&self) -> Option<(usize, usize)> {
        self.file_operations
            .restore_progress
            .as_ref()
            .map(|p| (p.completed, p.total))
    }

    pub fn restore_title(&self) -> String {
        let Some(r) = &self.file_operations.restore else {
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
        self.file_operations
            .restore
            .as_ref()
            .map_or(0, |r| r.scroll)
    }

    pub fn restore_target_count(&self) -> usize {
        self.file_operations
            .restore
            .as_ref()
            .map_or(0, |r| r.targets.len())
    }

    pub fn restore_visible_rows(&self) -> usize {
        self.restore_target_count().min(8)
    }

    pub fn restore_target_name_at(&self, index: usize) -> Option<&str> {
        self.file_operations
            .restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .map(|target| target.name.as_str())
    }

    pub fn restore_target_path_at(&self, index: usize) -> Option<&std::path::Path> {
        self.file_operations
            .restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .map(|target| target.path.as_path())
    }

    pub fn restore_target_is_dir_at(&self, index: usize) -> bool {
        self.file_operations
            .restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .is_some_and(|target| target.is_dir)
    }

    pub fn restore_confirmed(&self) -> bool {
        self.file_operations
            .restore
            .as_ref()
            .is_some_and(|r| r.confirmed)
    }

    pub(crate) fn handle_restore_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.restore = None;
            return Ok(());
        }
        match key.code {
            KeyCode::Esc => {
                self.file_operations.restore = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(r) = &mut self.file_operations.restore {
                    r.scroll = r.scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(r) = &mut self.file_operations.restore {
                    let visible = r.targets.len().min(8);
                    let max_scroll = r.targets.len().saturating_sub(visible);
                    r.scroll = (r.scroll + 1).min(max_scroll);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(r) = &mut self.file_operations.restore {
                    r.confirmed = true;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(r) = &mut self.file_operations.restore {
                    r.confirmed = false;
                }
            }
            KeyCode::Tab => {
                if let Some(r) = &mut self.file_operations.restore {
                    r.confirmed = !r.confirmed;
                }
            }
            KeyCode::Enter => {
                if self
                    .file_operations
                    .restore
                    .as_ref()
                    .is_some_and(|r| r.confirmed)
                {
                    self.confirm_restore()?;
                } else {
                    self.file_operations.restore = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_restore_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .input
                    .screen_regions
                    .restore_panel
                    .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
                if !inside {
                    self.file_operations.restore = None;
                    return Ok(());
                }
                if self
                    .input
                    .screen_regions
                    .restore_confirm_btn
                    .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into()))
                {
                    self.confirm_restore()?;
                } else if self
                    .input
                    .screen_regions
                    .restore_cancel_btn
                    .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into()))
                {
                    self.file_operations.restore = None;
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(r) = &mut self.file_operations.restore {
                    r.scroll = r.scroll.saturating_sub(1);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(r) = &mut self.file_operations.restore {
                    let visible = r.targets.len().min(8);
                    let max_scroll = r.targets.len().saturating_sub(visible);
                    r.scroll = (r.scroll + 1).min(max_scroll);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn confirm_restore(&mut self) -> Result<()> {
        if self.file_operations.restore_progress.is_some() {
            self.status = "Restore in progress — press Esc to cancel".to_string();
            self.file_operations.restore = None;
            return Ok(());
        }
        let Some(r) = self.file_operations.restore.take() else {
            return Ok(());
        };
        if r.targets.is_empty() {
            return Ok(());
        }
        self.file_browser.selected_paths.clear();
        let target_paths: Vec<PathBuf> =
            r.targets.iter().map(|target| target.path.clone()).collect();
        let source_cwd = self.queue_directory_escape_for_paths(&target_paths)?;

        let restored_paths: std::collections::HashSet<_> =
            r.targets.iter().map(|t| &t.path).collect();
        let next_selection = self
            .file_browser
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !restored_paths.contains(&e.path))
            .find(|(i, _)| *i >= self.file_browser.selected)
            .or_else(|| {
                self.file_browser
                    .entries
                    .iter()
                    .enumerate()
                    .rfind(|(_, e)| !restored_paths.contains(&e.path))
            })
            .map(|(_, e)| e.path.clone());

        let token = self.file_operations.restore_token.wrapping_add(1);
        self.file_operations.restore_token = token;
        self.file_operations.restore_progress = Some(RestoreProgress {
            completed: 0,
            total: r.targets.len(),
            next_selection,
        });
        self.file_operations.restore_source_cwd = Some(source_cwd.clone());

        self.job_scheduler.submit_restore(RestoreRequest {
            token,
            targets: r.targets,
        });

        Ok(())
    }
}
