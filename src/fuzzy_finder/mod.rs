mod candidate_index;
mod fuzzy_matching;
mod query_editing;
mod search_state;

pub use candidate_index::SearchScope;
pub(crate) use candidate_index::{
    SearchCandidate, SearchIndex, SearchIndexBatch, SearchIndexStats, collect_candidates_streaming,
};
pub(crate) use fuzzy_matching::filter_candidates_in;
pub use search_state::SearchRow;
pub(crate) use search_state::{FuzzyFinderState, SearchCache, SearchState};
#[cfg(test)]
pub(crate) use search_state::{
    SearchMatchCacheEntry, build_base_search_cache_entry, build_search_cache_entry,
};
