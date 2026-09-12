use super::super::text_editing::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use crate::app::App;
use crate::file_browser::{DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::path::PathBuf;

impl App {
    pub(crate) fn extract_focused_archive(&mut self) -> Result<()> {
        let selection_active = self.selection_count() > 0;
        let focused_is_dir =
            !selection_active && self.selected_entry().is_some_and(|entry| entry.is_dir());
        let paths = if selection_active {
            self.selected_paths_sorted()
        } else {
            self.selected_entry()
                .map(|entry| vec![entry.path.clone()])
                .unwrap_or_default()
        };
        match self
            .file_operations
            .prepare_archive_extract(paths, selection_active, focused_is_dir)
        {
            crate::file_operations::ArchiveExtractPreparation::Ready(request) => {
                if self.start_archive_extract(request) && selection_active {
                    self.file_browser.selected_paths.clear();
                }
            }
            crate::file_operations::ArchiveExtractPreparation::Rejected(status) => {
                self.status = status;
            }
        }
        Ok(())
    }

    fn start_archive_extract(
        &mut self,
        request: crate::file_operations::ArchiveExtractRequest,
    ) -> bool {
        let Some(request) = self
            .file_operations
            .start_archive_extract(request, self.file_browser.cwd.clone())
        else {
            self.status = "Extraction already in progress".to_string();
            return false;
        };
        self.status.clear();
        if self.job_scheduler.submit_archive_extract(request) {
            true
        } else {
            self.file_operations.reject_archive_extract_submission();
            self.status = "Extraction already in progress".to_string();
            false
        }
    }

    pub(crate) fn open_archive_password_prompt(
        &mut self,
        request: crate::file_operations::ArchiveExtractRequest,
        error: Option<String>,
    ) {
        self.overlays.help = false;
        self.overlays.goto = None;
        self.overlays.open_with = None;
        self.fuzzy_finder.search = None;
        self.file_operations
            .open_archive_password_prompt(request, error);
        self.status.clear();
    }

    pub(crate) fn confirm_archive_password(&mut self) -> Result<()> {
        match self.file_operations.confirm_archive_password() {
            crate::file_operations::ArchivePasswordConfirmation::None => {}
            crate::file_operations::ArchivePasswordConfirmation::Create { applied } => {
                if applied {
                    self.status.clear();
                }
            }
            crate::file_operations::ArchivePasswordConfirmation::Extract(request) => {
                if self.start_archive_extract(request) {
                    self.file_operations.accept_archive_password();
                }
            }
        }
        Ok(())
    }

    pub(crate) fn cancel_archive_password_prompt(&mut self) -> Result<()> {
        match self.file_operations.cancel_archive_password_prompt() {
            crate::file_operations::ArchivePasswordCancellation::None => {}
            crate::file_operations::ArchivePasswordCancellation::Continue(request) => {
                self.start_archive_extract(request);
            }
            crate::file_operations::ArchivePasswordCancellation::Finished(batch) => {
                self.finish_archive_extract(batch.status(), batch.reselect_path());
            }
        }
        Ok(())
    }

    pub(crate) fn finish_archive_extract(
        &mut self,
        status: String,
        reselect_path: Option<PathBuf>,
    ) {
        let completion = self.file_operations.finish_archive_extract(
            self.file_browser.cwd.clone(),
            status,
            reselect_path,
        );
        let nav_target = self
            .file_browser
            .directory_runtime
            .pending_load
            .as_ref()
            .map(|load| load.target_cwd.as_path());
        let nav_to_source = nav_target == Some(completion.source_cwd.as_path());
        if nav_to_source || (completion.source_cwd == self.file_browser.cwd && nav_target.is_none())
        {
            let _ = self.queue_directory_load(PendingDirectoryLoad {
                token: 0,
                target_cwd: completion.source_cwd,
                previous_cwd: self.file_browser.cwd.clone(),
                previous_selected_path: None,
                previous_selection_name: None,
                reselect_path: completion.reselect_path,
                history_mode: DirectoryHistoryMode::None,
                refresh_search: false,
                completion: DirectoryLoadCompletion::Status(completion.status),
            });
        } else {
            self.status = completion.status;
        }
    }

    pub(crate) fn open_archive_create_prompt(&mut self) {
        if self.file_operations.archive_creation_in_progress() {
            self.status = "Archive creation already in progress".to_string();
            return;
        }
        let (sources, default_name) = if self.file_browser.selected_paths.is_empty() {
            let Some(entry) = self.selected_entry() else {
                self.status = "Select items to archive".to_string();
                return;
            };
            (vec![entry.path.clone()], format!("{}.zip", entry.name))
        } else {
            (self.selected_paths_sorted(), "archive.zip".to_string())
        };
        self.overlays.help = false;
        self.overlays.goto = None;
        self.overlays.open_with = None;
        self.fuzzy_finder.search = None;
        self.file_operations
            .open_archive_create_prompt(sources, default_name);
        self.status.clear();
    }

    fn confirm_archive_create(&mut self) {
        let Some(request) = self
            .file_operations
            .confirm_archive_create(&self.file_browser.cwd)
        else {
            return;
        };
        if self.job_scheduler.submit_archive_create(request) {
            self.file_operations.accept_archive_create_submission();
            self.file_browser.clear_selection();
            self.status.clear();
        } else {
            self.file_operations.reject_archive_create_submission();
            self.status = "Archive creation already in progress".to_string();
        }
    }

    fn open_archive_create_password_prompt(&mut self) {
        self.file_operations.open_archive_create_password_prompt();
    }

    fn remove_archive_create_password(&mut self) {
        if self.file_operations.remove_archive_create_password() {
            self.status = "Archive password removed".to_string();
        }
    }

    pub(crate) fn handle_archive_create_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.dismiss_archive_create();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.file_operations.dismiss_archive_create(),
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => self.confirm_archive_create(),
            KeyCode::Char('p' | 'P')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.open_archive_create_password_prompt();
            }
            KeyCode::Char('r' | 'R')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.remove_archive_create_password();
            }
            KeyCode::PageUp if key.modifiers == KeyModifiers::NONE => {
                self.scroll_archive_create_sources_by(-8, 8);
            }
            KeyCode::PageDown if key.modifiers == KeyModifiers::NONE => {
                self.scroll_archive_create_sources_by(8, 8);
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    overlay.cursor_col = previous_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    overlay.cursor_col = next_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    overlay.cursor_col = overlay.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    let len = overlay.input.chars().count();
                    if overlay.cursor_col < len {
                        overlay.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    overlay.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    overlay.cursor_col = overlay.input.chars().count();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    let start = previous_delete_start(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, start, overlay.cursor_col);
                    overlay.cursor_col = start;
                    overlay.error = None;
                }
            }
            KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    let start = previous_delete_start(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, start, overlay.cursor_col);
                    overlay.cursor_col = start;
                    overlay.error = None;
                }
            }
            KeyCode::Delete
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut()
                    && overlay.cursor_col > 0
                {
                    let start = char_to_byte(&overlay.input, overlay.cursor_col - 1);
                    let end = char_to_byte(&overlay.input, overlay.cursor_col);
                    overlay.input.replace_range(start..end, "");
                    overlay.cursor_col -= 1;
                    overlay.error = None;
                }
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    let len = overlay.input.chars().count();
                    if overlay.cursor_col < len {
                        let start = char_to_byte(&overlay.input, overlay.cursor_col);
                        let end = char_to_byte(&overlay.input, overlay.cursor_col + 1);
                        overlay.input.replace_range(start..end, "");
                        overlay.error = None;
                    }
                }
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_create_overlay_mut() {
                    let byte = char_to_byte(&overlay.input, overlay.cursor_col);
                    overlay.input.insert(byte, ch);
                    overlay.cursor_col += 1;
                    overlay.error = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_archive_create_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::ScrollDown
                if self.archive_create_mouse_in_list(mouse.column, mouse.row) =>
            {
                self.scroll_archive_create_sources_by(3, self.archive_create_visible_rows());
            }
            MouseEventKind::ScrollUp
                if self.archive_create_mouse_in_list(mouse.column, mouse.row) =>
            {
                self.scroll_archive_create_sources_by(-3, self.archive_create_visible_rows());
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .input
                    .screen_regions
                    .archive_create_panel
                    .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
                if !inside {
                    self.file_operations.dismiss_archive_create();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn archive_create_mouse_in_list(&self, column: u16, row: u16) -> bool {
        self.input
            .screen_regions
            .archive_create_list_area
            .or(self.input.screen_regions.archive_create_panel)
            .is_some_and(|area| area.contains((column, row).into()))
    }

    fn archive_create_visible_rows(&self) -> usize {
        self.input
            .screen_regions
            .archive_create_list_area
            .map_or(8, |area| area.height as usize)
            .max(1)
    }

    fn scroll_archive_create_sources_by(&mut self, delta: isize, visible_rows: usize) {
        let Some(overlay) = self.file_operations.archive_create_overlay_mut() else {
            return;
        };
        let max_scroll = overlay
            .source_names
            .len()
            .saturating_sub(visible_rows.max(1));
        overlay.source_scroll = overlay
            .source_scroll
            .saturating_add_signed(delta)
            .min(max_scroll);
    }

    pub(crate) fn handle_archive_password_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.cancel_archive_password_prompt()?;
            return Ok(());
        }

        if key.modifiers == KeyModifiers::ALT && matches!(key.code, KeyCode::Char('v' | 'V')) {
            self.file_operations.toggle_archive_password_visibility();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.cancel_archive_password_prompt()?;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_archive_password()?;
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    overlay.cursor_col = previous_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    overlay.cursor_col = next_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    overlay.cursor_col = overlay.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    let len = overlay.input.chars().count();
                    if overlay.cursor_col < len {
                        overlay.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    overlay.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    overlay.cursor_col = overlay.input.chars().count();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut()
                    && overlay.cursor_col > 0
                {
                    let start = previous_delete_start(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, start, overlay.cursor_col);
                    overlay.cursor_col = start;
                    overlay.error = None;
                }
            }
            KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut()
                    && overlay.cursor_col > 0
                {
                    let start = previous_delete_start(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, start, overlay.cursor_col);
                    overlay.cursor_col = start;
                    overlay.error = None;
                }
            }
            KeyCode::Delete
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut()
                    && overlay.cursor_col > 0
                {
                    let start = char_to_byte(&overlay.input, overlay.cursor_col - 1);
                    let end = char_to_byte(&overlay.input, overlay.cursor_col);
                    overlay.input.replace_range(start..end, "");
                    overlay.cursor_col -= 1;
                    overlay.error = None;
                }
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    let len = overlay.input.chars().count();
                    if overlay.cursor_col < len {
                        let start = char_to_byte(&overlay.input, overlay.cursor_col);
                        let end = char_to_byte(&overlay.input, overlay.cursor_col + 1);
                        overlay.input.replace_range(start..end, "");
                        overlay.error = None;
                    }
                }
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(overlay) = self.file_operations.archive_password_overlay_mut() {
                    let byte = char_to_byte(&overlay.input, overlay.cursor_col);
                    overlay.input.insert(byte, ch);
                    overlay.cursor_col += 1;
                    overlay.error = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_archive_password_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            if self
                .input
                .screen_regions
                .archive_password_visibility_btn
                .is_some_and(|btn| btn.contains((mouse.column, mouse.row).into()))
            {
                self.file_operations.toggle_archive_password_visibility();
                return Ok(());
            }

            let inside = self
                .input
                .screen_regions
                .archive_password_panel
                .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
            if !inside {
                self.cancel_archive_password_prompt()?;
            }
        }
        Ok(())
    }
}
