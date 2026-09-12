use crate::app::*;
use crate::app::{ArchiveExtractBatchState, ArchiveExtractRequest};
use crate::archive::{ArchiveEncryption, ArchivePassword};
use crate::input_handling::text_editing::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::fmt;

#[derive(Clone, Debug)]
pub(crate) struct ArchiveExtractProgress {
    pub(crate) completed: usize,
    pub(crate) total: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) enum ArchivePasswordPurpose {
    Extract { request: ArchiveExtractRequest },
    Create,
}

#[derive(Clone)]
pub(crate) struct ArchivePasswordOverlay {
    pub(crate) purpose: ArchivePasswordPurpose,
    pub(crate) input: String,
    pub(crate) cursor_col: usize,
    pub(crate) visible: bool,
    pub(crate) error: Option<String>,
}

impl fmt::Debug for ArchivePasswordOverlay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArchivePasswordOverlay")
            .field("purpose", &self.purpose)
            .field("input", &"<redacted>")
            .field("cursor_col", &self.cursor_col)
            .field("visible", &self.visible)
            .field("error", &self.error)
            .finish()
    }
}

impl App {
    pub fn archive_extract_progress(&self) -> Option<(usize, Option<usize>)> {
        self.file_operations
            .archive_extract_progress
            .as_ref()
            .map(|progress| (progress.completed, progress.total))
    }

    pub(crate) fn extract_focused_archive(&mut self) -> Result<()> {
        if self.file_operations.archive_extract_progress.is_some() {
            self.status = "Extraction already in progress".to_string();
            return Ok(());
        }

        if self.selection_count() > 0 {
            return self.extract_selected_archives();
        }

        let Some(entry) = self.selected_entry() else {
            self.status = "Select an archive to extract".to_string();
            return Ok(());
        };
        if entry.is_dir() {
            self.status = "Select an archive to extract".to_string();
            return Ok(());
        }

        let archive_path = entry.path.clone();
        let request = ArchiveExtractRequest {
            token: 0,
            archives: vec![archive_path.clone()],
            password: None,
            batch: ArchiveExtractBatchState::new(1, 0),
        };
        if let Err(error) = crate::archive::plan_extract(&archive_path) {
            self.status = error.to_string();
            return Ok(());
        }
        let _ = self.start_archive_extract(request)?;
        Ok(())
    }

    fn extract_selected_archives(&mut self) -> Result<()> {
        let selected = self.selected_paths_sorted();
        let mut archives = Vec::new();
        let mut skipped_non_archives = 0usize;
        for path in selected {
            if path.is_file() && crate::archive::plan_extract(&path).is_ok() {
                archives.push(path);
            } else {
                skipped_non_archives += 1;
            }
        }

        if archives.is_empty() {
            self.status = "No archives selected".to_string();
            return Ok(());
        }

        let request = ArchiveExtractRequest {
            token: 0,
            batch: ArchiveExtractBatchState::new(archives.len(), skipped_non_archives),
            archives,
            password: None,
        };
        if self.start_archive_extract(request)? {
            self.file_browser.selected_paths.clear();
        }
        Ok(())
    }

    pub fn archive_password_is_open(&self) -> bool {
        self.file_operations.archive_password.is_some()
    }

    pub fn archive_password_archive_name(&self) -> String {
        let Some(overlay) = &self.file_operations.archive_password else {
            return "archive".to_string();
        };
        match &overlay.purpose {
            ArchivePasswordPurpose::Extract { request } => request
                .archives
                .first()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("archive")
                .to_string(),
            ArchivePasswordPurpose::Create => self.archive_create_input().to_string(),
        }
    }

