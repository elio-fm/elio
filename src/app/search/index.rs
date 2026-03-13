use super::super::*;
use std::collections::HashMap;

impl App {
    pub(in crate::app) fn refresh_search_matches(&mut self, _previous_query: &str) {
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

            search.matches = crate::fs::search::filter_candidates_in(
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
