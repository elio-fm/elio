mod candidate_index;
mod fuzzy_matching;

pub use candidate_index::SearchScope;
pub(crate) use candidate_index::{
    SearchCandidate, SearchIndex, SearchIndexBatch, SearchIndexStats, collect_candidates_streaming,
};
pub(crate) use fuzzy_matching::filter_candidates_in;
