use super::*;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::{collections::HashMap, ffi::OsStr, path::PathBuf, sync::Arc};

impl App {
    pub fn search_is_open(&self) -> bool {
        self.search.is_some()
    }

    pub fn search_query(&self) -> &str {
        self.search
            .as_ref()
            .map(|search| search.query.as_str())
            .unwrap_or("")
    }

    pub fn search_match_count(&self) -> usize {
        self.search
            .as_ref()
            .map(|search| search.matches.len())
            .unwrap_or(0)
    }

    pub fn search_candidate_count(&self) -> usize {
        self.search
            .as_ref()
            .and_then(|search| search.cached_matches.get("").map(Vec::len))
            .unwrap_or(0)
    }

    pub fn search_scope(&self) -> Option<SearchScope> {
        self.search.as_ref().map(|search| search.scope)
    }

    pub fn search_is_loading(&self) -> bool {
        self.search.as_ref().is_some_and(|search| search.loading)
    }

    pub fn search_error(&self) -> Option<&str> {
        self.search
            .as_ref()
            .and_then(|search| search.error.as_deref())
    }

    pub fn search_rows(&self, max_rows: usize) -> Vec<SearchRow> {
        let Some(search) = &self.search else {
            return Vec::new();
        };

        let end = (search.scroll + max_rows).min(search.matches.len());
        (search.scroll..end)
            .filter_map(|visible_index| {
                let candidate_index = search.matches.get(visible_index).copied()?;
                let candidate = search.candidates.get(candidate_index)?;
                Some(SearchRow {
                    index: visible_index,
                    name: candidate.name.clone(),
                    relative: candidate.relative.clone(),
                    is_dir: candidate.is_dir,
                    selected: visible_index == search.selected,
                })
            })
            .collect()
    }

    pub(super) fn open_fuzzy_finder(&mut self, scope: SearchScope) -> Result<()> {
        self.clear_wheel_scroll();
        self.help_open = false;
        let cached = self
            .search_cache
            .as_ref()
            .filter(|cache| cache.cwd == self.cwd && cache.scope == scope)
            .map(|cache| cache.candidates.clone());
        let candidates = cached.clone().unwrap_or_else(|| Arc::new(Vec::new()));
        let base_matches = (0..candidates.len()).collect::<Vec<_>>();
        let matches = base_matches
            .iter()
            .copied()
            .take(SEARCH_MATCH_LIMIT)
            .collect::<Vec<_>>();
        let loading = cached.is_none();
        if cached.is_none() {
            self.prewarm_search_index(scope);
        }
        self.search = Some(SearchOverlay {
            scope,
            query: String::new(),
            query_cursor: 0,
            candidates,
            matches,
            cached_matches: HashMap::from([(String::new(), base_matches)]),
            selected: 0,
            scroll: 0,
            loading,
            error: None,
        });
        self.status.clear();
        Ok(())
    }

