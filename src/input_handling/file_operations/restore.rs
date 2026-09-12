use crate::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::collections::HashSet;

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
            match crate::file_operations::trash_target_scope(
                &targets,
                &self.file_browser.cwd,
                self.file_browser.in_trash,
            ) {
                crate::file_operations::TrashTargetScope::Mixed => {
                    self.status = "Selection mixes trash and normal files".to_string();
                    return;
                }
                crate::file_operations::TrashTargetScope::Normal => {
                    self.status = "Cannot restore normal files".to_string();
                    return;
                }
                crate::file_operations::TrashTargetScope::Trash => {}
            }
        }
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.file_operations.open_restore_prompt(targets);
    }

    pub(crate) fn confirm_restore(&mut self) -> Result<()> {
        let overlay = match self.file_operations.take_restore_confirmation() {
            crate::file_operations::RestoreConfirmation::None => return Ok(()),
            crate::file_operations::RestoreConfirmation::InProgress => {
                self.status = "Restore in progress — press Esc to cancel".to_string();
                return Ok(());
            }
            crate::file_operations::RestoreConfirmation::Targets(overlay) => overlay,
        };
        self.file_browser.selected_paths.clear();
        let target_paths = overlay
            .targets
            .iter()
            .map(|target| target.path.clone())
            .collect::<Vec<_>>();
        let source_cwd = self.queue_directory_escape_for_paths(&target_paths)?;
        let restored_paths = overlay
            .targets
            .iter()
            .map(|target| &target.path)
            .collect::<HashSet<_>>();
        let next_selection = self
            .file_browser
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| !restored_paths.contains(&entry.path))
            .find(|(index, _)| *index >= self.file_browser.selected)
            .or_else(|| {
                self.file_browser
                    .entries
                    .iter()
                    .enumerate()
                    .rfind(|(_, entry)| !restored_paths.contains(&entry.path))
            })
            .map(|(_, entry)| entry.path.clone());
        let request = self
            .file_operations
            .start_restore(overlay, source_cwd, next_selection);
        self.job_scheduler.submit_restore(request);
        Ok(())
    }

    pub(crate) fn selected_trash_targets(&self) -> Vec<crate::file_operations::TrashTarget> {
        if !self.file_browser.selected_paths.is_empty() {
            return self
                .selected_paths_sorted()
                .into_iter()
                .map(crate::file_operations::trash_target_from_path)
                .collect();
        }
        self.selected_entry()
            .map(|entry| {
                vec![crate::file_operations::TrashTarget {
                    path: entry.path.clone(),
                    name: entry.name.clone(),
                    is_dir: entry.is_dir(),
                }]
            })
            .unwrap_or_default()
    }

    pub(crate) fn trash_target_is_inside_trash(&self, path: &std::path::Path) -> bool {
        crate::file_operations::trash_target_is_inside_trash(
            path,
            &self.file_browser.cwd,
            self.file_browser.in_trash,
        )
    }

    pub(crate) fn handle_restore_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.dismiss_restore();
            return Ok(());
        }
        match key.code {
            KeyCode::Esc => {
                self.file_operations.dismiss_restore();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(r) = self.file_operations.restore_overlay_mut() {
                    r.scroll = r.scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(r) = self.file_operations.restore_overlay_mut() {
                    let visible = r.targets.len().min(8);
                    let max_scroll = r.targets.len().saturating_sub(visible);
                    r.scroll = (r.scroll + 1).min(max_scroll);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(r) = self.file_operations.restore_overlay_mut() {
                    r.confirmed = true;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(r) = self.file_operations.restore_overlay_mut() {
                    r.confirmed = false;
                }
            }
            KeyCode::Tab => {
                if let Some(r) = self.file_operations.restore_overlay_mut() {
                    r.confirmed = !r.confirmed;
                }
            }
            KeyCode::Enter => {
                if self.file_operations.restore_confirmed() {
                    self.confirm_restore()?;
                } else {
                    self.file_operations.dismiss_restore();
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
                    self.file_operations.dismiss_restore();
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
                    self.file_operations.dismiss_restore();
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(r) = self.file_operations.restore_overlay_mut() {
                    r.scroll = r.scroll.saturating_sub(1);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(r) = self.file_operations.restore_overlay_mut() {
                    let visible = r.targets.len().min(8);
                    let max_scroll = r.targets.len().saturating_sub(visible);
                    r.scroll = (r.scroll + 1).min(max_scroll);
                }
            }
            _ => {}
        }
        Ok(())
    }
}
