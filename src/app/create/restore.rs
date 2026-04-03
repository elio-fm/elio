use super::super::{
    App,
    jobs::RestoreRequest,
    state::{RestoreOverlay, RestoreProgress},
};
use crate::fs::rect_contains;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub(in crate::app) fn open_restore_prompt(&mut self) {
        if !self.navigation.in_trash {
            return;
        }
        let targets = self.selected_trash_targets();

        if targets.is_empty() {
            return;
        }

        self.overlays.help = false;
        self.overlays.search = None;
        self.overlays.create = None;
        self.overlays.trash = None;
        self.overlays.restore = Some(RestoreOverlay {
            targets,
            scroll: 0,
            confirmed: true,
        });
    }

    pub fn restore_is_open(&self) -> bool {
        self.overlays.restore.is_some()
    }

    /// Returns `(completed, total)` for an in-progress restore, or `None` when idle.
    pub fn restore_progress(&self) -> Option<(usize, usize)> {
        self.jobs
            .restore_progress
            .as_ref()
            .map(|p| (p.completed, p.total))
    }

    pub fn restore_title(&self) -> String {
        let Some(r) = &self.overlays.restore else {
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
        self.overlays.restore.as_ref().map_or(0, |r| r.scroll)
    }

    pub fn restore_target_count(&self) -> usize {
        self.overlays
            .restore
            .as_ref()
            .map_or(0, |r| r.targets.len())
    }

    pub fn restore_visible_rows(&self) -> usize {
        self.restore_target_count().min(8)
    }

    pub fn restore_target_name_at(&self, index: usize) -> Option<&str> {
        self.overlays
            .restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .map(|target| target.name.as_str())
    }

    pub fn restore_target_path_at(&self, index: usize) -> Option<&std::path::Path> {
        self.overlays
            .restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .map(|target| target.path.as_path())
    }

    pub fn restore_target_is_dir_at(&self, index: usize) -> bool {
        self.overlays
            .restore
            .as_ref()
            .and_then(|r| r.targets.get(index))
            .is_some_and(|target| target.is_dir)
    }

    pub fn restore_confirmed(&self) -> bool {
        self.overlays.restore.as_ref().is_some_and(|r| r.confirmed)
    }

    pub(in crate::app) fn handle_restore_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.restore = None;
            return Ok(());
        }
        match key.code {
            KeyCode::Esc => {
                self.overlays.restore = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(r) = &mut self.overlays.restore {
                    r.scroll = r.scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(r) = &mut self.overlays.restore {
                    let visible = r.targets.len().min(8);
                    let max_scroll = r.targets.len().saturating_sub(visible);
                    r.scroll = (r.scroll + 1).min(max_scroll);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(r) = &mut self.overlays.restore {
                    r.confirmed = true;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(r) = &mut self.overlays.restore {
                    r.confirmed = false;
                }
            }
            KeyCode::Tab => {
                if let Some(r) = &mut self.overlays.restore {
                    r.confirmed = !r.confirmed;
                }
            }
            KeyCode::Enter => {
                if self.overlays.restore.as_ref().is_some_and(|r| r.confirmed) {
                    self.confirm_restore()?;
                } else {
                    self.overlays.restore = None;
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
                    .input
                    .frame_state
                    .restore_panel
                    .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
                if !inside {
                    self.overlays.restore = None;
                    return Ok(());
                }
                if self
                    .input
                    .frame_state
                    .restore_confirm_btn
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
                {
                    self.confirm_restore()?;
                } else if self
                    .input
                    .frame_state
                    .restore_cancel_btn
                    .is_some_and(|rect| rect_contains(rect, mouse.column, mouse.row))
                {
                    self.overlays.restore = None;
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(r) = &mut self.overlays.restore {
                    r.scroll = r.scroll.saturating_sub(1);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(r) = &mut self.overlays.restore {
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
        if self.jobs.restore_progress.is_some() {
            self.status = "Restore in progress — press Esc to cancel".to_string();
            self.overlays.restore = None;
            return Ok(());
        }
        let Some(r) = self.overlays.restore.take() else {
            return Ok(());
        };
        if r.targets.is_empty() {
            return Ok(());
        }
        self.navigation.selected_paths.clear();

        let token = self.jobs.restore_token.wrapping_add(1);
        self.jobs.restore_token = token;
        self.jobs.restore_progress = Some(RestoreProgress {
            completed: 0,
            total: r.targets.len(),
        });
        self.jobs.restore_source_cwd = Some(self.navigation.cwd.clone());

        self.jobs.scheduler.submit_restore(RestoreRequest {
            token,
            targets: r.targets,
        });

        Ok(())
    }
}
