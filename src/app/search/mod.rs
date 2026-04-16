mod index;
mod overlay;

#[cfg(test)]
mod tests;

use super::{SEARCH_MATCH_LIMIT, SearchMatchCacheEntry};

pub(in crate::app) fn search_cache_key(query: &str) -> String {
    query.to_lowercase()
}

pub(in crate::app) fn build_search_cache_entry(
    pool: Vec<usize>,
    matches: Vec<usize>,
) -> SearchMatchCacheEntry {
    SearchMatchCacheEntry { pool, matches }
}

pub(in crate::app) fn build_base_search_cache_entry(pool: Vec<usize>) -> SearchMatchCacheEntry {
    let matches = pool.iter().copied().take(SEARCH_MATCH_LIMIT).collect();
    build_search_cache_entry(pool, matches)
}
