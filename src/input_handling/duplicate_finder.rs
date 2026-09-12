use super::*;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(crate) fn handle_duplicate_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            if self.duplicate_loading() {
                self.stop_duplicate_scan_with_partial_results();
                return Ok(());
            }
            self.clear_duplicate_selection_or_close();
            return Ok(());
        }
        if is_duplicate_help_shortcut(key) {
            self.clear_wheel_scroll();
            self.overlays.help_scroll = 0;
            self.overlays.help = true;
            return Ok(());
        }
        if let Some(action) = crate::config::key_bindings().action_for_key(key) {
            use crate::config::Action;
            match action {
                Action::Quit | Action::QuitWithoutCd => {
                    self.dispatch_action(action)?;
                    return Ok(());
                }
                Action::Open => {
                    self.open_duplicate_targets()?;
                    return Ok(());
                }
                Action::OpenWith => {
                    self.open_duplicate_open_with_overlay();
                    return Ok(());
                }
                Action::OpenOrEnter => {
                    self.reveal_duplicate_focus()?;
                    return Ok(());
                }
                Action::CopyPath => {
                    self.open_copy_overlay_for_paths(self.duplicate_action_paths());
                    return Ok(());
                }
                Action::Rename => {
                    self.open_duplicate_rename();
                    return Ok(());
                }
                Action::RenameInEditor => {
                    self.open_duplicate_editor_bulk_rename()?;
                    return Ok(());
                }
                Action::Trash => {
                    self.open_duplicate_trash_prompt();
                    return Ok(());
                }
                Action::DeletePermanently => {
                    self.open_duplicate_delete_permanently_prompt();
                    return Ok(());
                }
                Action::SelectAll => {
                    self.select_all_duplicates();
                    return Ok(());
                }
                Action::ToggleSelection => {
                    self.toggle_duplicate_selection();
                    return Ok(());
                }
                Action::FindDuplicates => {
                    self.close_duplicate_finder();
                    return Ok(());
                }
                Action::NavUp => {
                    self.move_duplicate_selection(-1);
                    return Ok(());
                }
                Action::NavDown => {
                    self.move_duplicate_selection(1);
                    return Ok(());
                }
                Action::PageUp => {
                    self.page_duplicate_selection(-1);
                    return Ok(());
                }
                Action::PageDown => {
                    self.page_duplicate_selection(1);
                    return Ok(());
                }
                Action::JumpFirst => {
                    self.set_duplicate_selection(0);
                    return Ok(());
                }
                Action::JumpLast => {
                    let last = self.duplicate_file_count().saturating_sub(1);
                    self.set_duplicate_selection(last);
                    return Ok(());
                }
                Action::TogglePreview => {
                    self.toggle_duplicate_preview();
                    return Ok(());
                }
                Action::ScrollPreviewUp => {
                    self.scroll_preview_lines(-1);
                    return Ok(());
                }
                Action::ScrollPreviewDown => {
                    self.scroll_preview_lines(1);
                    return Ok(());
                }
                Action::ScrollPreviewLeft => {
                    self.scroll_preview_columns(-1);
                    return Ok(());
                }
                Action::ScrollPreviewRight => {
                    self.scroll_preview_columns(1);
                    return Ok(());
                }
                _ => {}
            }
        }
        if key.modifiers == KeyModifiers::NONE && matches!(key.code, KeyCode::Esc) {
            if self.duplicate_loading() {
                self.stop_duplicate_scan_with_partial_results();
                return Ok(());
            }
            self.clear_duplicate_selection_or_close();
        }
        Ok(())
    }

    pub(crate) fn move_duplicate_selection(&mut self, delta: isize) {
        if self.duplicate_file_count() == 0 {
            return;
        }
        if let Some(duplicates) = &mut self.duplicate_finder.session {
            duplicates.move_selection(delta);
        }
        self.sync_duplicate_scroll();
        self.refresh_duplicate_preview();
    }
    pub(crate) fn page_duplicate_selection(&mut self, direction: isize) {
        let visible = self.input.screen_regions.duplicate_rows_visible.max(1) as isize;
        self.move_duplicate_selection(direction * visible);
    }
    pub(crate) fn set_duplicate_selection(&mut self, index: usize) {
        if let Some(duplicates) = &mut self.duplicate_finder.session {
            duplicates.set_selection(index);
        }
        self.sync_duplicate_scroll();
        self.refresh_duplicate_preview();
    }
    pub(crate) fn sync_duplicate_scroll(&mut self) -> bool {
        let rows_visible = self.input.screen_regions.duplicate_rows_visible;
        self.duplicate_finder
            .session
            .as_mut()
            .is_some_and(|duplicates| duplicates.sync_scroll(rows_visible))
    }
    pub(crate) fn toggle_duplicate_selection(&mut self) {
        let changed =
            self.duplicate_finder.session.as_mut().is_some_and(
                crate::duplicate_finder::DuplicateFinderSession::toggle_focused_selection,
            );
        if changed {
            self.status.clear();
            self.move_duplicate_selection(1);
        }
    }
    pub(crate) fn select_all_duplicates(&mut self) {
        if let Some(duplicates) = &mut self.duplicate_finder.session {
            duplicates.select_all();
            self.status.clear();
        }
    }
    pub(crate) fn clear_duplicate_selection_or_close(&mut self) {
        if self
            .duplicate_finder
            .session
            .as_mut()
            .is_some_and(crate::duplicate_finder::DuplicateFinderSession::clear_selection)
        {
            self.status.clear();
            return;
        }
        self.close_duplicate_finder();
    }
    pub(crate) fn reveal_duplicate_focus(&mut self) -> Result<()> {
        let Some(path) = self.duplicate_focused_path() else {
            return Ok(());
        };
        self.close_duplicate_finder();
        self.reveal_path(path)?;
        Ok(())
    }

    pub(crate) fn handle_duplicate_mouse(
        &mut self,
        mouse: crossterm::event::MouseEvent,
    ) -> Result<()> {
        use crossterm::event::{MouseButton, MouseEventKind};
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hit) = self
                    .input
                    .screen_regions
                    .duplicate_hits
                    .iter()
                    .find(|hit| hit.rect.contains((mouse.column, mouse.row).into()))
                    .cloned()
                {
                    self.set_duplicate_selection(hit.index);
                    if let Some(path) = self.duplicate_focused_path() {
                        if self.is_double_click(&path) {
                            self.reveal_duplicate_focus()?;
                        }
                        self.input.last_click = Some(ClickState {
                            path,
                            at: std::time::Instant::now(),
                        });
                    }
                } else if self
                    .input
                    .screen_regions
                    .duplicate_panel
                    .is_none_or(|rect| !rect.contains((mouse.column, mouse.row).into()))
                {
                    self.close_duplicate_finder();
                }
            }
            MouseEventKind::ScrollDown => {
                if self
                    .input
                    .screen_regions
                    .preview_panel
                    .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into()))
                {
                    self.scroll_preview_lines(1);
                } else {
                    self.move_duplicate_selection(1);
                }
            }
            MouseEventKind::ScrollUp => {
                if self
                    .input
                    .screen_regions
                    .preview_panel
                    .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into()))
                {
                    self.scroll_preview_lines(-1);
                } else {
                    self.move_duplicate_selection(-1);
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn toggle_duplicate_preview(&mut self) {
        self.queue_terminal_image_geometry_clear();
        let mut hidden = false;
        if let Some(overlay) = &mut self.duplicate_finder.session {
            overlay.preview_visible = !overlay.preview_visible;
            hidden = !overlay.preview_visible;
            if hidden {
                overlay.preview_path = None;
            }
        }
        if hidden {
            self.clear_image_preview_selection_activation();
        }
        self.refresh_duplicate_preview();
    }
}

fn is_duplicate_help_shortcut(key: KeyEvent) -> bool {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return false;
    }
    matches!(key.code, KeyCode::Char('?'))
        || matches!(key.code, KeyCode::Char('/')) && key.modifiers.contains(KeyModifiers::SHIFT)
}