    pub fn archive_password_title_prefix(&self) -> &'static str {
        let Some(overlay) = &self.file_operations.archive_password else {
            return "Password for";
        };
        match overlay.purpose {
            ArchivePasswordPurpose::Extract { .. } => "Password for",
            ArchivePasswordPurpose::Create
                if self
                    .file_operations
                    .archive_create
                    .as_ref()
                    .is_some_and(|create| create.options.encryption.is_password_set()) =>
            {
                "Change password for"
            }
            ArchivePasswordPurpose::Create => "New password for",
        }
    }

    pub fn archive_password_placeholder(&self) -> &'static str {
        let Some(overlay) = &self.file_operations.archive_password else {
            return "password…";
        };
        match overlay.purpose {
            ArchivePasswordPurpose::Extract { .. } => "password…",
            ArchivePasswordPurpose::Create => "new password…",
        }
    }

    pub fn archive_password_input(&self) -> &str {
        self.file_operations
            .archive_password
            .as_ref()
            .map_or("", |overlay| &overlay.input)
    }

    pub fn archive_password_cursor_col(&self) -> usize {
        self.file_operations
            .archive_password
            .as_ref()
            .map_or(0, |overlay| overlay.cursor_col)
    }

    pub fn archive_password_error(&self) -> Option<&str> {
        self.file_operations
            .archive_password
            .as_ref()
            .and_then(|overlay| overlay.error.as_deref())
    }

    pub(crate) fn open_archive_password_prompt(
        &mut self,
        request: ArchiveExtractRequest,
        error: Option<String>,
    ) {
        self.overlays.help = false;
        self.file_operations.trash = None;
        self.file_operations.restore = None;
        self.file_operations.create = None;
        self.file_operations.rename = None;
        self.file_operations.bulk_rename = None;
        self.overlays.goto = None;
        self.file_operations.copy = None;
        self.overlays.open_with = None;
        self.fuzzy_finder.search = None;
        self.file_operations.archive_password = Some(ArchivePasswordOverlay {
            purpose: ArchivePasswordPurpose::Extract { request },
            input: String::new(),
            cursor_col: 0,
            visible: false,
            error,
        });
        self.status.clear();
    }

    pub fn archive_password_is_visible(&self) -> bool {
        self.file_operations
            .archive_password
            .as_ref()
            .is_some_and(|overlay| overlay.visible)
    }

    pub(crate) fn toggle_archive_password_visibility(&mut self) {
        if let Some(overlay) = &mut self.file_operations.archive_password {
            overlay.visible = !overlay.visible;
        }
    }

    pub(crate) fn handle_archive_password_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.cancel_archive_password_prompt()?;
            return Ok(());
        }

        if key.modifiers == KeyModifiers::ALT && matches!(key.code, KeyCode::Char('v' | 'V')) {
            self.toggle_archive_password_visibility();
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
                if let Some(overlay) = &mut self.file_operations.archive_password {
                    overlay.cursor_col = previous_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = &mut self.file_operations.archive_password {
                    overlay.cursor_col = next_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_password {
                    overlay.cursor_col = overlay.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_password {
                    let len = overlay.input.chars().count();
                    if overlay.cursor_col < len {
                        overlay.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_password {
                    overlay.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_password {
                    overlay.cursor_col = overlay.input.chars().count();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = &mut self.file_operations.archive_password
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
                if let Some(overlay) = &mut self.file_operations.archive_password
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
                if let Some(overlay) = &mut self.file_operations.archive_password {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(overlay) = &mut self.file_operations.archive_password {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_password
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
                if let Some(overlay) = &mut self.file_operations.archive_password {
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
                if let Some(overlay) = &mut self.file_operations.archive_password {
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
                self.toggle_archive_password_visibility();
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

    fn confirm_archive_password(&mut self) -> Result<()> {
        let Some(overlay) = &self.file_operations.archive_password else {
            return Ok(());
        };
        let password = overlay.input.clone();
        if password.is_empty() {
            if let Some(overlay) = &mut self.file_operations.archive_password {
                overlay.error = Some("Password cannot be empty".to_string());
            }
            return Ok(());
        }

        match &overlay.purpose {
            ArchivePasswordPurpose::Extract { request } => {
                let mut request = request.clone();
                request.password = Some(ArchivePassword::new(password));
                if self.start_archive_extract(request)? {
                    self.file_operations.archive_password = None;
                }
            }
            ArchivePasswordPurpose::Create => {
                if let Some(overlay) = &mut self.file_operations.archive_create {
                    overlay.options.encryption =
                        ArchiveEncryption::Password(ArchivePassword::new(password));
                    overlay.error = None;
                    self.status.clear();
                }
                self.file_operations.archive_password = None;
            }
        }
        Ok(())
    }

    fn cancel_archive_password_prompt(&mut self) -> Result<()> {
        let Some(overlay) = self.file_operations.archive_password.take() else {
            return Ok(());
        };
        if let ArchivePasswordPurpose::Extract { request } = overlay.purpose {
            self.skip_password_archive(request)?;
        }
        Ok(())
    }

    fn skip_password_archive(&mut self, mut request: ArchiveExtractRequest) -> Result<()> {
        if !request.archives.is_empty() {
            request.archives.remove(0);
            request.batch.skipped_archives += 1;
        }
        request.password = None;
        if request.archives.is_empty() {
            self.finish_archive_extract_batch(request.batch);
            return Ok(());
        }
        let _ = self.start_archive_extract(request)?;
        Ok(())
    }

    pub(crate) fn finish_archive_extract_batch(&mut self, batch: ArchiveExtractBatchState) {
        let status = batch.status();
        let dest_dir = batch.reselect_path();
        let source_cwd = self
            .file_operations
            .archive_extract_source_cwd
            .take()
            .unwrap_or_else(|| self.file_browser.cwd.clone());
        self.file_operations.archive_extract_request = None;
        let nav_target = self
            .file_browser
            .directory_runtime
            .pending_load
            .as_ref()
            .map(|l| l.target_cwd.as_path());
        let nav_to_source = nav_target == Some(source_cwd.as_path());
        if nav_to_source || (source_cwd == self.file_browser.cwd && nav_target.is_none()) {
            let _ = self.queue_directory_load(PendingDirectoryLoad {
                token: 0,
                target_cwd: source_cwd,
                previous_cwd: self.file_browser.cwd.clone(),
                previous_selected_path: None,
                previous_selection_name: None,
                reselect_path: dest_dir,
                history_mode: DirectoryHistoryMode::None,
                refresh_search: false,
                completion: DirectoryLoadCompletion::Status(status),
            });
        } else {
            self.status = status;
        }
    }

    fn start_archive_extract(&mut self, mut request: ArchiveExtractRequest) -> Result<bool> {
        if self.file_operations.archive_extract_progress.is_some() {
            self.status = "Extraction already in progress".to_string();
            return Ok(false);
        }

        let token = self.file_operations.archive_extract_token.wrapping_add(1);
        self.file_operations.archive_extract_token = token;
        request.token = token;
        self.file_operations.archive_extract_progress = Some(ArchiveExtractProgress {
            completed: request.batch.finished_archives(),
            total: Some(request.batch.total_archives),
        });
        if self.file_operations.archive_extract_source_cwd.is_none() {
            self.file_operations.archive_extract_source_cwd = Some(self.file_browser.cwd.clone());
        }
        self.file_operations.archive_extract_request = Some(request.clone());
        self.status.clear();

        let submitted = self.job_scheduler.submit_archive_extract(request);
        if !submitted {
            self.file_operations.archive_extract_progress = None;
            self.file_operations.archive_extract_source_cwd = None;
            self.file_operations.archive_extract_request = None;
            self.status = "Extraction already in progress".to_string();
            return Ok(false);
        }
        Ok(true)
    }
}
