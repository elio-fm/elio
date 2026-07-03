use super::*;
use crate::app::jobs::ArchiveCreateRequest;
use crate::app::text_edit::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use crate::archive::{ArchiveEncryption, CreateArchiveOptions, normalize_zip_output_name};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::path::PathBuf;

impl App {
    pub fn archive_create_progress(&self) -> Option<(usize, usize)> {
        self.jobs
            .archive_create_progress
            .as_ref()
            .map(|progress| (progress.completed, progress.total))
    }

    pub fn archive_create_is_open(&self) -> bool {
        self.overlays.archive_create.is_some()
    }

    pub fn archive_create_input(&self) -> &str {
        self.overlays
            .archive_create
            .as_ref()
            .map_or("", |overlay| overlay.input.as_str())
    }

    pub fn archive_create_cursor_col(&self) -> usize {
        self.overlays
            .archive_create
            .as_ref()
            .map_or(0, |overlay| overlay.cursor_col)
    }

    pub fn archive_create_error(&self) -> Option<&str> {
        self.overlays
            .archive_create
            .as_ref()
            .and_then(|overlay| overlay.error.as_deref())
    }

    pub fn archive_create_protection_label(&self) -> &'static str {
        let Some(overlay) = &self.overlays.archive_create else {
            return "";
        };
        if overlay.options.format.supports_encryption()
            && overlay.options.encryption.is_password_set()
        {
            "Password set"
        } else {
            ""
        }
    }

    pub fn archive_create_protection_hint(&self) -> &'static str {
        let Some(overlay) = &self.overlays.archive_create else {
            return "";
        };
        if !overlay.options.format.supports_encryption() {
            ""
        } else if overlay.options.encryption.is_password_set() {
            "Alt+P change  Alt+R remove"
        } else {
            "Alt+P add password"
        }
    }

    pub fn archive_create_source_names(&self) -> &[String] {
        self.overlays
            .archive_create
            .as_ref()
            .map_or(&[], |overlay| overlay.source_names.as_slice())
    }

    pub fn archive_create_title(&self) -> String {
        let Some(overlay) = &self.overlays.archive_create else {
            return "Create archive".to_string();
        };
        let files = overlay
            .source_names
            .iter()
            .filter(|name| !name.ends_with('/'))
            .count();
        let dirs = overlay.source_names.len().saturating_sub(files);
        match (files, dirs) {
            (1, 0) => "Create archive from 1 file".to_string(),
            (0, 1) => "Create archive from 1 folder".to_string(),
            (f, 0) => format!("Create archive from {f} files"),
            (0, d) => format!("Create archive from {d} folders"),
            (f, d) => format!(
                "Create archive from {f} file{} and {d} folder{}",
                if f == 1 { "" } else { "s" },
                if d == 1 { "" } else { "s" },
            ),
        }
    }

    pub(in crate::app) fn open_archive_create_prompt(&mut self) {
        if self.jobs.archive_create_progress.is_some() {
            self.status = "Archive creation already in progress".to_string();
            return;
        }
        let Some((sources, names, default_name)) = self.archive_create_targets() else {
            self.status = "Select items to archive".to_string();
            return;
        };
        self.overlays.help = false;
        self.overlays.trash = None;
        self.overlays.restore = None;
        self.overlays.archive_password = None;
        self.overlays.create = None;
        self.overlays.rename = None;
        self.overlays.bulk_rename = None;
        self.overlays.goto = None;
        self.overlays.copy = None;
        self.overlays.open_with = None;
        self.overlays.search = None;
        let cursor_col = archive_create_default_cursor_col(&default_name);
        self.overlays.archive_create = Some(ArchiveCreateOverlay {
            sources,
            source_names: names,
            source_scroll: 0,
            cursor_col,
            input: default_name,
            options: CreateArchiveOptions::default(),
            error: None,
        });
        self.status.clear();
    }

    fn archive_create_targets(&self) -> Option<(Vec<PathBuf>, Vec<String>, String)> {
        if !self.navigation.selected_paths.is_empty() {
            let sources = self.selected_paths_sorted();
            let names = sources
                .iter()
                .map(|path| archive_source_label(path))
                .collect::<Vec<_>>();
            return Some((sources, names, "archive.zip".to_string()));
        }
        let entry = self.selected_entry()?;
        let name = entry.name.clone();
        let default = format!("{name}.zip");
        Some((
            vec![entry.path.clone()],
            vec![archive_source_label(&entry.path)],
            default,
        ))
    }

    pub(in crate::app) fn handle_archive_create_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.overlays.archive_create = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.overlays.archive_create = None,
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_archive_create()?
            }
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
                if let Some(overlay) = &mut self.overlays.archive_create {
                    overlay.cursor_col = previous_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = &mut self.overlays.archive_create {
                    overlay.cursor_col = next_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_create {
                    overlay.cursor_col = overlay.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_create {
                    let len = overlay.input.chars().count();
                    if overlay.cursor_col < len {
                        overlay.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_create {
                    overlay.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_create {
                    overlay.cursor_col = overlay.input.chars().count();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = &mut self.overlays.archive_create {
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
                if let Some(overlay) = &mut self.overlays.archive_create {
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
                if let Some(overlay) = &mut self.overlays.archive_create {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(overlay) = &mut self.overlays.archive_create {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.overlays.archive_create
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
                if let Some(overlay) = &mut self.overlays.archive_create {
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
                if let Some(overlay) = &mut self.overlays.archive_create {
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

    pub(in crate::app) fn handle_archive_create_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
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
                    .frame_state
                    .archive_create_panel
                    .is_some_and(|panel| rect_contains(panel, mouse.column, mouse.row));
                if !inside {
                    self.overlays.archive_create = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn archive_create_mouse_in_list(&self, column: u16, row: u16) -> bool {
        self.input
            .frame_state
            .archive_create_list_area
            .or(self.input.frame_state.archive_create_panel)
            .is_some_and(|area| rect_contains(area, column, row))
    }

    fn archive_create_visible_rows(&self) -> usize {
        self.input
            .frame_state
            .archive_create_list_area
            .map_or(8, |area| area.height as usize)
            .max(1)
    }

    pub fn archive_create_source_scroll(&self, visible_rows: usize) -> usize {
        self.overlays.archive_create.as_ref().map_or(0, |overlay| {
            overlay
                .source_scroll
                .min(overlay.source_names.len().saturating_sub(visible_rows))
        })
    }

    fn scroll_archive_create_sources_by(&mut self, delta: isize, visible_rows: usize) {
        let Some(overlay) = &mut self.overlays.archive_create else {
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

    fn confirm_archive_create(&mut self) -> Result<()> {
        let Some(overlay) = &self.overlays.archive_create else {
            return Ok(());
        };
        let output_name = match normalize_zip_output_name(&overlay.input) {
            Ok(name) => name,
            Err(error) => {
                if let Some(overlay) = &mut self.overlays.archive_create {
                    overlay.error = Some(error.to_string());
                }
                return Ok(());
            }
        };
        let sources = overlay.sources.clone();
        let options = overlay.options.clone();
        if self.start_archive_create(sources, output_name, options)? {
            self.overlays.archive_create = None;
        }
        Ok(())
    }

    fn start_archive_create(
        &mut self,
        sources: Vec<PathBuf>,
        output_name: String,
        options: CreateArchiveOptions,
    ) -> Result<bool> {
        if self.jobs.archive_create_progress.is_some() {
            self.status = "Archive creation already in progress".to_string();
            return Ok(false);
        }
        if let Err(error) = crate::archive::plan_create_archive(
            &self.navigation.cwd,
            sources.clone(),
            &output_name,
            options.clone(),
        ) {
            if let Some(overlay) = &mut self.overlays.archive_create {
                overlay.error = Some(error.to_string());
            } else {
                self.status = error.to_string();
            }
            return Ok(false);
        }

        let token = self.jobs.archive_create_token.wrapping_add(1);
        self.jobs.archive_create_token = token;
        self.jobs.archive_create_progress = Some(ArchiveCreateProgress {
            completed: 0,
            total: 0,
        });
        self.jobs.archive_create_source_cwd = Some(self.navigation.cwd.clone());
        self.jobs.archive_create_path = Some(self.navigation.cwd.join(&output_name));
        self.status.clear();

        let submitted = self
            .jobs
            .scheduler
            .submit_archive_create(ArchiveCreateRequest {
                token,
                cwd: self.navigation.cwd.clone(),
                sources,
                output_name,
                options,
            });
        if !submitted {
            self.jobs.archive_create_progress = None;
            self.jobs.archive_create_source_cwd = None;
            self.jobs.archive_create_path = None;
            self.status = "Archive creation already in progress".to_string();
            return Ok(false);
        }
        self.clear_selection();
        Ok(true)
    }

    fn open_archive_create_password_prompt(&mut self) {
        let Some(overlay) = &self.overlays.archive_create else {
            return;
        };
        if !overlay.options.format.supports_encryption() {
            if let Some(overlay) = &mut self.overlays.archive_create {
                overlay.error = Some(format!(
                    "{} does not support password protection",
                    overlay.options.format.label()
                ));
            }
            return;
        }
        let input = match &overlay.options.encryption {
            ArchiveEncryption::Password(password) => password.as_str().to_string(),
            ArchiveEncryption::None => String::new(),
        };
        let cursor_col = input.chars().count();
        self.overlays.archive_password = Some(ArchivePasswordOverlay {
            purpose: ArchivePasswordPurpose::Create,
            input,
            cursor_col,
            visible: false,
            error: None,
        });
    }

    fn remove_archive_create_password(&mut self) {
        if let Some(overlay) = &mut self.overlays.archive_create
            && overlay.options.encryption.is_password_set()
        {
            overlay.options.encryption = ArchiveEncryption::None;
            overlay.error = None;
            self.status = "Archive password removed".to_string();
        }
    }
}

fn archive_create_default_cursor_col(name: &str) -> usize {
    let base = name.strip_suffix(".zip").unwrap_or(name);
    base.chars().count()
}

fn archive_source_label(path: &std::path::Path) -> String {
    let mut name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("item")
        .to_string();
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
        && !name.ends_with('/')
    {
        name.push('/');
    }
    name
}
