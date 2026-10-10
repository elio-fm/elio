use super::*;
use crate::chooser::validate_name;
use crate::input_handling::text_editing::{
    insert_text_at_cursor, next_delete_end, next_word_start, previous_delete_start,
    previous_word_start, remove_char_range,
};

impl App {
    pub(crate) fn handle_save_as_key(&mut self, key: KeyEvent) {
        let Some(state) = self.chooser.save_as_mut() else {
            return;
        };
        if let Some(path) = state.overwrite_path() {
            match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    state.select_overwrite_confirmation(true);
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    state.select_overwrite_confirmation(false);
                }
                KeyCode::Tab => state.toggle_overwrite_confirmation(),
                KeyCode::Enter if state.overwrite_confirmed() => {
                    let path = path.to_path_buf();
                    state.cancel_overwrite();
                    self.confirm_save_as_path(&path, true);
                }
                KeyCode::Enter => state.cancel_overwrite(),
                KeyCode::Esc => state.cancel_overwrite(),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.cancel_overwrite()
                }
                _ => {}
            }
            return;
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT);
        let alt = key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Left | KeyCode::Right if control => {
                let (input, cursor) = state.input_mut();
                *cursor = if key.code == KeyCode::Left {
                    previous_word_start(input, *cursor)
                } else {
                    next_word_start(input, *cursor)
                };
            }
            KeyCode::Backspace | KeyCode::Char('h' | 'w') if control => {
                let (input, cursor) = state.input_mut();
                let start = previous_delete_start(input, *cursor);
                remove_char_range(input, start, *cursor);
                *cursor = start;
                state.clear_error();
            }
            code if (code == KeyCode::Delete && control) || (code == KeyCode::Char('d') && alt) => {
                let (input, cursor) = state.input_mut();
                let end = next_delete_end(input, *cursor);
                remove_char_range(input, *cursor, end);
                state.clear_error();
            }
            KeyCode::Esc => state.close(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.close(),
            KeyCode::Enter if key.modifiers == KeyModifiers::NONE => self.submit_save_as(),
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                let (_, cursor) = state.input_mut();
                *cursor = cursor.saturating_sub(1);
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                let (input, cursor) = state.input_mut();
                *cursor = (*cursor + 1).min(input.chars().count());
            }
            KeyCode::Home if key.modifiers == KeyModifiers::NONE => *state.input_mut().1 = 0,
            KeyCode::End if key.modifiers == KeyModifiers::NONE => {
                let (input, cursor) = state.input_mut();
                *cursor = input.chars().count();
            }
            KeyCode::Backspace if key.modifiers == KeyModifiers::NONE => {
                let (input, cursor) = state.input_mut();
                let end = *cursor;
                let start = end.saturating_sub(1);
                remove_char_range(input, start, end);
                *cursor = start;
                state.clear_error();
            }
            KeyCode::Delete if key.modifiers == KeyModifiers::NONE => {
                let (input, cursor) = state.input_mut();
                let end = (*cursor + 1).min(input.chars().count());
                remove_char_range(input, *cursor, end);
                state.clear_error();
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let (input, cursor) = state.input_mut();
                insert_text_at_cursor(input, cursor, &ch.to_string());
                state.clear_error();
            }
            _ => {}
        }
    }
    pub(crate) fn handle_save_as_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return Ok(());
        }

        let point = (mouse.column, mouse.row).into();
        let regions = &self.input.screen_regions;
        let inside_panel = regions
            .save_as_panel
            .is_some_and(|panel| panel.contains(point));
        if !inside_panel {
            if let Some(save) = self.chooser.save_as_mut() {
                if save.overwrite() {
                    save.cancel_overwrite();
                } else {
                    save.close();
                }
            }
            return Ok(());
        }

        if !self.chooser.save_as().is_some_and(|save| save.overwrite()) {
            return Ok(());
        }

        if regions
            .save_as_cancel_btn
            .is_some_and(|button| button.contains(point))
        {
            if let Some(save) = self.chooser.save_as_mut() {
                save.cancel_overwrite();
            }
            return Ok(());
        }

        if regions
            .save_as_confirm_btn
            .is_some_and(|button| button.contains(point))
        {
            let path = self.chooser.save_as_mut().and_then(|save| {
                let path = save.overwrite_path()?.to_path_buf();
                save.cancel_overwrite();
                Some(path)
            });
            if let Some(path) = path {
                self.confirm_save_as_path(&path, true);
            }
        }
        Ok(())
    }
    pub(crate) fn submit_save_as(&mut self) {
        let Some(state) = self.chooser.save_as_mut() else {
            return;
        };
        if let Err(error) = validate_name(state.input()) {
            state.set_error(error.to_string());
            return;
        }
        let path = self.file_browser.cwd.join(state.input());
        self.confirm_save_as_path(&path, false);
    }
    pub(crate) fn open_save_as_prompt(&mut self) {
        if let Some(state) = self.chooser.save_as_mut() {
            state.open();
        }
    }
    pub(super) fn confirm_save_as_path(&mut self, path: &Path, overwrite_authorized: bool) {
        // Validate on both submission and overwrite acceptance. A missing target
        // does not imply its parent still exists, or authorize replacing a file.
        let Some(state) = self.chooser.save_as_mut() else {
            return;
        };
        match path.parent().map(std::fs::metadata) {
            Some(Ok(meta)) if meta.is_dir() => {}
            _ => {
                state.set_error("Destination parent is not an existing directory");
                return;
            }
        }
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                state.set_error("Cannot save over a symlink");
                return;
            }
            Ok(meta) if meta.is_dir() => {
                state.set_error("A directory already has that name");
                return;
            }
            Ok(meta) if !meta.is_file() => {
                state.set_error("Cannot save over a special file");
                return;
            }
            Ok(_) if !overwrite_authorized => {
                state.ask_overwrite(path);
                return;
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                state.set_error(error.to_string());
                return;
            }
            _ => {}
        }
        if self.chooser.confirm_path(&self.file_browser.cwd, path) {
            self.should_quit = true;
        }
    }
}
