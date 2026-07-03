use super::*;
use crate::app::text_edit::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use crate::archive::{ArchiveEncryption, ArchivePassword};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::path::PathBuf;

impl App {
    pub fn archive_extract_progress(&self) -> Option<(usize, Option<usize>)> {
        self.jobs
            .archive_extract_progress
            .as_ref()
            .map(|progress| (progress.completed, progress.total))
    }

    pub(in crate::app) fn extract_focused_archive(&mut self) -> Result<()> {
        if self.jobs.archive_extract_progress.is_some() {
            self.status = "Extraction already in progress".to_string();
            return Ok(());
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
        let _ = self.start_archive_extract(archive_path, None)?;
        Ok(())
    }

    pub fn archive_password_is_open(&self) -> bool {
        self.overlays.archive_password.is_some()
    }

    pub fn archive_password_archive_name(&self) -> String {
        let Some(overlay) = &self.overlays.archive_password else {
            return "archive".to_string();
        };
        match &overlay.purpose {
            ArchivePasswordPurpose::Extract { archive_path } => archive_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("archive")
                .to_string(),
            ArchivePasswordPurpose::Create => self.archive_create_input().to_string(),
        }
    }

    pub fn archive_password_title_prefix(&self) -> &'static str {
        let Some(overlay) = &self.overlays.archive_password else {
            return "Password for";
        };
        match overlay.purpose {
            ArchivePasswordPurpose::Extract { .. } => "Password for",
            ArchivePasswordPurpose::Create
                if self
                    .overlays
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
        let Some(overlay) = &self.overlays.archive_password else {
            return "password…";
        };
        match overlay.purpose {
            ArchivePasswordPurpose::Extract { .. } => "password…",
            ArchivePasswordPurpose::Create => "new password…",
        }
    }

    pub fn archive_password_input(&self) -> &str {
        self.overlays
            .archive_password
            .as_ref()
            .map_or("", |overlay| &overlay.input)
    }

    pub fn archive_password_cursor_col(&self) -> usize {
        self.overlays
            .archive_password
            .as_ref()
            .map_or(0, |overlay| overlay.cursor_col)
    }

    pub fn archive_password_error(&self) -> Option<&str> {
        self.overlays
            .archive_password
            .as_ref()
            .and_then(|overlay| overlay.error.as_deref())
    }

    pub(in crate::app) fn open_archive_password_prompt(
        &mut self,
        archive_path: PathBuf,
        error: Option<String>,
    ) {
        self.overlays.help = false;
        self.overlays.trash = None;
        self.overlays.restore = None;
        self.overlays.create = None;
        self.overlays.rename = None;
        self.overlays.bulk_rename = None;
        self.overlays.goto = None;
        self.overlays.copy = None;
        self.overlays.open_with = None;
        self.overlays.search = None;
        self.overlays.archive_password = Some(ArchivePasswordOverlay {
            purpose: ArchivePasswordPurpose::Extract { archive_path },
            input: String::new(),
            cursor_col: 0,
            visible: false,
            error,
        });
        self.status.clear();
    }

    pub fn archive_password_is_visible(&self) -> bool {
        self.overlays
            .archive_password
            .as_ref()
            .is_some_and(|overlay| overlay.visible)
    }

    pub(in crate::app) fn toggle_archive_password_visibility(&mut self) {
        if let Some(overlay) = &mut self.overlays.archive_password {
            overlay.visible = !overlay.visible;
        }
    }

    pub(in crate::app) fn handle_archive_password_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.archive_password = None;
            return Ok(());
        }

        if key.modifiers == KeyModifiers::ALT && matches!(key.code, KeyCode::Char('v' | 'V')) {
            self.toggle_archive_password_visibility();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.overlays.archive_password = None;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_archive_password()?;
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = &mut self.overlays.archive_password {
                    overlay.cursor_col = previous_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = &mut self.overlays.archive_password {
                    overlay.cursor_col = next_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_password {
                    overlay.cursor_col = overlay.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_password {
                    let len = overlay.input.chars().count();
                    if overlay.cursor_col < len {
                        overlay.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_password {
                    overlay.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_password {
                    overlay.cursor_col = overlay.input.chars().count();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = &mut self.overlays.archive_password
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
                if let Some(overlay) = &mut self.overlays.archive_password
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
                if let Some(overlay) = &mut self.overlays.archive_password {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(overlay) = &mut self.overlays.archive_password {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_password
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
                if let Some(overlay) = &mut self.overlays.archive_password {
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
                if let Some(overlay) = &mut self.overlays.archive_password {
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

    pub(in crate::app) fn handle_archive_password_mouse(
        &mut self,
        mouse: MouseEvent,
    ) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            if self
                .input
                .frame_state
                .archive_password_visibility_btn
                .is_some_and(|btn| rect_contains(btn, mouse.column, mouse.row))
            {
                self.toggle_archive_password_visibility();
                return Ok(());
            }

            let inside = self
                .input
                .frame_state
                .archive_password_panel
                .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
            if !inside {
                self.overlays.archive_password = None;
            }
        }
        Ok(())
    }

    fn confirm_archive_password(&mut self) -> Result<()> {
        let Some(overlay) = &self.overlays.archive_password else {
            return Ok(());
        };
        let password = overlay.input.clone();
        if password.is_empty() {
            if let Some(overlay) = &mut self.overlays.archive_password {
                overlay.error = Some("Password cannot be empty".to_string());
            }
            return Ok(());
        }

        match &overlay.purpose {
            ArchivePasswordPurpose::Extract { archive_path } => {
                let archive_path = archive_path.clone();
                if self.start_archive_extract(archive_path, Some(ArchivePassword::new(password)))? {
                    self.overlays.archive_password = None;
                }
            }
            ArchivePasswordPurpose::Create => {
                if let Some(overlay) = &mut self.overlays.archive_create {
                    overlay.options.encryption =
                        ArchiveEncryption::Password(ArchivePassword::new(password));
                    overlay.error = None;
                    self.status.clear();
                }
                self.overlays.archive_password = None;
            }
        }
        Ok(())
    }

    fn start_archive_extract(
        &mut self,
        archive_path: PathBuf,
        password: Option<ArchivePassword>,
    ) -> Result<bool> {
        if self.jobs.archive_extract_progress.is_some() {
            self.status = "Extraction already in progress".to_string();
            return Ok(false);
        }

        if let Err(error) = crate::archive::plan_extract(&archive_path) {
            self.status = error.to_string();
            return Ok(false);
        }

        let token = self.jobs.archive_extract_token.wrapping_add(1);
        self.jobs.archive_extract_token = token;
        self.jobs.archive_extract_progress = Some(ArchiveExtractProgress {
            completed: 0,
            total: None,
        });
        self.jobs.archive_extract_source_cwd = Some(self.navigation.cwd.clone());
        self.jobs.archive_extract_path = Some(archive_path.clone());
        self.status.clear();

        let submitted = self
            .jobs
            .scheduler
            .submit_archive_extract(ArchiveExtractRequest {
                token,
                archive_path,
                password,
            });
        if !submitted {
            self.jobs.archive_extract_progress = None;
            self.jobs.archive_extract_source_cwd = None;
            self.jobs.archive_extract_path = None;
            self.status = "Extraction already in progress".to_string();
            return Ok(false);
        }
        Ok(true)
    }
}