    pub(super) fn handle_search_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.search = None;
            self.clear_wheel_scroll();
            self.status.clear();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.search = None;
                self.clear_wheel_scroll();
                self.status.clear();
            }
            KeyCode::Enter => self.confirm_search_selection()?,
            KeyCode::Left => self.move_search_cursor(-1),
            KeyCode::Right => self.move_search_cursor(1),
            KeyCode::Up => self.move_search_selection(-1),
            KeyCode::Down => self.move_search_selection(1),
            KeyCode::PageUp => self.page_search(-1),
            KeyCode::PageDown => self.page_search(1),
            KeyCode::Home => self.move_search_cursor_to(0),
            KeyCode::End => self.move_search_cursor_to_end(),
            KeyCode::Backspace => {
                let previous_query = self
                    .search
                    .as_ref()
                    .map(|search| search.query.clone())
                    .unwrap_or_default();
                if let Some(search) = &mut self.search {
                    remove_char_before_cursor(&mut search.query, &mut search.query_cursor);
                }
                self.refresh_search_matches(&previous_query);
            }
            KeyCode::Delete => {
                let previous_query = self
                    .search
                    .as_ref()
                    .map(|search| search.query.clone())
                    .unwrap_or_default();
                if let Some(search) = &mut self.search {
                    remove_char_at_cursor(&mut search.query, search.query_cursor);
                }
                self.refresh_search_matches(&previous_query);
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let previous_query = self
                    .search
                    .as_ref()
                    .map(|search| search.query.clone())
                    .unwrap_or_default();
                if let Some(search) = &mut self.search {
                    insert_char_at_cursor(&mut search.query, &mut search.query_cursor, ch);
                }
                self.refresh_search_matches(&previous_query);
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn handle_search_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hit) = self
                    .frame_state
                    .search_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    self.select_search_index(hit.index);
                    self.confirm_search_selection()?;
                } else if self
                    .frame_state
                    .search_panel
                    .is_none_or(|rect| !rect_contains(rect, mouse.column, mouse.row))
                {
                    self.search = None;
                    self.clear_wheel_scroll();
                    self.status.clear();
                }
            }
            MouseEventKind::ScrollDown => Self::queue_scroll(
                &mut self.wheel_scroll.search,
                1,
                self.wheel_step_divisor,
                WHEEL_SCROLL_QUEUE_LIMIT_SEARCH,
            ),
            MouseEventKind::ScrollUp => Self::queue_scroll(
                &mut self.wheel_scroll.search,
                -1,
                self.wheel_step_divisor,
                WHEEL_SCROLL_QUEUE_LIMIT_SEARCH,
            ),
            _ => {}
        }
        Ok(())
    }

    pub(super) fn refresh_search_matches(&mut self, _previous_query: &str) {
        let Some(search) = &mut self.search else {
            return;
        };
        search.query_cursor = search.query_cursor.min(search.query.chars().count());

        let next_query = search.query.clone();
        if let Some(cached) = search.cached_matches.get(&next_query).cloned() {
            search.matches = cached;
        } else {
            let pool = search
                .cached_matches
                .get("")
                .cloned()
                .unwrap_or_else(|| (0..search.candidates.len()).collect::<Vec<_>>());

            search.matches = crate::search::filter_candidates_in(
                &search.candidates,
                pool,
                &next_query,
                SEARCH_MATCH_LIMIT,
            );
            prune_search_cache(&mut search.cached_matches, &next_query);
            search
                .cached_matches
                .insert(next_query.clone(), search.matches.clone());
        }

        if search.matches.is_empty() {
            search.selected = 0;
            search.scroll = 0;
            return;
        }

        search.selected = search.selected.min(search.matches.len().saturating_sub(1));
        self.sync_search_scroll();
    }

    pub(super) fn move_search_selection(&mut self, delta: isize) {
        let Some(search) = &mut self.search else {
            return;
        };
        if search.matches.is_empty() {
            search.selected = 0;
            search.scroll = 0;
            return;
        }

        let max_index = search.matches.len().saturating_sub(1) as isize;
        search.selected = (search.selected as isize + delta).clamp(0, max_index) as usize;
        self.sync_search_scroll();
    }

    pub fn search_query_cursor(&self) -> usize {
        self.search
            .as_ref()
            .map(|search| search.query_cursor.min(search.query.chars().count()))
            .unwrap_or(0)
    }

    fn move_search_cursor(&mut self, delta: isize) {
        let Some(search) = &mut self.search else {
            return;
        };
        let max = search.query.chars().count() as isize;
        search.query_cursor = (search.query_cursor as isize + delta).clamp(0, max) as usize;
    }

    fn move_search_cursor_to(&mut self, index: usize) {
        let Some(search) = &mut self.search else {
            return;
        };
        search.query_cursor = index.min(search.query.chars().count());
    }

    fn move_search_cursor_to_end(&mut self) {
        let Some(search) = &mut self.search else {
            return;
        };
        search.query_cursor = search.query.chars().count();
    }

    fn page_search(&mut self, direction: isize) {
        let visible = self.frame_state.search_rows_visible.max(1) as isize;
        self.move_search_selection(direction * visible);
    }

    fn select_search_index(&mut self, index: usize) {
        let Some(search) = &mut self.search else {
            return;
        };
        if search.matches.is_empty() {
            search.selected = 0;
            search.scroll = 0;
            return;
        }
        search.selected = index.min(search.matches.len().saturating_sub(1));
        self.sync_search_scroll();
    }

    fn confirm_search_selection(&mut self) -> Result<()> {
        let Some(path) = self.search.as_ref().and_then(|search| {
            search
                .matches
                .get(search.selected)
                .copied()
                .and_then(|index| search.candidates.get(index))
                .map(|candidate| candidate.path.clone())
        }) else {
            return Ok(());
        };

        self.reveal_path(path)?;
        self.search = None;
        Ok(())
    }

    pub(super) fn sync_search_scroll(&mut self) -> bool {
        let Some(search) = &mut self.search else {
            return false;
        };
        if search.matches.is_empty() {
            let changed = search.scroll != 0;
            search.scroll = 0;
            return changed;
        }

        let previous = search.scroll;
        let rows_visible = self.frame_state.search_rows_visible.max(1);
        if search.selected < search.scroll {
            search.scroll = search.selected;
        } else if search.selected >= search.scroll + rows_visible {
            search.scroll = search.selected + 1 - rows_visible;
        }
        let max_scroll = search.matches.len().saturating_sub(rows_visible);
        search.scroll = search.scroll.min(max_scroll);
        previous != search.scroll
    }

    pub(crate) fn prewarm_search_index(&mut self, scope: SearchScope) {
        self.search_token = self.search_token.wrapping_add(1);
        self.search_loading = true;
        self.search_cache = None;
        let request = SearchRequest {
            token: self.search_token,
            cwd: self.cwd.clone(),
            scope,
        };
        if !self.scheduler.submit_search(request) {
            self.search_loading = false;
            if let Some(search) = &mut self.search
                && search.scope == scope
            {
                search.loading = false;
                search.error = Some("Search worker unavailable".to_string());
            }
        }
    }

    fn reveal_path(&mut self, path: PathBuf) -> Result<()> {
        if path.is_dir() {
            return self.set_dir_transition(
                path,
                DirectoryHistoryMode::PushCurrent,
                None,
                DirectoryLoadCompletion::Status("Opened folder from search".to_string()),
            );
        }

        let Some(parent) = path.parent() else {
            return Ok(());
        };

        let file_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .map(str::to_string)
            .unwrap_or_default();
        self.set_dir_transition(
            parent.to_path_buf(),
            DirectoryHistoryMode::PushCurrent,
            Some(path),
            DirectoryLoadCompletion::Status(format!("Located {}", file_name)),
        )
    }
}

