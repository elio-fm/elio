use super::text_editing::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use crate::app::App;
use crate::file_browser::{DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad};
use crate::file_operations::ClipOp;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::{collections::HashSet, path::PathBuf};

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
        if self.file_operations.archive_create_progress.is_some() {
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

    pub(crate) fn yank(&mut self) {
        self.set_clipboard(ClipOp::Yank);
    }

    pub(crate) fn cut(&mut self) {
        self.set_clipboard(ClipOp::Cut);
    }

    fn set_clipboard(&mut self, op: ClipOp) {
        let paths = self.file_browser.selected_or_focused_paths_sorted();
        if self.file_operations.set_clipboard(paths, op) {
            self.file_browser.selected_paths.clear();
            self.status.clear();
        }
    }

    pub(crate) fn paste(&mut self) -> Result<()> {
        let preparation = self.file_operations.prepare_paste(&self.file_browser.cwd);
        if let Some(request) = preparation.request {
            self.job_scheduler.submit_paste(request);
        }
        if let Some(status) = preparation.status {
            self.status = status;
        }
        Ok(())
    }

    pub(crate) fn drop_external_paths(&mut self, paths: Vec<PathBuf>, op: ClipOp) -> Result<bool> {
        let preparation = self
            .file_operations
            .prepare_drop(&self.file_browser.cwd, paths, op);
        if let Some(request) = preparation.request {
            self.job_scheduler.submit_paste(request);
        }
        self.status = preparation.status;
        Ok(preparation.accepted)
    }

    pub(crate) fn link_yanked(&mut self, relative: bool) -> Result<()> {
        let completion = self
            .file_operations
            .create_symlinks(&self.file_browser.cwd, relative)?;
        if completion.created {
            let _ = self.queue_directory_reload(false);
        }
        self.status = completion.status;
        Ok(())
    }

    pub(crate) fn open_rename_prompt(&mut self) {
        if self.file_browser.in_trash {
            return;
        }
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let name = entry.name.clone();
        let is_dir = entry.is_dir();
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.file_operations.open_rename_prompt(name, is_dir);
    }

    pub(crate) fn open_bulk_rename_prompt(&mut self) {
        if self.file_browser.in_trash {
            return;
        }
        let selected_paths = self.selected_paths_sorted();
        if selected_paths
            .iter()
            .any(|path| self.trash_target_is_inside_trash(path))
        {
            self.status = "Cannot rename items from Trash".to_string();
            return;
        }
        if selected_paths.is_empty() {
            return;
        }
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.file_operations.open_bulk_rename_prompt(selected_paths);
    }

    pub(crate) fn confirm_bulk_rename(&mut self) -> Result<()> {
        let confirmation = self.file_operations.confirm_bulk_rename();
        self.apply_bulk_rename_confirmation(confirmation)
    }

    #[cfg(unix)]
    pub(crate) fn open_editor_bulk_rename(&mut self) -> Result<()> {
        if self.file_browser.in_trash || self.cwd_is_inside_trash_subfolder() {
            return Ok(());
        }
        let selected_paths = if self.file_browser.selected_paths.is_empty() {
            self.selected_entry()
                .map(|entry| vec![entry.path.clone()])
                .unwrap_or_default()
        } else {
            self.selected_paths_in_selection_order()
        };
        if selected_paths.is_empty() {
            return Ok(());
        }
        if selected_paths
            .iter()
            .any(|path| self.trash_target_is_inside_trash(path))
        {
            self.status = "Cannot rename items from Trash".to_string();
            return Ok(());
        }

        let launch: crate::file_operations::EditorBulkRenameLaunch = self
            .file_operations
            .prepare_editor_bulk_rename(selected_paths)?;
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.pending_terminal_task = Some(crate::app::PendingTerminalTask::EditorBulkRename {
            program: launch.program,
            args: launch.args,
            session: launch.session,
        });
        self.status.clear();
        Ok(())
    }

    #[cfg(not(unix))]
    pub(crate) fn open_editor_bulk_rename(&mut self) -> Result<()> {
        self.status = "Editor batch rename is only supported on Unix-like systems".to_string();
        Ok(())
    }

    #[cfg(unix)]
    pub(crate) fn finish_editor_bulk_rename(
        &mut self,
        session: crate::file_operations::BulkRenameEditorSession,
        launch_result: std::io::Result<std::process::ExitStatus>,
    ) -> Result<()> {
        match self
            .file_operations
            .finish_editor_bulk_rename(session, launch_result)?
        {
            crate::file_operations::EditorRenameReview::Ready => self.status.clear(),
            crate::file_operations::EditorRenameReview::Status(status) => self.status = status,
        }
        Ok(())
    }

    pub(crate) fn confirm_editor_rename(&mut self) -> Result<()> {
        let confirmation = self.file_operations.confirm_editor_rename();
        self.apply_bulk_rename_confirmation(confirmation)
    }

    fn apply_bulk_rename_confirmation(
        &mut self,
        confirmation: crate::file_operations::BulkRenameConfirmation,
    ) -> Result<()> {
        let completion = match confirmation {
            crate::file_operations::BulkRenameConfirmation::None => return Ok(()),
            crate::file_operations::BulkRenameConfirmation::Status(status) => {
                self.status = status;
                return Ok(());
            }
            crate::file_operations::BulkRenameConfirmation::Applied(completion) => completion,
        };
        let reload_cwd = self
            .current_directory_escape_for_paths(&completion.changed_old_paths)
            .unwrap_or_else(|| self.file_browser.cwd.clone());
        self.file_browser.selected_paths.clear();
        self.apply_duplicate_rename_pairs(completion.duplicate_rename_pairs);
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: reload_cwd,
            previous_cwd: self.file_browser.cwd.clone(),
            previous_selected_path: None,
            previous_selection_name: None,
            reselect_path: completion.reselect_path,
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(completion.status),
        })?;
        Ok(())
    }

    pub(crate) fn confirm_rename(&mut self) -> Result<()> {
        let Some((original_name, new_name)) = self.file_operations.prepare_rename_names() else {
            return Ok(());
        };
        if self.duplicates_is_open() {
            return self.confirm_duplicate_rename(original_name, new_name);
        }
        let old_path = self
            .file_browser
            .entries
            .iter()
            .find(|entry| entry.name == original_name)
            .map(|entry| entry.path.clone());
        let cwd = self.file_browser.cwd.clone();
        let Some(completion) = self.file_operations.confirm_rename(&cwd, old_path) else {
            return Ok(());
        };
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: cwd.clone(),
            previous_cwd: cwd,
            previous_selected_path: None,
            previous_selection_name: None,
            reselect_path: Some(completion.new_path),
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(completion.status),
        })?;
        Ok(())
    }

    pub(crate) fn open_create_prompt(&mut self) {
        self.overlays.help = false;
        self.fuzzy_finder.search = None;
        self.file_operations.open_create_prompt();
    }

    pub(crate) fn confirm_create(&mut self) -> Result<()> {
        let cwd = self.file_browser.cwd.clone();
        let Some(completion) = self.file_operations.confirm_create(&cwd)? else {
            return Ok(());
        };
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: cwd.clone(),
            previous_cwd: cwd,
            previous_selected_path: self.selected_entry().map(|entry| entry.path.clone()),
            previous_selection_name: self.selected_entry().map(|entry| entry.name.clone()),
            reselect_path: completion.reselect_path,
            history_mode: DirectoryHistoryMode::None,
            refresh_search: false,
            completion: DirectoryLoadCompletion::Status(completion.status),
        })?;
        Ok(())
    }

    pub(crate) fn open_copy_overlay(&mut self) {
        let paths = self.file_browser.selected_or_focused_paths_sorted();
        self.open_copy_overlay_for_paths(paths);
    }

    pub(crate) fn open_copy_overlay_for_paths(&mut self, paths: Vec<PathBuf>) {
        if !self
            .file_operations
            .open_copy_overlay(&self.file_browser.cwd, &paths)
        {
            self.status = "Nothing to copy".to_string();
            return;
        }
        self.overlays.help = false;
        self.status.clear();
    }

    fn confirm_copy_index(&mut self, index: usize) {
        if let Some(status) = self.file_operations.confirm_copy_index(index) {
            self.status = status;
        }
    }

    fn cancel_editor_rename_confirm(&mut self) {
        self.file_operations.dismiss_editor_rename_confirm();
        self.status = "Editor rename cancelled".to_string();
    }

    pub(crate) fn handle_archive_create_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.archive_create = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.file_operations.archive_create = None,
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
                if let Some(overlay) = &mut self.file_operations.archive_create {
                    overlay.cursor_col = previous_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = &mut self.file_operations.archive_create {
                    overlay.cursor_col = next_word_start(&overlay.input, overlay.cursor_col);
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_create {
                    overlay.cursor_col = overlay.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_create {
                    let len = overlay.input.chars().count();
                    if overlay.cursor_col < len {
                        overlay.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_create {
                    overlay.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_create {
                    overlay.cursor_col = overlay.input.chars().count();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(overlay) = &mut self.file_operations.archive_create {
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
                if let Some(overlay) = &mut self.file_operations.archive_create {
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
                if let Some(overlay) = &mut self.file_operations.archive_create {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(overlay) = &mut self.file_operations.archive_create {
                    let end = next_delete_end(&overlay.input, overlay.cursor_col);
                    remove_char_range(&mut overlay.input, overlay.cursor_col, end);
                    overlay.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.archive_create
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
                if let Some(overlay) = &mut self.file_operations.archive_create {
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
                if let Some(overlay) = &mut self.file_operations.archive_create {
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
                    self.file_operations.archive_create = None;
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
        let Some(overlay) = &mut self.file_operations.archive_create else {
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

    pub(crate) fn handle_bulk_rename_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.bulk_rename = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.file_operations.bulk_rename = None;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_bulk_rename()?;
            }
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => {
                self.bulk_rename_move_vertical(-1);
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                self.bulk_rename_move_vertical(1);
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    let new_col = previous_word_start(&r.new_names[r.cursor_line], r.cursor_col);
                    r.cursor_col = new_col;
                    r.preferred_col = new_col;
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    let new_col = next_word_start(&r.new_names[r.cursor_line], r.cursor_col);
                    r.cursor_col = new_col;
                    r.preferred_col = new_col;
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    r.cursor_col = r.cursor_col.saturating_sub(1);
                    r.preferred_col = r.cursor_col;
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    let len = r.new_names[r.cursor_line].chars().count();
                    if r.cursor_col < len {
                        r.cursor_col += 1;
                    }
                    r.preferred_col = r.cursor_col;
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    r.cursor_col = 0;
                    r.preferred_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    r.cursor_col = r.new_names[r.cursor_line].chars().count();
                    r.preferred_col = r.cursor_col;
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.bulk_rename
                    && r.cursor_col > 0
                {
                    let start = previous_delete_start(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], start, r.cursor_col);
                    r.cursor_col = start;
                    r.preferred_col = start;
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.bulk_rename
                    && r.cursor_col > 0
                {
                    let start = previous_delete_start(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], start, r.cursor_col);
                    r.cursor_col = start;
                    r.preferred_col = start;
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Delete
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    let end = next_delete_end(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], r.cursor_col, end);
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    let end = next_delete_end(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], r.cursor_col, end);
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.bulk_rename
                    && r.cursor_col > 0
                {
                    let start = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col - 1);
                    let end = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col);
                    r.new_names[r.cursor_line].replace_range(start..end, "");
                    r.cursor_col -= 1;
                    r.preferred_col = r.cursor_col;
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    let len = r.new_names[r.cursor_line].chars().count();
                    if r.cursor_col < len {
                        let start = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col);
                        let end = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col + 1);
                        r.new_names[r.cursor_line].replace_range(start..end, "");
                        r.line_errors[r.cursor_line] = None;
                    }
                }
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.bulk_rename {
                    let byte = char_to_byte(&r.new_names[r.cursor_line], r.cursor_col);
                    r.new_names[r.cursor_line].insert(byte, ch);
                    r.cursor_col += 1;
                    r.preferred_col = r.cursor_col;
                    r.line_errors[r.cursor_line] = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn bulk_rename_move_vertical(&mut self, delta: isize) {
        let Some(r) = &mut self.file_operations.bulk_rename else {
            return;
        };
        let new_line =
            (r.cursor_line as isize + delta).clamp(0, r.items.len() as isize - 1) as usize;
        if new_line == r.cursor_line {
            return;
        }
        r.cursor_line = new_line;
        let max_col = r.new_names[r.cursor_line].chars().count();
        r.cursor_col = r.preferred_col.min(max_col);
    }

    pub(crate) fn handle_bulk_rename_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .input
                    .screen_regions
                    .rename_panel
                    .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
                if !inside {
                    self.file_operations.bulk_rename = None;
                    return Ok(());
                }
                if let Some(list_area) = self.input.screen_regions.bulk_rename_list_area
                    && list_area.contains((mouse.column, mouse.row).into())
                {
                    let scroll_top = self.input.screen_regions.bulk_rename_scroll_top;
                    let row_offset = (mouse.row - list_area.y) as usize;
                    let line_idx = scroll_top + row_offset;
                    let count = self.file_operations.bulk_rename_item_count();
                    if line_idx < count {
                        let line_len = self
                            .file_operations
                            .bulk_rename_new_name(line_idx)
                            .chars()
                            .count();
                        let char_col = (mouse.column.saturating_sub(list_area.x + 3)) as usize;
                        let cursor_col = char_col.min(line_len);
                        if let Some(r) = &mut self.file_operations.bulk_rename {
                            r.cursor_line = line_idx;
                            r.cursor_col = cursor_col;
                            r.preferred_col = cursor_col;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                self.bulk_rename_move_vertical(-1);
            }
            MouseEventKind::ScrollDown => {
                self.bulk_rename_move_vertical(1);
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_copy_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.copy = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.file_operations.copy = None;
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(index) = self.copy_row_index_for_shortcut(ch) {
                    self.confirm_copy_index(index);
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub(crate) fn handle_copy_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .input
                .screen_regions
                .copy_panel
                .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
            if !inside {
                self.file_operations.copy = None;
                return Ok(());
            }

            if let Some(hit) = self
                .input
                .screen_regions
                .copy_hits
                .iter()
                .find(|hit| hit.rect.contains((mouse.column, mouse.row).into()))
                .cloned()
            {
                self.confirm_copy_index(hit.index);
            }
        }

        Ok(())
    }

    fn copy_row_index_for_shortcut(&self, ch: char) -> Option<usize> {
        let needle = ch.to_ascii_lowercase();
        self.file_operations.copy.as_ref().and_then(|overlay| {
            overlay
                .rows
                .iter()
                .position(|row| row.shortcut.to_ascii_lowercase() == needle)
        })
    }

    pub(crate) fn create_insert_newline(&mut self) {
        let Some(create) = &mut self.file_operations.create else {
            return;
        };
        let tail = {
            let byte = char_to_byte(&create.lines[create.cursor_line], create.cursor_col);
            create.lines[create.cursor_line].split_off(byte)
        };
        create.cursor_line += 1;
        create.lines.insert(create.cursor_line, tail);
        create.line_errors.insert(create.cursor_line, None);
        create.cursor_col = 0;
        create.preferred_col = 0;
    }

    fn create_move_horizontal(&mut self, delta: isize) {
        let Some(create) = &mut self.file_operations.create else {
            return;
        };
        if delta < 0 {
            if create.cursor_col > 0 {
                create.cursor_col -= 1;
            } else if create.cursor_line > 0 {
                create.cursor_line -= 1;
                create.cursor_col = create.lines[create.cursor_line].chars().count();
            }
        } else {
            let len = create.lines[create.cursor_line].chars().count();
            if create.cursor_col < len {
                create.cursor_col += 1;
            } else if create.cursor_line + 1 < create.lines.len() {
                create.cursor_line += 1;
                create.cursor_col = 0;
            }
        }
        create.preferred_col = create.cursor_col;
    }

    fn create_move_word(&mut self, direction: isize) {
        let Some(create) = &mut self.file_operations.create else {
            return;
        };
        let line = &create.lines[create.cursor_line];
        let new_col = if direction < 0 {
            previous_word_start(line, create.cursor_col)
        } else {
            next_word_start(line, create.cursor_col)
        };
        create.cursor_col = new_col;
        create.preferred_col = new_col;
    }

    fn create_move_vertical(&mut self, delta: isize) {
        let Some(create) = &mut self.file_operations.create else {
            return;
        };
        let new_line = (create.cursor_line as isize + delta)
            .clamp(0, create.lines.len() as isize - 1) as usize;
        if new_line == create.cursor_line {
            return;
        }
        create.cursor_line = new_line;
        let max_col = create.lines[create.cursor_line].chars().count();
        create.cursor_col = create.preferred_col.min(max_col);
    }

    fn create_backspace(&mut self) {
        let Some(create) = &mut self.file_operations.create else {
            return;
        };
        if create.cursor_col > 0 {
            let start = char_to_byte(&create.lines[create.cursor_line], create.cursor_col - 1);
            let end = char_to_byte(&create.lines[create.cursor_line], create.cursor_col);
            create.lines[create.cursor_line].replace_range(start..end, "");
            create.cursor_col -= 1;
            create.preferred_col = create.cursor_col;
            create.line_errors[create.cursor_line] = None;
        } else if create.cursor_line > 0 {
            let removed = create.lines.remove(create.cursor_line);
            create.line_errors.remove(create.cursor_line);
            create.cursor_line -= 1;
            create.cursor_col = create.lines[create.cursor_line].chars().count();
            create.preferred_col = create.cursor_col;
            create.lines[create.cursor_line].push_str(&removed);
            create.line_errors[create.cursor_line] = None;
        }
    }

    fn create_delete(&mut self) {
        let Some(create) = &mut self.file_operations.create else {
            return;
        };
        let len = create.lines[create.cursor_line].chars().count();
        if create.cursor_col < len {
            let start = char_to_byte(&create.lines[create.cursor_line], create.cursor_col);
            let end = char_to_byte(&create.lines[create.cursor_line], create.cursor_col + 1);
            create.lines[create.cursor_line].replace_range(start..end, "");
            create.line_errors[create.cursor_line] = None;
        } else if create.cursor_line + 1 < create.lines.len() {
            let next = create.lines.remove(create.cursor_line + 1);
            create.line_errors.remove(create.cursor_line + 1);
            create.lines[create.cursor_line].push_str(&next);
            create.line_errors[create.cursor_line] = None;
        }
    }

    fn create_delete_word_back(&mut self) {
        let Some(create) = &mut self.file_operations.create else {
            return;
        };
        if create.cursor_col == 0 {
            return;
        }
        let line = &mut create.lines[create.cursor_line];
        let start = previous_delete_start(line, create.cursor_col);
        remove_char_range(line, start, create.cursor_col);
        create.cursor_col = start;
        create.preferred_col = start;
        create.line_errors[create.cursor_line] = None;
    }

    fn create_delete_word_forward(&mut self) {
        let Some(create) = &mut self.file_operations.create else {
            return;
        };
        let line = &mut create.lines[create.cursor_line];
        let end = next_delete_end(line, create.cursor_col);
        remove_char_range(line, create.cursor_col, end);
        create.line_errors[create.cursor_line] = None;
    }

    pub(crate) fn handle_create_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.create = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.file_operations.create = None;
            }
            KeyCode::Enter
                if (key.modifiers.contains(KeyModifiers::ALT)
                    || key.modifiers.contains(KeyModifiers::SHIFT))
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.create_insert_newline();
            }
            KeyCode::Char('j') if key.modifiers == KeyModifiers::CONTROL => {
                self.create_insert_newline();
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_create()?;
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_move_word(-1);
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_move_word(1);
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                self.create_move_horizontal(-1);
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                self.create_move_horizontal(1);
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.file_operations.create {
                    c.cursor_col = 0;
                    c.preferred_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(c) = &mut self.file_operations.create {
                    let len = c.lines[c.cursor_line].chars().count();
                    c.cursor_col = len;
                    c.preferred_col = len;
                }
            }
            KeyCode::Up if key.modifiers == KeyModifiers::NONE => {
                self.create_move_vertical(-1);
            }
            KeyCode::Down if key.modifiers == KeyModifiers::NONE => {
                self.create_move_vertical(1);
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_delete_word_back();
            }
            KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_delete_word_back();
            }
            KeyCode::Delete
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.create_delete_word_forward();
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.create_delete_word_forward();
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                self.create_backspace();
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                self.create_delete();
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(c) = &mut self.file_operations.create {
                    let byte = char_to_byte(&c.lines[c.cursor_line], c.cursor_col);
                    c.lines[c.cursor_line].insert(byte, ch);
                    c.cursor_col += 1;
                    c.preferred_col = c.cursor_col;
                    c.line_errors[c.cursor_line] = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_create_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .input
                    .screen_regions
                    .create_panel
                    .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
                if !inside {
                    self.file_operations.create = None;
                    return Ok(());
                }
                if let Some(list_area) = self.input.screen_regions.create_list_area
                    && list_area.contains((mouse.column, mouse.row).into())
                {
                    let scroll_top = self.input.screen_regions.create_scroll_top;
                    let row_offset = (mouse.row - list_area.y) as usize;
                    let line_idx = scroll_top + row_offset;
                    let line_count = self.file_operations.create_line_count();
                    if line_idx < line_count {
                        let line_len = self.file_operations.create_line(line_idx).chars().count();
                        let char_col = (mouse.column.saturating_sub(list_area.x + 3)) as usize;
                        let cursor_col = char_col.min(line_len);
                        if let Some(c) = &mut self.file_operations.create {
                            c.cursor_line = line_idx;
                            c.cursor_col = cursor_col;
                            c.preferred_col = cursor_col;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                self.create_move_vertical(-1);
            }
            MouseEventKind::ScrollDown => {
                self.create_move_vertical(1);
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_editor_rename_confirm_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.cancel_editor_rename_confirm();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc if key.modifiers == KeyModifiers::NONE => {
                self.cancel_editor_rename_confirm();
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                if self.file_operations.editor_rename_confirmed() {
                    self.confirm_editor_rename()?;
                } else {
                    self.cancel_editor_rename_confirm();
                }
            }
            KeyCode::Left | KeyCode::Char('h') if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.editor_rename_confirm {
                    overlay.confirmed = true;
                }
            }
            KeyCode::Right | KeyCode::Char('l') if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.editor_rename_confirm {
                    overlay.confirmed = false;
                }
            }
            KeyCode::Tab if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = &mut self.file_operations.editor_rename_confirm {
                    overlay.confirmed = !overlay.confirmed;
                }
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                self.file_operations.scroll_editor_rename_confirm(-1);
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                self.file_operations.scroll_editor_rename_confirm(1);
            }
            KeyCode::PageUp if key.modifiers == KeyModifiers::NONE => {
                self.file_operations.scroll_editor_rename_confirm(-10);
            }
            KeyCode::PageDown if key.modifiers == KeyModifiers::NONE => {
                self.file_operations.scroll_editor_rename_confirm(10);
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_editor_rename_confirm_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        let inside = self
            .input
            .screen_regions
            .rename_panel
            .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) if !inside => {
                self.cancel_editor_rename_confirm();
            }
            MouseEventKind::Down(MouseButton::Left)
                if self
                    .input
                    .screen_regions
                    .editor_rename_confirm_btn
                    .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into())) =>
            {
                self.confirm_editor_rename()?;
            }
            MouseEventKind::Down(MouseButton::Left)
                if self
                    .input
                    .screen_regions
                    .editor_rename_cancel_btn
                    .is_some_and(|rect| rect.contains((mouse.column, mouse.row).into())) =>
            {
                self.cancel_editor_rename_confirm();
            }
            MouseEventKind::ScrollUp if inside => {
                self.file_operations.scroll_editor_rename_confirm(-1);
            }
            MouseEventKind::ScrollDown if inside => {
                self.file_operations.scroll_editor_rename_confirm(1);
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_rename_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.rename = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.file_operations.rename = None;
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_rename()?;
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.rename {
                    let new_col = previous_word_start(&r.input, r.cursor_col);
                    r.cursor_col = new_col;
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.rename {
                    let new_col = next_word_start(&r.input, r.cursor_col);
                    r.cursor_col = new_col;
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.rename {
                    r.cursor_col = r.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.rename {
                    let len = r.input.chars().count();
                    if r.cursor_col < len {
                        r.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.rename {
                    r.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.rename {
                    r.cursor_col = r.input.chars().count();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.rename
                    && r.cursor_col > 0
                {
                    let start = previous_delete_start(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, start, r.cursor_col);
                    r.cursor_col = start;
                    r.error = None;
                }
            }
            KeyCode::Char('h' | 'w')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.rename
                    && r.cursor_col > 0
                {
                    let start = previous_delete_start(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, start, r.cursor_col);
                    r.cursor_col = start;
                    r.error = None;
                }
            }
            KeyCode::Delete
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.rename {
                    let end = next_delete_end(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, r.cursor_col, end);
                    r.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(r) = &mut self.file_operations.rename {
                    let end = next_delete_end(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, r.cursor_col, end);
                    r.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.rename
                    && r.cursor_col > 0
                {
                    let start = char_to_byte(&r.input, r.cursor_col - 1);
                    let end = char_to_byte(&r.input, r.cursor_col);
                    r.input.replace_range(start..end, "");
                    r.cursor_col -= 1;
                    r.error = None;
                }
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = &mut self.file_operations.rename {
                    let len = r.input.chars().count();
                    if r.cursor_col < len {
                        let start = char_to_byte(&r.input, r.cursor_col);
                        let end = char_to_byte(&r.input, r.cursor_col + 1);
                        r.input.replace_range(start..end, "");
                        r.error = None;
                    }
                }
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(r) = &mut self.file_operations.rename {
                    let byte = char_to_byte(&r.input, r.cursor_col);
                    r.input.insert(byte, ch);
                    r.cursor_col += 1;
                    r.error = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_rename_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let inside = self
                .input
                .screen_regions
                .rename_panel
                .is_some_and(|panel| panel.contains((mouse.column, mouse.row).into()));
            if !inside {
                self.file_operations.rename = None;
            }
        }
        Ok(())
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
