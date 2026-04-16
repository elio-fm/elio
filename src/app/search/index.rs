use super::super::*;
use std::collections::HashMap;

impl App {
    pub(in crate::app) fn refresh_search_matches(&mut self, previous_query: &str) {
        let Some(search) = &mut self.overlays.search else {
            return;
        };
        search.query_cursor = search.query_cursor.min(search.query.chars().count());

        let next_query = search.query.clone();
        let next_query_key = super::search_cache_key(&next_query);
        if let Some(cached) = search.cached_matches.get(&next_query_key).cloned() {
            search.matches = cached.matches;
        } else {
            let result = {
                let pool = select_search_pool(search, previous_query, &next_query);
                crate::fs::search::filter_candidates_in(
                    &search.candidates,
                    pool.iter().copied(),
                    &next_query,
                    SEARCH_MATCH_LIMIT,
                )
            };

            search.matches = result.matches.clone();
            prune_search_cache(&mut search.cached_matches, &next_query_key);
            search.cached_matches.insert(
                next_query_key,
                super::build_search_cache_entry(result.pool, result.matches),
            );
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
        self.jobs.search_token = self.jobs.search_token.wrapping_add(1);
        self.jobs.search_loading = true;
        self.jobs.search_cache = None;
        let request = SearchRequest {
            token: self.jobs.search_token,
            cwd: self.navigation.cwd.clone(),
            scope,
            show_hidden: self.effective_show_hidden(),
        };
        if !self.jobs.scheduler.submit_search(request) {
            self.jobs.search_loading = false;
            if let Some(search) = &mut self.overlays.search
                && search.scope == scope
            {
                search.loading = false;
                search.error = Some("Search worker unavailable".to_string());
            }
        }
    }
}

fn select_search_pool<'a>(
    search: &'a SearchOverlay,
    previous_query: &str,
    next_query: &str,
) -> &'a [usize] {
    let next_query_key = super::search_cache_key(next_query);
    let previous_query_key = super::search_cache_key(previous_query);

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
