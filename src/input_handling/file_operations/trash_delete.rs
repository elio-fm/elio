use crate::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::collections::HashSet;

impl App {
    pub(crate) fn open_trash_prompt(&mut self) {
        let targets = self.selected_trash_targets();
        if targets.is_empty() {
            return;
        }
        match crate::file_operations::trash_target_scope(
            &targets,
            &self.file_browser.cwd,
            self.file_browser.in_trash,
        ) {
            crate::file_operations::TrashTargetScope::Normal => {
                self.open_trash_prompt_for_targets(targets, false, true)
            }
            crate::file_operations::TrashTargetScope::Trash => {
                self.open_trash_prompt_for_targets(targets, true, true)
            }
            crate::file_operations::TrashTargetScope::Mixed => {
                self.status = "Selection mixes trash and normal files".to_string();
            }
        }
    }

    pub(crate) fn open_delete_permanently_prompt(&mut self) {
        let targets = self.selected_trash_targets();
        if !targets.is_empty() {
            self.open_trash_prompt_for_targets(targets, true, true);
        }
    }

    pub(crate) fn open_trash_prompt_for_explicit_targets(
        &mut self,
        targets: Vec<crate::file_operations::TrashTarget>,
        permanent: bool,
    ) {
        self.open_trash_prompt_for_targets(targets, permanent, false);
    }

    fn open_trash_prompt_for_targets(
        &mut self,
        targets: Vec<crate::file_operations::TrashTarget>,
        permanent: bool,
        close_duplicates: bool,
    ) {
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        if close_duplicates {
            self.duplicate_finder.session = None;
        }
        self.file_operations.open_trash_prompt(targets, permanent);
    }

    pub(crate) fn confirm_trash(&mut self) -> Result<()> {
        let overlay = match self.file_operations.take_trash_confirmation() {
            crate::file_operations::TrashConfirmation::None => return Ok(()),
            crate::file_operations::TrashConfirmation::InProgress { permanent } => {
                self.status = if permanent {
                    "Delete in progress — press Esc to cancel".to_string()
                } else {
                    "Trash in progress".to_string()
                };
                return Ok(());
            }
            crate::file_operations::TrashConfirmation::Targets(overlay) => overlay,
        };
        self.file_browser.selected_paths.clear();
        let target_paths = overlay
            .targets
            .iter()
            .map(|target| target.path.clone())
            .collect::<Vec<_>>();
        let source_cwd = self.queue_directory_escape_for_paths(&target_paths)?;
        let deleted_paths = overlay
            .targets
            .iter()
            .map(|target| &target.path)
            .collect::<HashSet<_>>();
        let next_selection = self
            .file_browser
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| !deleted_paths.contains(&entry.path))
            .find(|(index, _)| *index >= self.file_browser.selected)
            .or_else(|| {
                self.file_browser
                    .entries
                    .iter()
                    .enumerate()
                    .rfind(|(_, entry)| !deleted_paths.contains(&entry.path))
            })
            .map(|(_, entry)| entry.path.clone());
        #[cfg(unix)]
        if !overlay.permanent && crate::file_operations::likely_cross_device_trash(&overlay.targets)
        {
            self.status = "Copying to trash…".to_string();
        }
        let request = self.file_operations.start_trash(
            overlay,
            source_cwd,
            next_selection,
            self.duplicate_finder.session.is_some(),
        );
        self.job_scheduler.submit_trash(request);
        Ok(())
    }

    pub(crate) fn handle_trash_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.trash = None;
            return Ok(());
        }
        match key.code {
            KeyCode::Esc => {
                self.file_operations.trash = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(t) = &mut self.file_operations.trash {
                    t.scroll = t.scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(t) = &mut self.file_operations.trash {
                    let visible = t.targets.len().min(8);
                    let max_scroll = t.targets.len().saturating_sub(visible);
                    t.scroll = (t.scroll + 1).min(max_scroll);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(t) = &mut self.file_operations.trash {
                    t.confirmed = true;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(t) = &mut self.file_operations.trash {
                    t.confirmed = false;
                }
            }
            KeyCode::Tab => {
                if let Some(t) = &mut self.file_operations.trash {
                    t.confirmed = !t.confirmed;
                }
            }
            KeyCode::Enter => {
                if self
                    .file_operations
                    .trash
                    .as_ref()
                    .is_some_and(|t| t.confirmed)
                {
                    self.confirm_trash()?;
                } else {
                    self.file_operations.trash = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_trash_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .input
                    .screen_regions
                    .trash_panel
                    .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
                if !inside {
                    self.file_operations.trash = None;
                    return Ok(());
                }
                if self
                    .input
                    .screen_regions
                    .trash_confirm_btn
                    .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into()))
                {
                    self.confirm_trash()?;
                } else if self
                    .input
                    .screen_regions
                    .trash_cancel_btn
                    .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into()))
                {
                    self.file_operations.trash = None;
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(t) = &mut self.file_operations.trash {
                    t.scroll = t.scroll.saturating_sub(1);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(t) = &mut self.file_operations.trash {
                    let visible = t.targets.len().min(8);
                    let max_scroll = t.targets.len().saturating_sub(visible);
                    t.scroll = (t.scroll + 1).min(max_scroll);
                }
            }
            _ => {}
        }
        Ok(())
    }
}
