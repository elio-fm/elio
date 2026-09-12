use super::super::text_editing::{
    char_to_byte, next_delete_end, next_word_start, previous_delete_start, previous_word_start,
    remove_char_range,
};
use crate::app::App;
use crate::file_browser::{DirectoryHistoryMode, DirectoryLoadCompletion, PendingDirectoryLoad};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

impl App {
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

    fn cancel_editor_rename_confirm(&mut self) {
        self.file_operations.dismiss_editor_rename_confirm();
        self.status = "Editor rename cancelled".to_string();
    }

    pub(crate) fn handle_bulk_rename_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.file_operations.dismiss_bulk_rename();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.file_operations.dismiss_bulk_rename();
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
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
                    let new_col = previous_word_start(&r.new_names[r.cursor_line], r.cursor_col);
                    r.cursor_col = new_col;
                    r.preferred_col = new_col;
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
                    let new_col = next_word_start(&r.new_names[r.cursor_line], r.cursor_col);
                    r.cursor_col = new_col;
                    r.preferred_col = new_col;
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
                    r.cursor_col = r.cursor_col.saturating_sub(1);
                    r.preferred_col = r.cursor_col;
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
                    let len = r.new_names[r.cursor_line].chars().count();
                    if r.cursor_col < len {
                        r.cursor_col += 1;
                    }
                    r.preferred_col = r.cursor_col;
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
                    r.cursor_col = 0;
                    r.preferred_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
                    r.cursor_col = r.new_names[r.cursor_line].chars().count();
                    r.preferred_col = r.cursor_col;
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut()
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
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut()
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
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
                    let end = next_delete_end(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], r.cursor_col, end);
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
                    let end = next_delete_end(&r.new_names[r.cursor_line], r.cursor_col);
                    remove_char_range(&mut r.new_names[r.cursor_line], r.cursor_col, end);
                    r.line_errors[r.cursor_line] = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut()
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
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
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
                if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
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
        let Some(r) = self.file_operations.bulk_rename_overlay_mut() else {
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
                    self.file_operations.dismiss_bulk_rename();
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
                        if let Some(r) = self.file_operations.bulk_rename_overlay_mut() {
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
                if let Some(overlay) = self.file_operations.editor_rename_confirm_overlay_mut() {
                    overlay.confirmed = true;
                }
            }
            KeyCode::Right | KeyCode::Char('l') if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.editor_rename_confirm_overlay_mut() {
                    overlay.confirmed = false;
                }
            }
            KeyCode::Tab if key.modifiers == KeyModifiers::NONE => {
                if let Some(overlay) = self.file_operations.editor_rename_confirm_overlay_mut() {
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
            self.file_operations.dismiss_rename();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.file_operations.dismiss_rename();
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
                self.confirm_rename()?;
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = self.file_operations.rename_overlay_mut() {
                    let new_col = previous_word_start(&r.input, r.cursor_col);
                    r.cursor_col = new_col;
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = self.file_operations.rename_overlay_mut() {
                    let new_col = next_word_start(&r.input, r.cursor_col);
                    r.cursor_col = new_col;
                }
            }
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.rename_overlay_mut() {
                    r.cursor_col = r.cursor_col.saturating_sub(1);
                }
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.rename_overlay_mut() {
                    let len = r.input.chars().count();
                    if r.cursor_col < len {
                        r.cursor_col += 1;
                    }
                }
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.rename_overlay_mut() {
                    r.cursor_col = 0;
                }
            }
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.rename_overlay_mut() {
                    r.cursor_col = r.input.chars().count();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(r) = self.file_operations.rename_overlay_mut()
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
                if let Some(r) = self.file_operations.rename_overlay_mut()
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
                if let Some(r) = self.file_operations.rename_overlay_mut() {
                    let end = next_delete_end(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, r.cursor_col, end);
                    r.error = None;
                }
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(r) = self.file_operations.rename_overlay_mut() {
                    let end = next_delete_end(&r.input, r.cursor_col);
                    remove_char_range(&mut r.input, r.cursor_col, end);
                    r.error = None;
                }
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                if let Some(r) = self.file_operations.rename_overlay_mut()
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
                if let Some(r) = self.file_operations.rename_overlay_mut() {
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
                if let Some(r) = self.file_operations.rename_overlay_mut() {
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
                self.file_operations.dismiss_rename();
            }
        }
        Ok(())
    }
}
