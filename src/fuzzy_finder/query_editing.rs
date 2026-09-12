use super::SearchState;

impl SearchState {
    pub(crate) fn query_cursor(&self) -> usize {
        self.query_cursor.min(self.query.chars().count())
    }

    pub(crate) fn move_cursor(&mut self, delta: isize) {
        let max = self.query.chars().count() as isize;
        self.query_cursor = (self.query_cursor as isize + delta).clamp(0, max) as usize;
    }

    pub(crate) fn move_cursor_to_previous_word(&mut self) {
        self.query_cursor = previous_word_start(&self.query, self.query_cursor);
    }

    pub(crate) fn move_cursor_to_next_word(&mut self) {
        self.query_cursor = next_word_start(&self.query, self.query_cursor);
    }

    pub(crate) fn move_cursor_to(&mut self, index: usize) {
        self.query_cursor = index.min(self.query.chars().count());
    }

    pub(crate) fn move_cursor_to_end(&mut self) {
        self.query_cursor = self.query.chars().count();
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        let previous_query = self.query.clone();
        let byte_index = char_to_byte_index(&self.query, self.query_cursor);
        self.query.insert(byte_index, ch);
        self.query_cursor += 1;
        self.refresh_matches(&previous_query);
    }

    pub(crate) fn delete_char_before_cursor(&mut self) {
        let previous_query = self.query.clone();
        if self.query_cursor == 0 {
            self.refresh_matches(&previous_query);
            return;
        }
        let start = char_to_byte_index(&self.query, self.query_cursor.saturating_sub(1));
        let end = char_to_byte_index(&self.query, self.query_cursor);
        self.query.replace_range(start..end, "");
        self.query_cursor -= 1;
        self.refresh_matches(&previous_query);
    }

    pub(crate) fn delete_char_at_cursor(&mut self) {
        let previous_query = self.query.clone();
        let start = char_to_byte_index(&self.query, self.query_cursor);
        if start >= self.query.len() {
            self.refresh_matches(&previous_query);
            return;
        }
        let end = char_to_byte_index(&self.query, self.query_cursor + 1);
        self.query.replace_range(start..end, "");
        self.refresh_matches(&previous_query);
    }

    pub(crate) fn delete_word_before_cursor(&mut self) {
        let previous_query = self.query.clone();
        if self.query_cursor == 0 {
            self.refresh_matches(&previous_query);
            return;
        }
        let start = previous_word_delete_start(&self.query, self.query_cursor);
        remove_char_range(&mut self.query, start, self.query_cursor);
        self.query_cursor = start;
        self.refresh_matches(&previous_query);
    }

    pub(crate) fn delete_word_at_cursor(&mut self) {
        let previous_query = self.query.clone();
        let end = next_word_delete_end(&self.query, self.query_cursor);
        remove_char_range(&mut self.query, self.query_cursor, end);
        self.refresh_matches(&previous_query);
    }
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn is_search_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn previous_word_start(text: &str, cursor: usize) -> usize {
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = cursor.min(chars.len());
    while index > 0 && chars[index - 1].is_whitespace() {
        index -= 1;
    }
    while index > 0 && !chars[index - 1].is_whitespace() && !is_search_word_char(chars[index - 1]) {
        index -= 1;
    }
    while index > 0 && is_search_word_char(chars[index - 1]) {
        index -= 1;
    }
    index
}

fn next_word_start(text: &str, cursor: usize) -> usize {
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = cursor.min(chars.len());
    while index < chars.len() && is_search_word_char(chars[index]) {
        index += 1;
    }
    while index < chars.len() && !is_search_word_char(chars[index]) {
        index += 1;
    }
    index
}

fn remove_char_range(text: &mut String, start_char: usize, end_char: usize) {
    let start = char_to_byte_index(text, start_char);
    let end = char_to_byte_index(text, end_char);
    if start < end {
        text.replace_range(start..end, "");
    }
}

fn previous_word_delete_start(text: &str, cursor: usize) -> usize {
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = cursor.min(chars.len());
    while index > 0 && !is_search_word_char(chars[index - 1]) {
        index -= 1;
    }
    while index > 0 && is_search_word_char(chars[index - 1]) {
        index -= 1;
    }
    index
}

fn next_word_delete_end(text: &str, cursor: usize) -> usize {
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = cursor.min(chars.len());
    if index >= chars.len() {
        return chars.len();
    }
    if is_search_word_char(chars[index]) {
        while index < chars.len() && is_search_word_char(chars[index]) {
            index += 1;
        }
        while index < chars.len() && !is_search_word_char(chars[index]) {
            index += 1;
        }
        return index;
    }
    while index < chars.len() && !is_search_word_char(chars[index]) {
        index += 1;
    }
    while index < chars.len() && is_search_word_char(chars[index]) {
        index += 1;
    }
    index
}

#[cfg(test)]
#[path = "tests/query_editing.rs"]
mod tests;
