use std::{collections::HashMap, path::PathBuf, sync::Arc};

use super::{SearchCandidate, SearchIndexStats, SearchScope, filter_candidates_in};

pub(crate) const SEARCH_MATCH_LIMIT: usize = 250;
const SEARCH_CACHE_LIMIT: usize = 32;

#[derive(Default)]
pub(crate) struct FuzzyFinderState {
    pub(crate) search: Option<SearchState>,
    pub(crate) token: u64,
    pub(crate) loading: bool,
    pub(crate) cache: Option<SearchCache>,
}

pub(crate) struct SearchState {
    pub(crate) scope: SearchScope,
    pub(crate) query: String,
    pub(crate) query_cursor: usize,
    pub(crate) candidates: Arc<Vec<SearchCandidate>>,
    pub(crate) matches: Vec<usize>,
    pub(crate) cached_matches: HashMap<String, SearchMatchCacheEntry>,
    pub(crate) selected: usize,
    pub(crate) scroll: usize,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) stats: SearchIndexStats,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchMatchCacheEntry {
    pub(crate) pool: Vec<usize>,
    pub(crate) matches: Vec<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchCache {
    pub(crate) cwd: PathBuf,
    pub(crate) scope: SearchScope,
    pub(crate) show_hidden: bool,
    pub(crate) fingerprint: crate::fs::DirectoryFingerprint,
    pub(crate) candidates: Arc<Vec<SearchCandidate>>,
    pub(crate) stats: SearchIndexStats,
}

#[derive(Clone, Debug)]
pub struct SearchRow {
    pub index: usize,
    pub path: PathBuf,
    pub name: String,
    pub relative: String,
    pub is_dir: bool,
    pub symlink: Option<crate::fs::SymlinkInfo>,
    pub selected: bool,
}

impl SearchState {
    pub(crate) fn new(
        scope: SearchScope,
        candidates: Arc<Vec<SearchCandidate>>,
        stats: SearchIndexStats,
        loading: bool,
    ) -> Self {
        let base_matches = (0..candidates.len()).collect::<Vec<_>>();
        let matches = base_matches
            .iter()
            .copied()
            .take(SEARCH_MATCH_LIMIT)
            .collect();
        Self {
            scope,
            query: String::new(),
            query_cursor: 0,
            candidates,
            matches,
            cached_matches: HashMap::from([(
                String::new(),
                build_base_search_cache_entry(base_matches),
            )]),
            selected: 0,
            scroll: 0,
            loading,
            error: None,
            stats,
        }
    }

    pub(crate) fn match_count(&self) -> usize {
        self.cached_matches
            .get(&search_cache_key(&self.query))
            .map(|entry| entry.pool.len())
            .unwrap_or(self.matches.len())
    }

    pub(crate) fn candidate_count(&self) -> usize {
        self.cached_matches
            .get("")
            .map(|entry| entry.pool.len())
            .unwrap_or(0)
    }

    pub(crate) fn scanned_count(&self) -> usize {
        self.stats.visited_nodes.max(self.candidate_count())
    }

    pub(crate) fn rows(&self, max_rows: usize) -> Vec<SearchRow> {
        let end = (self.scroll + max_rows).min(self.matches.len());
        (self.scroll..end)
            .filter_map(|visible_index| {
                let candidate_index = self.matches.get(visible_index).copied()?;
                let candidate = self.candidates.get(candidate_index)?;
                Some(SearchRow {
                    index: visible_index,
                    path: candidate.path.clone(),
                    name: candidate.name.clone(),
                    relative: candidate.relative.clone(),
                    is_dir: candidate.is_dir,
                    symlink: candidate.symlink.clone(),
                    selected: visible_index == self.selected,
                })
            })
            .collect()
    }

    pub(crate) fn move_selection(&mut self, delta: isize) {
        if self.matches.is_empty() {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        let max = self.matches.len().saturating_sub(1) as isize;
        self.selected = (self.selected as isize + delta).clamp(0, max) as usize;
    }

    pub(crate) fn select_index(&mut self, index: usize) {
        if self.matches.is_empty() {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = index.min(self.matches.len().saturating_sub(1));
    }

    pub(crate) fn sync_scroll(&mut self, rows_visible: usize) -> bool {
        if self.matches.is_empty() {
            let changed = self.scroll != 0;
            self.scroll = 0;
            return changed;
        }
        let previous = self.scroll;
        let rows_visible = rows_visible.max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + rows_visible {
            self.scroll = self.selected + 1 - rows_visible;
        }
        self.scroll = self
            .scroll
            .min(self.matches.len().saturating_sub(rows_visible));
        previous != self.scroll
    }

    pub(crate) fn selected_path(&self) -> Option<PathBuf> {
        self.matches
            .get(self.selected)
            .copied()
            .and_then(|index| self.candidates.get(index))
            .map(|candidate| candidate.path.clone())
    }

    pub(crate) fn restart_loading(&mut self) {
        self.candidates = Arc::new(Vec::new());
        self.matches.clear();
        self.cached_matches =
            HashMap::from([(String::new(), build_base_search_cache_entry(Vec::new()))]);
        self.selected = 0;
        self.scroll = 0;
        self.loading = true;
        self.error = None;
        self.stats = SearchIndexStats::default();
    }

    pub(crate) fn replace_candidates(
        &mut self,
        candidates: Arc<Vec<SearchCandidate>>,
        stats: SearchIndexStats,
    ) {
        self.candidates = candidates;
        self.cached_matches = HashMap::from([(
            String::new(),
            build_base_search_cache_entry((0..self.candidates.len()).collect()),
        )]);
        self.loading = false;
        self.error = None;
        self.stats = stats;
        self.refresh_matches("");
    }

    pub(crate) fn fail_loading(&mut self, error: String) {
        self.candidates = Arc::new(Vec::new());
        self.matches.clear();
        self.cached_matches =
            HashMap::from([(String::new(), build_base_search_cache_entry(Vec::new()))]);
        self.selected = 0;
        self.scroll = 0;
        self.loading = false;
        self.error = Some(error);
        self.stats = SearchIndexStats::default();
    }

    pub(crate) fn append_candidates(&mut self, candidates: Vec<SearchCandidate>) {
        let start = self.candidates.len();
        let end = start + candidates.len();
        Arc::make_mut(&mut self.candidates).extend(candidates);

        let base = self
            .cached_matches
            .entry(String::new())
            .or_insert_with(|| build_base_search_cache_entry((0..start).collect()));
        for index in start..end {
            base.pool.push(index);
            if base.matches.len() < SEARCH_MATCH_LIMIT {
                base.matches.push(index);
            }
        }

        let query = self.query.clone();
        let query_key = search_cache_key(&query);
        self.cached_matches
            .retain(|cached_query, _| cached_query.is_empty() || cached_query == &query_key);

        if query_key.is_empty() {
            if let Some(entry) = self.cached_matches.get("") {
                self.matches = entry.matches.clone();
            }
        } else {
            self.update_streamed_query(&query, &query_key, start, end);
        }
        self.selected = self.selected.min(self.matches.len().saturating_sub(1));
        if self.matches.is_empty() {
            self.selected = 0;
            self.scroll = 0;
        }
    }

    pub(crate) fn refresh_matches(&mut self, previous_query: &str) {
        self.query_cursor = self.query_cursor.min(self.query.chars().count());
        let next_query = self.query.clone();
        let next_query_key = search_cache_key(&next_query);
        if let Some(cached) = self.cached_matches.get(&next_query_key).cloned() {
            self.matches = cached.matches;
        } else {
            let result = {
                let pool = select_search_pool(self, previous_query, &next_query);
                filter_candidates_in(
                    &self.candidates,
                    pool.iter().copied(),
                    &next_query,
                    SEARCH_MATCH_LIMIT,
                )
            };
            self.matches = result.matches.clone();
            prune_search_cache(&mut self.cached_matches, &next_query_key);
            self.cached_matches.insert(
                next_query_key,
                build_search_cache_entry(result.pool, result.matches),
            );
        }
        self.selected = self.selected.min(self.matches.len().saturating_sub(1));
        if self.matches.is_empty() {
            self.selected = 0;
            self.scroll = 0;
        }
    }

    fn update_streamed_query(&mut self, query: &str, query_key: &str, start: usize, end: usize) {
        let Some(existing) = self.cached_matches.remove(query_key) else {
            let result = filter_candidates_in(&self.candidates, 0..end, query, SEARCH_MATCH_LIMIT);
            self.matches = result.matches.clone();
            self.cached_matches.insert(
                query_key.to_string(),
                build_search_cache_entry(result.pool, result.matches),
            );
            return;
        };

        let new_result =
            filter_candidates_in(&self.candidates, start..end, query, SEARCH_MATCH_LIMIT);
        let mut pool = existing.pool;
        pool.extend(new_result.pool.iter().copied());
        let rerank_pool = existing
            .matches
            .iter()
            .copied()
            .chain(new_result.pool.iter().copied());
        let matches =
            filter_candidates_in(&self.candidates, rerank_pool, query, SEARCH_MATCH_LIMIT).matches;
        self.matches = matches.clone();
        self.cached_matches.insert(
            query_key.to_string(),
            build_search_cache_entry(pool, matches),
        );
    }
}

pub(crate) fn search_cache_key(query: &str) -> String {
    query.to_lowercase()
}

pub(crate) fn build_search_cache_entry(
    pool: Vec<usize>,
    matches: Vec<usize>,
) -> SearchMatchCacheEntry {
    SearchMatchCacheEntry { pool, matches }
}

pub(crate) fn build_base_search_cache_entry(pool: Vec<usize>) -> SearchMatchCacheEntry {
    let matches = pool.iter().copied().take(SEARCH_MATCH_LIMIT).collect();
    build_search_cache_entry(pool, matches)
}

fn select_search_pool<'a>(
    search: &'a SearchState,
    previous_query: &str,
    next_query: &str,
) -> &'a [usize] {
    let next_query_key = search_cache_key(next_query);
    let previous_query_key = search_cache_key(previous_query);
    if !previous_query_key.is_empty()
        && next_query_key.starts_with(&previous_query_key)
        && let Some(entry) = search.cached_matches.get(&previous_query_key)
    {
        return &entry.pool;
    }
    if let Some(entry) = search
        .cached_matches
        .iter()
        .filter(|(query, _)| !query.is_empty() && next_query_key.starts_with(query.as_str()))
        .max_by_key(|(query, _)| query.len())
        .map(|(_, entry)| entry)
    {
        return &entry.pool;
    }
    search
        .cached_matches
        .get("")
        .map(|entry| entry.pool.as_slice())
        .unwrap_or(&[])
}

fn prune_search_cache(
    cached_matches: &mut HashMap<String, SearchMatchCacheEntry>,
    active_query: &str,
) {
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

#[cfg(test)]
#[path = "tests/search_state.rs"]
mod tests;
