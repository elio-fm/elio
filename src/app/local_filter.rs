use super::*;
use crate::app::text_edit::{
    next_delete_end, next_word_start, previous_delete_start, previous_word_start,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub fn local_filter_is_editing(&self) -> bool {
        self.navigation.local_filter.active
    }

    pub fn local_filter_query(&self) -> &str {
        &self.navigation.local_filter.query
    }

    pub fn local_filter_has_query(&self) -> bool {
        !self.navigation.local_filter.query.trim().is_empty()
    }

    pub fn local_filter_cursor(&self) -> usize {
        self.navigation
            .local_filter
            .cursor
            .min(self.navigation.local_filter.query.chars().count())
    }

    pub(in crate::app) fn open_local_filter(&mut self) {
        self.clear_wheel_scroll();
        self.overlays.help = false;
        self.navigation.local_filter.active = true;
        self.navigation.local_filter.cursor = self.navigation.local_filter.query.chars().count();
        self.status.clear();
    }

    pub(in crate::app) fn clear_local_filter(&mut self) {
        let was_filtered = !self.navigation.local_filter.query.is_empty();
        self.navigation.local_filter = LocalFilter::default();
        if was_filtered {
            self.apply_local_filter_preserving_selection();
        }
        self.status.clear();
    }

    pub(in crate::app) fn clear_local_filter_for_directory_change(&mut self) {
        self.navigation.local_filter = LocalFilter::default();
    }

    pub(in crate::app) fn handle_local_filter_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.clear_local_filter();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.clear_local_filter();
            }
            KeyCode::Enter => {
                self.navigation.local_filter.active = false;
                self.status.clear();
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.navigation.local_filter.cursor = previous_word_start(
                    &self.navigation.local_filter.query,
                    self.navigation.local_filter.cursor,
                );
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.navigation.local_filter.cursor = next_word_start(
                    &self.navigation.local_filter.query,
                    self.navigation.local_filter.cursor,
                );
            }
            KeyCode::Left => self.move_local_filter_cursor(-1),
            KeyCode::Right => self.move_local_filter_cursor(1),
            KeyCode::Home => self.navigation.local_filter.cursor = 0,
            KeyCode::End => {
                self.navigation.local_filter.cursor =
                    self.navigation.local_filter.query.chars().count();
            }
            _ if local_filter_key_deletes_previous_word(key) => {
                remove_word_before_cursor(
                    &mut self.navigation.local_filter.query,
                    &mut self.navigation.local_filter.cursor,
                );
                self.apply_local_filter_preserving_selection();
            }
            KeyCode::Backspace => {
                remove_char_before_cursor(
                    &mut self.navigation.local_filter.query,
                    &mut self.navigation.local_filter.cursor,
                );
                self.apply_local_filter_preserving_selection();
            }
            _ if local_filter_key_deletes_next_word(key) => {
                remove_word_at_cursor(
                    &mut self.navigation.local_filter.query,
                    self.navigation.local_filter.cursor,
                );
                self.apply_local_filter_preserving_selection();
            }
            KeyCode::Delete => {
                remove_char_at_cursor(
                    &mut self.navigation.local_filter.query,
                    self.navigation.local_filter.cursor,
                );
                self.apply_local_filter_preserving_selection();
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                insert_char_at_cursor(
                    &mut self.navigation.local_filter.query,
                    &mut self.navigation.local_filter.cursor,
                    ch,
                );
                self.apply_local_filter_preserving_selection();
            }
            _ => {}
        }
        Ok(())
    }

    fn move_local_filter_cursor(&mut self, delta: isize) {
        let max = self.navigation.local_filter.query.chars().count() as isize;
        self.navigation.local_filter.cursor =
            (self.navigation.local_filter.cursor as isize + delta).clamp(0, max) as usize;
    }

    pub(in crate::app) fn apply_local_filter_preserving_selection(&mut self) {
        let selected_path = self.selected_entry().map(|entry| entry.path.clone());
        self.apply_local_filter();
        self.navigation.selected = selected_path
            .and_then(|path| {
                self.navigation
                    .entries
                    .iter()
                    .position(|entry| entry.path == path)
            })
            .unwrap_or(0);
        self.clamp_selection();
        self.sync_scroll();
        self.refresh_preview();
        self.queue_visible_directory_item_counts();
    }

    pub(in crate::app) fn apply_local_filter(&mut self) {
        let query = self.navigation.local_filter.query.trim();
        if query.is_empty() {
            self.navigation.entries = self.navigation.unfiltered_entries.clone();
            return;
        }

        let query = query.to_ascii_lowercase();
        self.navigation.entries = self
            .navigation
            .unfiltered_entries
            .iter()
            .filter(|entry| entry.name.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-local-filter-{label}-{unique}"))
    }

    fn entry_names(app: &App) -> Vec<String> {
        app.navigation
            .entries
            .iter()
            .map(|entry| entry.name.clone())
            .collect()
    }

    #[test]
    fn local_filter_hides_non_matching_current_directory_entries() {
        let root = temp_path("matches");
        fs::create_dir_all(root.join("src")).expect("directory should be created");
        fs::write(root.join("src-main.rs"), "").expect("file should be created");
        fs::write(root.join("readme.md"), "").expect("file should be created");

        let mut app = App::new_at(root.clone()).expect("app should initialize");
        let all_count = app.navigation.entries.len();
        app.open_local_filter();
        app.handle_local_filter_key(KeyEvent::from(KeyCode::Char('s')))
            .expect("filter input should succeed");
        app.handle_local_filter_key(KeyEvent::from(KeyCode::Char('r')))
            .expect("filter input should succeed");
        app.handle_local_filter_key(KeyEvent::from(KeyCode::Char('c')))
            .expect("filter input should succeed");

        assert_eq!(
            entry_names(&app),
            vec!["src".to_string(), "src-main.rs".to_string()]
        );
        assert_eq!(app.navigation.unfiltered_entries.len(), all_count);
        assert_eq!(
            app.selected_entry().map(|entry| entry.name.as_str()),
            Some("src")
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn local_filter_preserves_matching_selection_and_only_prompt_escape_restores_entries() {
        let root = temp_path("selection");
        fs::create_dir_all(&root).expect("directory should be created");
        fs::write(root.join("alpha.txt"), "").expect("file should be created");
        fs::write(root.join("beta.txt"), "").expect("file should be created");
        fs::write(root.join("alphabet.txt"), "").expect("file should be created");

        let mut app = App::new_at(root.clone()).expect("app should initialize");
        let beta_index = app
            .navigation
            .entries
            .iter()
            .position(|entry| entry.name == "beta.txt")
            .expect("beta should exist");
        app.set_selected(beta_index);
        app.open_local_filter();
        for ch in "beta".chars() {
            app.handle_local_filter_key(KeyEvent::from(KeyCode::Char(ch)))
                .expect("filter input should succeed");
        }

        assert_eq!(entry_names(&app), vec!["beta.txt".to_string()]);
        assert_eq!(
            app.selected_entry().map(|entry| entry.name.as_str()),
            Some("beta.txt")
        );

        app.handle_local_filter_key(KeyEvent::from(KeyCode::Enter))
            .expect("enter should leave filter editing mode");
        assert!(!app.local_filter_is_editing());
        assert_eq!(app.local_filter_query(), "beta");
        assert_eq!(entry_names(&app), vec!["beta.txt".to_string()]);

        app.handle_event(crossterm::event::Event::Key(KeyEvent::from(KeyCode::Esc)))
            .expect("normal escape should keep inactive filter");
        assert!(!app.local_filter_is_editing());
        assert_eq!(app.local_filter_query(), "beta");
        assert_eq!(entry_names(&app), vec!["beta.txt".to_string()]);

        app.open_local_filter();
        app.handle_local_filter_key(KeyEvent::from(KeyCode::Esc))
            .expect("prompt escape should clear filter");
        assert!(!app.local_filter_is_editing());
        assert_eq!(app.local_filter_query(), "");
        assert_eq!(app.navigation.entries.len(), 3);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
