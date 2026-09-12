use super::*;
use crate::app::text_edit::{
    next_delete_end, next_word_start, previous_delete_start, previous_word_start,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub fn local_filter_is_editing(&self) -> bool {
        self.file_browser.local_filter.is_editing()
    }

    pub fn local_filter_query(&self) -> &str {
        self.file_browser.local_filter.query()
    }

    pub fn local_filter_has_query(&self) -> bool {
        self.file_browser.local_filter.has_query()
    }

    pub fn local_filter_cursor(&self) -> usize {
        self.file_browser.local_filter.cursor()
    }

    pub(crate) fn open_local_filter(&mut self) {
        self.clear_wheel_scroll();
        self.overlays.help = false;
        self.file_browser.local_filter.activate();
        self.status.clear();
    }

    pub(in crate::app) fn clear_local_filter(&mut self) {
        let was_filtered = !self.file_browser.local_filter.query.is_empty();
        self.file_browser.local_filter = LocalFilter::default();
        if was_filtered {
            self.apply_local_filter_preserving_selection();
        }
        self.status.clear();
    }

    pub(in crate::app) fn clear_local_filter_for_directory_change(&mut self) {
        self.file_browser.local_filter = LocalFilter::default();
    }

    pub(crate) fn handle_local_filter_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.clear_local_filter();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.clear_local_filter();
            }
            KeyCode::Enter => {
                self.file_browser.local_filter.finish_editing();
                self.status.clear();
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.file_browser.local_filter.cursor = previous_word_start(
                    &self.file_browser.local_filter.query,
                    self.file_browser.local_filter.cursor,
                );
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.file_browser.local_filter.cursor = next_word_start(
                    &self.file_browser.local_filter.query,
                    self.file_browser.local_filter.cursor,
                );
            }
            KeyCode::Left => self.move_local_filter_cursor(-1),
            KeyCode::Right => self.move_local_filter_cursor(1),
            KeyCode::Home => self.file_browser.local_filter.cursor = 0,
            KeyCode::End => {
                self.file_browser.local_filter.cursor =
                    self.file_browser.local_filter.query.chars().count();
            }
            _ if local_filter_key_deletes_previous_word(key) => {
                remove_word_before_cursor(
                    &mut self.file_browser.local_filter.query,
                    &mut self.file_browser.local_filter.cursor,
                );
                self.apply_local_filter_preserving_selection();
            }
            KeyCode::Backspace => {
                remove_char_before_cursor(
                    &mut self.file_browser.local_filter.query,
                    &mut self.file_browser.local_filter.cursor,
                );
                self.apply_local_filter_preserving_selection();
            }
            _ if local_filter_key_deletes_next_word(key) => {
                remove_word_at_cursor(
                    &mut self.file_browser.local_filter.query,
                    self.file_browser.local_filter.cursor,
                );
                self.apply_local_filter_preserving_selection();
            }
            KeyCode::Delete => {
                remove_char_at_cursor(
                    &mut self.file_browser.local_filter.query,
                    self.file_browser.local_filter.cursor,
                );
                self.apply_local_filter_preserving_selection();
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                insert_char_at_cursor(
                    &mut self.file_browser.local_filter.query,
                    &mut self.file_browser.local_filter.cursor,
                    ch,
                );
                self.apply_local_filter_preserving_selection();
            }
            _ => {}
        }
        Ok(())
    }

    fn move_local_filter_cursor(&mut self, delta: isize) {
        self.file_browser.local_filter.move_cursor(delta);
    }

    pub(in crate::app) fn apply_local_filter_preserving_selection(&mut self) {
        self.file_browser.apply_local_filter_preserving_selection();
        self.clamp_selection();
        self.sync_scroll();
        self.refresh_preview();
        self.queue_visible_directory_item_counts();
    }

    pub(in crate::app) fn apply_local_filter(&mut self) {
        self.file_browser.apply_local_filter();
    }
}

fn local_filter_key_deletes_previous_word(key: KeyEvent) -> bool {
    (key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('w')))
        || (key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Backspace))
}

fn local_filter_key_deletes_next_word(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::ALT) && matches!(key.code, KeyCode::Char('d'))
}

fn insert_char_at_cursor(text: &mut String, cursor: &mut usize, ch: char) {
    let byte_index = char_to_byte_index(text, *cursor);
    text.insert(byte_index, ch);
    *cursor += 1;
}

fn remove_char_before_cursor(text: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let start = char_to_byte_index(text, *cursor - 1);
    let end = char_to_byte_index(text, *cursor);
    text.replace_range(start..end, "");
    *cursor -= 1;
}

fn remove_char_at_cursor(text: &mut String, cursor: usize) {
    if cursor >= text.chars().count() {
        return;
    }
    let start = char_to_byte_index(text, cursor);
    let end = char_to_byte_index(text, cursor + 1);
    text.replace_range(start..end, "");
}

fn remove_word_before_cursor(text: &mut String, cursor: &mut usize) {
    let start = previous_delete_start(text, *cursor);
    let end = char_to_byte_index(text, *cursor);
    let start_byte = char_to_byte_index(text, start);
    text.replace_range(start_byte..end, "");
    *cursor = start;
}

fn remove_word_at_cursor(text: &mut String, cursor: usize) {
    let end = next_delete_end(text, cursor);
    let start_byte = char_to_byte_index(text, cursor);
    let end_byte = char_to_byte_index(text, end);
    text.replace_range(start_byte..end_byte, "");
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}
