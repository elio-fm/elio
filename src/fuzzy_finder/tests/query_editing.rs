use super::*;
use crate::fuzzy_finder::{SearchIndexStats, SearchScope};
use std::sync::Arc;

fn empty_search(query: &str, cursor: usize) -> SearchState {
    let mut search = SearchState::new(
        SearchScope::Files,
        Arc::new(Vec::new()),
        SearchIndexStats::default(),
        false,
    );
    search.query = query.to_string();
    search.query_cursor = cursor;
    search
}

#[test]
fn edits_query_at_character_boundaries() {
    let mut search = empty_search("aé", 1);
    search.insert_char('中');
    assert_eq!(search.query, "a中é");
    assert_eq!(search.query_cursor(), 2);

    search.delete_char_before_cursor();
    assert_eq!(search.query, "aé");
    search.delete_char_at_cursor();
    assert_eq!(search.query, "a");
}

#[test]
fn word_navigation_and_deletion_match_search_input_rules() {
    let mut search = empty_search("foo bar baz", 11);
    search.move_cursor_to_previous_word();
    assert_eq!(search.query_cursor(), 8);
    search.delete_word_before_cursor();
    assert_eq!(search.query, "foo baz");
    assert_eq!(search.query_cursor(), 4);
    search.delete_word_at_cursor();
    assert_eq!(search.query, "foo ");
}