fn prune_search_cache(cached_matches: &mut HashMap<String, Vec<usize>>, active_query: &str) {
    if cached_matches.len() < SEARCH_CACHE_LIMIT {
        return;
    }

    cached_matches.retain(|query, _| {
        query.is_empty() || active_query.starts_with(query) || query.starts_with(active_query)
    });

    while cached_matches.len() >= SEARCH_CACHE_LIMIT {
        let Some(stale_key) = cached_matches
            .keys()
            .filter(|query| !query.is_empty())
            .max_by_key(|query| query.len())
            .cloned()
        else {
            break;
        };
        cached_matches.remove(&stale_key);
    }
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
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
    let start = char_to_byte_index(text, cursor.saturating_sub(1));
    let end = char_to_byte_index(text, *cursor);
    text.replace_range(start..end, "");
    *cursor -= 1;
}

fn remove_char_at_cursor(text: &mut String, cursor: usize) {
    let start = char_to_byte_index(text, cursor);
    if start >= text.len() {
        return;
    }
    let end = char_to_byte_index(text, cursor + 1);
    text.replace_range(start..end, "");
}

#[cfg(test)]
mod tests {
    use super::*;
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
        std::env::temp_dir().join(format!("elio-searching-{label}-{unique}"))
    }

    #[test]
    fn opening_search_restarts_index_when_cache_missing_even_if_loading() {
        let root = temp_path("restarts-index");
        fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create temp tree");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.search_loading = true;
        let previous_token = app.search_token;

        app.open_fuzzy_finder(SearchScope::Folders)
            .expect("failed to open search");

        assert!(app.search_loading);
        assert!(app.search_token > previous_token);

        fs::remove_dir_all(root).expect("failed to remove temp tree");
    }

    #[test]
    fn opening_search_uses_hidden_candidates_even_when_browser_hides_dotfiles() {
        let root = temp_path("hidden-cache");
        fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create temp tree");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.show_hidden = false;
        app.search_cache = Some(SearchCache {
            cwd: root.clone(),
            scope: SearchScope::Folders,
            candidates: Arc::new(vec![crate::search::SearchCandidate {
                path: root.join(".hidden-root/needle"),
                name: "needle".to_string(),
                name_key: "needle".to_string(),
                relative: ".hidden-root/needle".to_string(),
                relative_key: ".hidden-root/needle".to_string(),
                is_dir: true,
            }]),
        });

        app.open_fuzzy_finder(SearchScope::Folders)
            .expect("failed to open search");

        assert_eq!(app.search_candidate_count(), 1);
        assert_eq!(
            app.search_rows(10).first().map(|row| row.relative.as_str()),
            Some(".hidden-root/needle")
        );

        fs::remove_dir_all(root).expect("failed to remove temp tree");
    }

    #[test]
    fn refining_query_rechecks_full_candidate_set() {
        let root = temp_path("query-refine");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let mut candidates = Vec::new();
        for index in 0..300 {
            let name = format!("f{index:03}");
            candidates.push(crate::search::SearchCandidate {
                path: root.join(&name),
                name: name.clone(),
                name_key: name.clone(),
                relative: name.clone(),
                relative_key: name,
                is_dir: true,
            });
        }
        candidates.push(crate::search::SearchCandidate {
            path: root.join("fastfetch"),
            name: "fastfetch".to_string(),
            name_key: "fastfetch".to_string(),
            relative: "fastfetch".to_string(),
            relative_key: "fastfetch".to_string(),
            is_dir: true,
        });

        app.search = Some(SearchOverlay {
            scope: SearchScope::Folders,
            query: "f".to_string(),
            query_cursor: 1,
            candidates: Arc::new(candidates),
            matches: Vec::new(),
            cached_matches: HashMap::from([(String::new(), (0..301).collect())]),
            selected: 0,
            scroll: 0,
            loading: false,
            error: None,
        });
        app.refresh_search_matches("");
        let fastfetch_index = app
            .search
            .as_ref()
            .and_then(|search| {
                search
                    .candidates
                    .iter()
                    .position(|candidate| candidate.relative == "fastfetch")
            })
            .expect("fastfetch candidate should exist");
        assert!(
            !app.search
                .as_ref()
                .expect("search should be open")
                .matches
                .contains(&fastfetch_index)
        );

        if let Some(search) = &mut app.search {
            search.query = "fastfetch".to_string();
        }
        app.refresh_search_matches("f");

        let search = app.search.as_ref().expect("search should be open");
        assert_eq!(search.matches.first().copied(), Some(fastfetch_index));

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn search_query_cursor_inserts_and_deletes_in_place() {
        let root = temp_path("cursor-edit");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.search = Some(SearchOverlay {
            scope: SearchScope::Folders,
            query: "fatch".to_string(),
            query_cursor: 2,
            candidates: Arc::new(Vec::new()),
            matches: Vec::new(),
            cached_matches: HashMap::from([(String::new(), Vec::new())]),
            selected: 0,
            scroll: 0,
            loading: false,
            error: None,
        });

        app.handle_search_key(KeyEvent::from(KeyCode::Char('s')))
            .expect("typing should work");
        assert_eq!(app.search_query(), "fastch");
        assert_eq!(app.search_query_cursor(), 3);

        app.handle_search_key(KeyEvent::from(KeyCode::Left))
            .expect("moving cursor should work");
        app.handle_search_key(KeyEvent::from(KeyCode::Delete))
            .expect("delete should work");
        assert_eq!(app.search_query(), "fatch");
        assert_eq!(app.search_query_cursor(), 2);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn search_rows_ignore_stale_match_indexes() {
        let root = temp_path("stale-match-indexes");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.search = Some(SearchOverlay {
            scope: SearchScope::Folders,
            query: String::new(),
            query_cursor: 0,
            candidates: Arc::new(Vec::new()),
            matches: vec![3],
            cached_matches: HashMap::from([(String::new(), vec![3])]),
            selected: 0,
            scroll: 0,
            loading: false,
            error: None,
        });

        assert!(app.search_rows(10).is_empty());

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn confirm_search_selection_keeps_overlay_open_when_reveal_fails() {
        let root = temp_path("reveal-fails");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        let missing = root.join("missing/file.txt");
        app.search = Some(SearchOverlay {
            scope: SearchScope::Files,
            query: "missing".to_string(),
            query_cursor: 7,
            candidates: Arc::new(vec![crate::search::SearchCandidate {
                path: missing,
                name: "file.txt".to_string(),
                name_key: "file.txt".to_string(),
                relative: "missing/file.txt".to_string(),
                relative_key: "missing/file.txt".to_string(),
                is_dir: false,
            }]),
            matches: vec![0],
            cached_matches: HashMap::from([(String::new(), vec![0])]),
            selected: 0,
            scroll: 0,
            loading: false,
            error: None,
        });

        assert!(app.confirm_search_selection().is_err());
        assert!(app.search.is_some());
        assert_eq!(app.cwd, root);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
